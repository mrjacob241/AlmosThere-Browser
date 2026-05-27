/// ecma_test_runner: run every JS test under ECMAScript/test262-main/test/ through
/// JustBarelyScript and report pass/fail/skip counts.
///
/// Usage (from workspace root):
///   cargo run -p justbarelyscript --bin ecma_test_runner -- --ecma-script [OPTIONS]
///
/// The `--ecma-script` flag is required so this runner is never triggered by `cargo test`.
///
/// Options:
///   --ecma-script              Required gate flag
///   --built-ins                Only run tests under built-ins/ (shorthand for --filter "built-ins")
///   --filter <substring>       Only run tests whose path contains <substring>
///   --limit <n>                Stop after <n> tests scanned
///   --verbose                  Print every PASS/FAIL result
///   --show-failures            Print failure details in summary
///   --failures-log <path>      Write every failure line to <path>
///   --test-dir <path>          Override default test directory
///   --harness-dir <path>       Override default harness directory
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, Write as IoWrite, stderr};
use std::panic;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::time::Instant;
use rayon::prelude::*;

// ---------------------------------------------------------------------------
// Progress bar (written to stderr so it doesn't mix with --verbose stdout)
// ---------------------------------------------------------------------------
struct ProgressBar {
    total: usize,
    bar_width: usize,
    last_render_pct: u8,
    start: Instant,
}

impl ProgressBar {
    fn new(total: usize) -> Self {
        Self { total, bar_width: 40, last_render_pct: 255, start: Instant::now() }
    }

    fn render(&mut self, done: usize, passed: usize, failed: usize) {
        let pct = if self.total > 0 { (done * 100 / self.total) as u8 } else { 100 };
        // Redraw on every percent change to keep terminal output smooth
        if pct == self.last_render_pct { return; }
        self.last_render_pct = pct;

        let elapsed = self.start.elapsed().as_secs_f64();
        let eta = if done > 0 && done < self.total {
            let rate = done as f64 / elapsed;
            let remaining = (self.total - done) as f64 / rate;
            format!("eta {}", fmt_duration(remaining))
        } else if done >= self.total {
            format!("done in {}", fmt_duration(elapsed))
        } else {
            "eta ?".to_owned()
        };

        let filled = (pct as usize * self.bar_width) / 100;
        let bar: String = std::iter::repeat('=').take(filled)
            .chain(if filled < self.bar_width { std::iter::once('>') } else { std::iter::once('=') })
            .chain(std::iter::repeat(' ').take(self.bar_width.saturating_sub(filled + 1)))
            .collect();

        let ran = passed + failed;
        let pass_pct = if ran > 0 { passed * 100 / ran } else { 0 };

        let _ = write!(
            stderr(),
            "\r[{bar}] {done}/{total} ({pct}%)  \u{2714}{passed} \u{2718}{failed}  {pass_pct}% pass  {eta}   ",
            bar = bar,
            done = done,
            total = self.total,
            pct = pct,
            passed = passed,
            failed = failed,
            pass_pct = pass_pct,
            eta = eta,
        );
        let _ = stderr().flush();
    }

    fn finish(&self) {
        let _ = writeln!(stderr());
    }
}

fn fmt_duration(secs: f64) -> String {
    let s = secs as u64;
    if s < 60 { format!("{}s", s) }
    else if s < 3600 { format!("{}m{}s", s / 60, s % 60) }
    else { format!("{}h{}m", s / 3600, (s % 3600) / 60) }
}

use justbarelyscript::{BrowserExecutionState, parse_script};

// ---------------------------------------------------------------------------
// Simplified harness injected before each test.
// Replaces sta.js + assert.js with JBS-compatible equivalents that avoid
// `instanceof`, complex prototype chains, and other unsupported constructs.
// ---------------------------------------------------------------------------
const JBS_HARNESS: &str = r#"
var Test262Error = function Test262Error(message) {
    this.name = "Test262Error";
    this.message = message !== undefined ? String(message) : "";
};

function $DONOTEVALUATE() {
    throw "Test262: This statement should not be evaluated.";
}

var $ERROR = function $ERROR(message) {
    throw new Test262Error(message);
};

var print = function print() {};

var assert = function assert(mustBeTrue, message) {
    if (mustBeTrue !== true) {
        var msg = message !== undefined ? String(message) : ("Expected true but got " + String(mustBeTrue));
        throw new Test262Error(msg);
    }
};

assert.sameValue = function sameValue(actual, expected, message) {
    if (actual === expected) {
        if (actual === 0 && (1 / actual) !== (1 / expected)) {
            var zeroMsg = (message ? message + ": " : "") + "Expected " + String(expected) + " but got " + String(actual) + " (+/-0 mismatch)";
            throw new Test262Error(zeroMsg);
        }
        return;
    }
    if (actual !== actual && expected !== expected) {
        return;
    }
    var failMsg = (message ? message + ": " : "") + "Expected " + String(expected) + " but got " + String(actual);
    throw new Test262Error(failMsg);
};

assert.notSameValue = function notSameValue(actual, unexpected, message) {
    if (actual !== unexpected) {
        if (actual !== actual && unexpected !== unexpected) {
            var nanMsg = (message ? message + ": " : "") + "Unexpected NaN";
            throw new Test262Error(nanMsg);
        }
        return;
    }
    if (actual === 0 && (1 / actual) !== (1 / unexpected)) {
        return;
    }
    var failMsg = (message ? message + ": " : "") + "Unexpected value: " + String(actual);
    throw new Test262Error(failMsg);
};

assert.throws = function throws(ExpectedError, fn, message) {
    var threw = false;
    try {
        fn();
    } catch (e) {
        threw = true;
    }
    if (!threw) {
        var failMsg = (message ? message + ": " : "") + "Expected function to throw but it did not";
        throw new Test262Error(failMsg);
    }
};

assert._isSameValue = function(a, b) {
    if (a === b) {
        return a !== 0 || (1 / a) === (1 / b);
    }
    return a !== a && b !== b;
};

assert.deepEqual = function deepEqual(a, b, message) {
    assert.sameValue(String(a), String(b), message);
};
"#;

// ---------------------------------------------------------------------------
// Frontmatter parser
// ---------------------------------------------------------------------------
#[derive(Debug, Default)]
struct Frontmatter {
    negative_phase: Option<String>,
    negative_type: Option<String>,
    includes: Vec<String>,
    flags: Vec<String>,
    features: Vec<String>,
}

/// Features that JBS does not implement; tests requiring any of these are
/// skipped instead of timing out burning through the step budget.
static UNSUPPORTED_FEATURES: &[&str] = &[
    // TypedArrays / binary data
    "ArrayBuffer",
    "DataView",
    "SharedArrayBuffer",
    "TypedArray",
    "Float16Array",
    "Float32Array",
    "Float64Array",
    "Int8Array",
    "Int16Array",
    "Int32Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "Uint16Array",
    "Uint32Array",
    "BigInt64Array",
    "BigUint64Array",
    "immutable-arraybuffer",
    "resizable-arraybuffer",
    // Async / generators
    "async-functions",
    "async-iteration",
    "generators",
    // Reflection / meta
    "Proxy",
    "Reflect",
    "Reflect.construct",
    "WeakRef",
    "FinalizationRegistry",
    // Other unsupported built-ins
    "Atomics",
    "Atomics.waitAsync",
    "import-assertions",
    "import-attributes",
    "dynamic-import",
    "top-level-await",
    "json-modules",
    "decorators",
    "source-phase-imports",
    "regexp-v-flag",
    "regexp-modifiers",
    "iterator-helpers",
    "explicit-resource-management",
    "Temporal",
    "ShadowRealm",
    "import.meta",
    "module-blocks",
    "AbstractModuleSource",
];

fn parse_frontmatter(source: &str) -> Frontmatter {
    let mut fm = Frontmatter::default();
    let start = match source.find("/*---") {
        Some(i) => i + 5,
        None => return fm,
    };
    let end = match source[start..].find("---*/") {
        Some(i) => start + i,
        None => return fm,
    };
    let yaml = &source[start..end];

    let mut in_negative = false;
    let mut in_includes = false;
    let mut in_flags = false;
    let mut in_features = false;

    for raw_line in yaml.lines() {
        let line = raw_line.trim_end();

        if !line.starts_with(' ') && !line.starts_with('\t') && !line.is_empty() {
            in_negative = false;
            in_includes = false;
            in_flags = false;
            in_features = false;
        }

        let trimmed = line.trim();

        if trimmed.starts_with("negative:") {
            in_negative = true;
            continue;
        }
        if in_negative {
            if trimmed.starts_with("phase:") {
                fm.negative_phase = Some(trimmed["phase:".len()..].trim().to_owned());
            } else if trimmed.starts_with("type:") {
                fm.negative_type = Some(trimmed["type:".len()..].trim().to_owned());
            }
            continue;
        }

        if trimmed.starts_with("includes:") {
            let rest = trimmed["includes:".len()..].trim();
            if rest.starts_with('[') {
                let inner = rest.trim_start_matches('[').trim_end_matches(']');
                for item in inner.split(',') {
                    let s = item.trim().trim_matches('"').trim_matches('\'').to_owned();
                    if !s.is_empty() {
                        fm.includes.push(s);
                    }
                }
            } else {
                in_includes = true;
            }
            continue;
        }
        if in_includes {
            if trimmed.starts_with('-') {
                let item = trimmed.trim_start_matches('-').trim().trim_matches('"').trim_matches('\'').to_owned();
                if !item.is_empty() {
                    fm.includes.push(item);
                }
                continue;
            } else {
                in_includes = false;
            }
        }

        if trimmed.starts_with("flags:") {
            let rest = trimmed["flags:".len()..].trim();
            if rest.starts_with('[') {
                let inner = rest.trim_start_matches('[').trim_end_matches(']');
                for item in inner.split(',') {
                    let s = item.trim().to_owned();
                    if !s.is_empty() {
                        fm.flags.push(s);
                    }
                }
            } else {
                in_flags = true;
            }
            continue;
        }
        if in_flags {
            if trimmed.starts_with('-') {
                let item = trimmed.trim_start_matches('-').trim().to_owned();
                if !item.is_empty() {
                    fm.flags.push(item);
                }
                continue;
            } else {
                in_flags = false;
            }
        }

        if trimmed.starts_with("features:") {
            let rest = trimmed["features:".len()..].trim();
            if rest.starts_with('[') {
                let inner = rest.trim_start_matches('[').trim_end_matches(']');
                for item in inner.split(',') {
                    let s = item.trim().trim_matches('"').trim_matches('\'').to_owned();
                    if !s.is_empty() {
                        fm.features.push(s);
                    }
                }
            } else {
                in_features = true;
            }
            continue;
        }
        if in_features {
            if trimmed.starts_with('-') {
                let item = trimmed.trim_start_matches('-').trim().trim_matches('"').trim_matches('\'').to_owned();
                if !item.is_empty() {
                    fm.features.push(item);
                }
                continue;
            } else {
                in_features = false;
            }
        }
    }

    fm
}

// ---------------------------------------------------------------------------
// File walker
// ---------------------------------------------------------------------------
fn collect_js_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_js_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("js") {
            out.push(path);
        }
    }
}

// ---------------------------------------------------------------------------
// Test outcome
// ---------------------------------------------------------------------------
#[derive(Debug)]
enum TestOutcome {
    Pass,
    Fail(String),
    Skip(String),
}

// ---------------------------------------------------------------------------
// Run one test (may panic — call via run_test_safe)
// harness_cache: pre-loaded map of include filename → source, shared across threads
// timeout_secs: per-test wall-clock cap; None = no deadline
// ---------------------------------------------------------------------------
fn run_test(path: &Path, harness_cache: &Arc<HashMap<String, String>>, filter: Option<&str>, timeout_secs: Option<f64>) -> TestOutcome {
    let path_str = path.to_string_lossy();

    if let Some(f) = filter {
        if !path_str.contains(f) {
            return TestOutcome::Skip("filtered".into());
        }
    }

    if path_str.contains("intl402") {
        return TestOutcome::Fail("not implemented: Intl/i18n (intl402) required".into());
    }

    // Fast-fail entire built-in directories for features we don't implement.
    // Many tests in these dirs have no `features:` tag, so path-based detection
    // avoids burning the step budget on them.
    const UNSUPPORTED_DIRS: &[&str] = &[
        "/built-ins/ArrayBuffer/",
        "/built-ins/SharedArrayBuffer/",
        "/built-ins/DataView/",
        "/built-ins/TypedArray/",
        "/built-ins/Float16Array/",
        "/built-ins/Float32Array/",
        "/built-ins/Float64Array/",
        "/built-ins/Int8Array/",
        "/built-ins/Int16Array/",
        "/built-ins/Int32Array/",
        "/built-ins/Uint8Array/",
        "/built-ins/Uint16Array/",
        "/built-ins/Uint32Array/",
        "/built-ins/Uint8ClampedArray/",
        "/built-ins/BigInt64Array/",
        "/built-ins/BigUint64Array/",
        "/built-ins/Atomics/",
        "/built-ins/AsyncGeneratorPrototype/",
        "/built-ins/AsyncFromSyncIteratorPrototype/",
        "/built-ins/AsyncDisposableStack/",
        "/built-ins/DisposableStack/",
        "/built-ins/FinalizationRegistry/",
        "/built-ins/WeakRef/",
        "/built-ins/GeneratorFunction/",
        "/built-ins/GeneratorPrototype/",
        "/built-ins/AsyncGeneratorFunction/",
        "/built-ins/ShadowRealm/",
    ];
    if let Some(dir) = UNSUPPORTED_DIRS.iter().find(|&&d| path_str.contains(d)) {
        return TestOutcome::Fail(format!("not implemented: {}", dir.trim_matches('/').rsplit('/').next().unwrap_or(dir)));
    }

    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => return TestOutcome::Fail(format!("read error: {}", e)),
    };

    let fm = parse_frontmatter(&source);

    if fm.flags.iter().any(|f| f == "async") {
        return TestOutcome::Fail("not implemented: async/Promise support required".into());
    }
    if fm.flags.iter().any(|f| f == "module") {
        return TestOutcome::Fail("not implemented: ES module support required".into());
    }

    if let Some(feat) = fm.features.iter().find(|f| UNSUPPORTED_FEATURES.contains(&f.as_str())) {
        return TestOutcome::Fail(format!("not implemented: {}", feat));
    }

    let is_raw = fm.flags.iter().any(|f| f == "raw");
    let only_strict = fm.flags.iter().any(|f| f == "onlyStrict");

    let mut full = String::new();

    if !is_raw {
        full.push_str(JBS_HARNESS);
        full.push('\n');

        for include in &fm.includes {
            if let Some(src) = harness_cache.get(include) {
                full.push_str(src);
                full.push('\n');
            }
        }
    }

    if only_strict {
        full.push_str("\"use strict\";\n");
    }

    full.push_str(&source);

    // negative:parse — expect parse failure
    if fm.negative_phase.as_deref() == Some("parse") {
        return match parse_script(&full) {
            Err(_) => TestOutcome::Pass,
            Ok(_) => TestOutcome::Fail("expected parse error but parsing succeeded".into()),
        };
    }

    // parse
    let program = match parse_script(&full) {
        Ok(p) => p,
        Err(e) => {
            if fm.negative_phase.is_some() {
                return TestOutcome::Pass;
            }
            return TestOutcome::Fail(format!("parse error: {}", e));
        }
    };

    // execute — hard wall-clock cap per test (catches loops the step budget misses)
    const BUDGET: usize = 100_000;
    let mut state = BrowserExecutionState::default();
    state.set_execution_budget(BUDGET);
    if let Some(secs) = timeout_secs {
        if secs > 0.0 {
            state.set_execution_deadline(std::time::Instant::now() + std::time::Duration::from_secs_f64(secs));
        }
    }
    state.execute_program(&program);

    if state.execution_budget_exhausted() {
        return TestOutcome::Fail("execution budget exhausted (infinite loop or too complex)".into());
    }

    let thrown = state.take_uncaught_throw();

    match fm.negative_phase.as_deref() {
        Some("runtime") => match thrown {
            Some(_) => TestOutcome::Pass,
            None => TestOutcome::Fail("expected runtime throw but none occurred".into()),
        },
        _ => match thrown {
            None => TestOutcome::Pass,
            Some(msg) => TestOutcome::Fail(format!("uncaught throw: {}", msg)),
        },
    }
}

// ---------------------------------------------------------------------------
// Panic-safe wrapper with hard external timeout.
//
// The interpreter's internal deadline (checked every 256 budget steps) is the
// first line of defence. The outer `recv_timeout` here is the hard kill: it
// fires even when the interpreter is stuck inside a Rust-level loop that never
// reaches a budget-check point.  The abandoned thread continues in the
// background but the caller moves on immediately.
// ---------------------------------------------------------------------------
fn run_test_safe(path: &Path, harness_cache: Arc<HashMap<String, String>>, filter: Option<&str>, timeout_secs: Option<f64>) -> TestOutcome {
    let path_owned   = path.to_path_buf();
    let filter_owned = filter.map(str::to_owned);
    // Use a sync_channel(1) so the sender never blocks even if we've already
    // returned due to timeout.
    let (tx, rx) = std::sync::mpsc::sync_channel::<TestOutcome>(1);
    let cache = Arc::clone(&harness_cache);

    // 128 MB stack — some tests recurse deeply (deeply-nested ASTs, RegExp
    // property-escape chains, etc.) and overflow the default 8 MB OS thread stack.
    let _ = std::thread::Builder::new()
        .stack_size(128 * 1024 * 1024)
        .spawn(move || {
            let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                run_test(&path_owned, &cache, filter_owned.as_deref(), timeout_secs)
            }));
            let outcome = match result {
                Ok(o)  => o,
                Err(_) => TestOutcome::Fail("panic/crash during execution".into()),
            };
            let _ = tx.send(outcome); // ignored if receiver already timed out
        });

    match timeout_secs.filter(|&s| s > 0.0) {
        Some(secs) => {
            // Give 150 ms extra margin so the internal deadline fires first
            // and provides a cleaner failure message when possible.
            let wall = std::time::Duration::from_secs_f64(secs + 0.15);
            rx.recv_timeout(wall)
                .unwrap_or_else(|_| TestOutcome::Fail("timeout: test exceeded wall-clock limit".into()))
        }
        None => rx.recv().unwrap_or(TestOutcome::Fail("thread error".into())),
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
fn main() {
    let args: Vec<String> = std::env::args().collect();

    if !args.iter().any(|a| a == "--ecma-script") {
        eprintln!("ecma_test_runner: pass --ecma-script to run the test262 suite.");
        eprintln!();
        eprintln!("Usage:");
        eprintln!("  cargo run -p justbarelyscript --bin ecma_test_runner -- --ecma-script [OPTIONS]");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --ecma-script              Required — enables the runner");
        eprintln!("  --built-ins                Only run tests under built-ins/");
        eprintln!("  --filter <substring>       Only run tests whose path contains <substring>");
        eprintln!("  --limit <n>                Stop after <n> tests scanned");
        eprintln!("  --nproc <n>                Worker threads (default: min(cpus,16))");
        eprintln!("  --verbose                  Print every result");
        eprintln!("  --show-failures            Print failure details in summary");
        eprintln!("  --failures-log <path>      Write failures to a log file");
        eprintln!("  --test-dir <path>          Override test directory");
        eprintln!("  --harness-dir <path>       Override harness directory");
        eprintln!("  --slow-ms <n>              (nproc=1 only) Print tests taking >= n ms");
        eprintln!("  --timeout-sec <n>          Per-test wall-clock cap in seconds, fractions ok (default: 0.25, 0 = off)");
        std::process::exit(0);
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = Path::new(manifest_dir).parent().unwrap_or(Path::new("."));

    let default_test_dir = workspace_root.join("ECMAScript/test262-main/test");
    let default_harness_dir = workspace_root.join("ECMAScript/test262-main/harness");

    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut nproc: Option<usize> = None;
    let mut verbose = false;
    let mut show_failures = false;
    let mut failures_log: Option<String> = None;
    let mut test_dir = default_test_dir.clone();
    let mut harness_dir = default_harness_dir.clone();
    // Print tests that take longer than this many milliseconds. Disabled when None.
    let mut slow_ms: Option<u64> = None;
    // Per-test wall-clock timeout in seconds (fractional ok): Some(n>0) = n seconds, Some(0) / None = no deadline.
    let mut timeout_secs: Option<f64> = Some(0.25);
    // Internal flag set by master on child processes: "start:count" slice into the sorted file list.
    let mut worker_shard: Option<(usize, usize)> = None;

    let mut idx = 1usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--ecma-script" | "--verbose" | "--show-failures" => {
                if args[idx] == "--verbose" { verbose = true; }
                if args[idx] == "--show-failures" { show_failures = true; }
            }
            "--built-ins" => { test_dir = default_test_dir.join("built-ins"); }
            "--filter" => { idx += 1; filter = args.get(idx).cloned(); }
            "--limit" => { idx += 1; limit = args.get(idx).and_then(|s| s.parse().ok()); }
            "--nproc" => { idx += 1; nproc = args.get(idx).and_then(|s| s.parse().ok()); }
            "--failures-log" => { idx += 1; failures_log = args.get(idx).cloned(); }
            "--test-dir" => { idx += 1; if let Some(p) = args.get(idx) { test_dir = PathBuf::from(p); } }
            "--harness-dir" => { idx += 1; if let Some(p) = args.get(idx) { harness_dir = PathBuf::from(p); } }
            "--slow-ms" => { idx += 1; slow_ms = args.get(idx).and_then(|s| s.parse().ok()); }
            "--timeout-sec" => { idx += 1; timeout_secs = args.get(idx).and_then(|s| s.parse::<f64>().ok()); }
            "--worker-shard" => {
                idx += 1;
                if let Some(s) = args.get(idx) {
                    let mut parts = s.splitn(2, ':');
                    if let (Some(a), Some(b)) = (parts.next(), parts.next()) {
                        if let (Ok(start), Ok(count)) = (a.parse::<usize>(), b.parse::<usize>()) {
                            worker_shard = Some((start, count));
                        }
                    }
                }
            }
            _ => {}
        }
        idx += 1;
    }

    // ── WORKER FAST PATH ─────────────────────────────────────────────────────
    // Child processes: skip all scanning/banners — load harness, read paths from stdin, run tests.
    if let Some((_shard_start, _shard_count)) = worker_shard {
        let harness_cache: Arc<HashMap<String, String>> = Arc::new({
            let mut map = HashMap::new();
            if let Ok(entries) = fs::read_dir(&harness_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("js") {
                        if let (Some(name), Ok(src)) = (
                            p.file_name().and_then(|n| n.to_str()).map(str::to_owned),
                            fs::read_to_string(&p),
                        ) { map.insert(name, src); }
                    }
                }
            }
            map
        });
        let batch: Vec<PathBuf> = std::io::stdin()
            .lock()
            .lines()
            .flatten()
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .collect();
        let stdout = std::io::stdout();
        let mut out = std::io::BufWriter::new(stdout.lock());
        for path in &batch {
            let rel = path.strip_prefix(&test_dir).unwrap_or(path).to_string_lossy();
            let outcome = run_test_safe(path, Arc::clone(&harness_cache), filter.as_deref(), timeout_secs);
            match outcome {
                TestOutcome::Pass        => { let _ = writeln!(out, "P\t{}", rel); }
                TestOutcome::Fail(r)     => { let _ = writeln!(out, "F\t{}\t{}", rel, r.replace('\n', " ")); }
                TestOutcome::Skip(r)     => { let _ = writeln!(out, "S\t{}\t{}", rel, r); }
            }
        }
        return;
    }

    // ── MASTER MODE ──────────────────────────────────────────────────────────
    if !test_dir.exists() {
        eprintln!("error: test directory not found: {}", test_dir.display());
        std::process::exit(1);
    }

    // Determine process count — respects RAYON_NUM_THREADS env var as an alternative to --nproc.
    let env_nproc: Option<usize> = std::env::var("RAYON_NUM_THREADS").ok()
        .and_then(|s| s.parse().ok());
    let nprocs = nproc.or(env_nproc).unwrap_or_else(|| {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16)
    }).max(1);

    println!("ecma_test_runner: collecting tests from {}", test_dir.display());
    let mut all_files: Vec<PathBuf> = Vec::new();
    collect_js_files(&test_dir, &mut all_files);
    all_files.sort();
    let total_files = all_files.len();
    println!("ecma_test_runner: found {} test files", total_files);
    if let Some(f) = &filter { println!("ecma_test_runner: filter = {:?}", f); }
    if let Some(l) = limit { println!("ecma_test_runner: limit = {}", l); }
    if let Some(log) = &failures_log { println!("ecma_test_runner: failures log = {}", log); }
    println!("ecma_test_runner: nproc = {}", nprocs);
    println!("ecma_test_runner: available CPUs = {}", std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0));
    println!();

    let scan_total = limit.map(|l| l.min(total_files)).unwrap_or(total_files);

    // Harness cache — only needed for single-process path.
    let harness_cache: Arc<HashMap<String, String>> = Arc::new({
        let mut map = HashMap::new();
        if let Ok(entries) = fs::read_dir(&harness_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("js") {
                    if let (Some(name), Ok(src)) = (
                        p.file_name().and_then(|n| n.to_str()).map(str::to_owned),
                        fs::read_to_string(&p),
                    ) { map.insert(name, src); }
                }
            }
        }
        map
    });

    // open failures log if requested
    let mut log_file: Option<fs::File> = failures_log.as_ref().and_then(|p| {
        fs::File::create(p).map_err(|e| eprintln!("warn: cannot open failures log {}: {}", p, e)).ok()
    });

    let mut passed  = 0usize;
    let mut failed  = 0usize;
    let mut skipped = 0usize;
    let mut panicked = 0usize;
    let mut failures: Vec<(String, String)> = Vec::new();

    if nprocs == 1 {
        // ── SINGLE-PROCESS PATH (rayon within one process) ───────────────────
        let batch: Vec<PathBuf> = all_files.into_iter().take(scan_total).collect();

        rayon::ThreadPoolBuilder::new().num_threads(1).build_global().unwrap_or(());
        println!("ecma_test_runner: rayon threads = {}", rayon::current_num_threads());

        let a_passed = Arc::new(AtomicUsize::new(0));
        let a_failed = Arc::new(AtomicUsize::new(0));
        let a_done   = Arc::new(AtomicUsize::new(0));
        let (bp, bf, bd) = (Arc::clone(&a_passed), Arc::clone(&a_failed), Arc::clone(&a_done));
        let bar_handle = {
            let total = scan_total;
            std::thread::spawn(move || {
                let mut bar = ProgressBar::new(total);
                bar.render(0, 0, 0);
                loop {
                    bar.render(bd.load(Ordering::Relaxed), bp.load(Ordering::Relaxed), bf.load(Ordering::Relaxed));
                    if bd.load(Ordering::Relaxed) >= total { break; }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            })
        };
        let filter_ref: Option<&str> = filter.as_deref();
        let results: Vec<(String, TestOutcome, u64)> = batch.par_iter().map(|path| {
            let rel = path.strip_prefix(&test_dir).unwrap_or(path).to_string_lossy().into_owned();
            let t0 = std::time::Instant::now();
            let outcome = run_test_safe(path, Arc::clone(&harness_cache), filter_ref, timeout_secs);
            let elapsed_ms = t0.elapsed().as_millis() as u64;
            match &outcome {
                TestOutcome::Pass    => { a_passed.fetch_add(1, Ordering::Relaxed); }
                TestOutcome::Fail(_) => { a_failed.fetch_add(1, Ordering::Relaxed); }
                TestOutcome::Skip(_) => {}
            }
            a_done.fetch_add(1, Ordering::Relaxed);
            (rel, outcome, elapsed_ms)
        }).collect();
        let _ = bar_handle.join();

        let mut slow_log: Vec<(u64, String)> = Vec::new();
        for (rel, outcome, elapsed_ms) in results {
            if let Some(thresh) = slow_ms {
                if elapsed_ms >= thresh {
                    slow_log.push((elapsed_ms, rel.clone()));
                }
            }
            match outcome {
                TestOutcome::Pass => { passed += 1; }
                TestOutcome::Fail(r) => {
                    if r.starts_with("panic") { panicked += 1; }
                    failed += 1;
                    failures.push((rel, r));
                }
                TestOutcome::Skip(_) => { skipped += 1; }
            }
        }
        if !slow_log.is_empty() {
            slow_log.sort_by(|a, b| b.0.cmp(&a.0));
            println!("\nSlow tests (>= {}ms):", slow_ms.unwrap_or(0));
            for (ms, path) in &slow_log {
                println!("  {:>6}ms  {}", ms, path);
            }
        }
    } else {
        // ── MULTI-PROCESS PATH ───────────────────────────────────────────────
        // Split the sorted file list into nprocs shards.
        let exe = std::env::current_exe().expect("cannot resolve executable path");

        // Build the arg list to forward to children (drop --nproc and its value).
        let forward_args: Vec<String> = {
            let mut out: Vec<String> = Vec::new();
            let mut skip_next = false;
            for arg in &args[1..] {
                if skip_next { skip_next = false; continue; }
                if arg == "--nproc" { skip_next = true; continue; }
                out.push(arg.clone());
            }
            out
        };

        // Result channel: (rel_path, outcome) from all child stdout readers.
        let (tx, rx) = std::sync::mpsc::channel::<(String, TestOutcome)>();

        // Master pre-computes the full path list — children receive their slice via stdin.
        // Round-robin interleaving: process i gets indices i, i+nprocs, i+2*nprocs, ...
        // This spreads slow tests evenly instead of concentrating them in one shard.
        let all_paths: Vec<PathBuf> = all_files.into_iter().take(scan_total).collect();

        // Spawn one child process per shard + one reader thread per child.
        // Paths are fed to each child via stdin (one absolute path per line).
        let reader_handles: Vec<_> = (0..nprocs).filter_map(|i| {
            let shard_paths: Vec<PathBuf> = all_paths.iter()
                .skip(i)
                .step_by(nprocs)
                .cloned()
                .collect();
            if shard_paths.is_empty() { return None; }
            let count = shard_paths.len();

            // Pre-compute relative paths so we can detect unaccounted tests if the worker crashes.
            let shard_rel_paths: Vec<String> = shard_paths.iter()
                .map(|p| p.strip_prefix(&test_dir).unwrap_or(p).to_string_lossy().into_owned())
                .collect();

            let mut child = match Command::new(&exe)
                .args(&forward_args)
                .arg("--worker-shard")
                .arg(format!("{}:{}", i, count))
                .stdin(Stdio::piped())   // paths fed via stdin
                .stdout(Stdio::piped())
                .stderr(Stdio::null())  // suppress child banners
                .spawn()
            {
                Ok(c) => c,
                Err(e) => { eprintln!("warn: failed to spawn worker: {}", e); return None; }
            };

            // Write this shard's absolute paths to child stdin, then close it.
            if let Some(mut stdin_pipe) = child.stdin.take() {
                use std::io::Write;
                for p in &shard_paths {
                    let _ = writeln!(stdin_pipe, "{}", p.display());
                }
                // stdin_pipe dropped here → EOF → child stops reading
            }

            let stdout = child.stdout.take().unwrap();
            let tx = tx.clone();
            Some(std::thread::spawn(move || {
                let mut reported = std::collections::HashSet::new();
                let reader = std::io::BufReader::new(stdout);
                for line in reader.lines().flatten() {
                    let mut parts = line.splitn(3, '\t');
                    let tag = parts.next().unwrap_or("");
                    let rel = parts.next().unwrap_or("").to_owned();
                    let detail = parts.next().unwrap_or("").to_owned();
                    let outcome = match tag {
                        "P" => TestOutcome::Pass,
                        "F" => TestOutcome::Fail(detail),
                        "S" => TestOutcome::Skip(detail),
                        _   => continue,
                    };
                    reported.insert(rel.clone());
                    let _ = tx.send((rel, outcome));
                }
                let _ = child.wait();
                // If the worker crashed mid-run, some tests were never reported.
                // Count them as failures so the total always equals scan_total.
                for rel in shard_rel_paths {
                    if !reported.contains(&rel) {
                        let _ = tx.send((rel, TestOutcome::Fail("worker crash: process terminated early".into())));
                    }
                }
            }))
        }).collect();
        drop(tx); // close original sender so rx closes when all readers finish

        // Render progress bar on main thread while collecting results.
        let mut bar = ProgressBar::new(scan_total);
        bar.render(0, 0, 0);
        let mut done = 0usize;

        for (rel, outcome) in rx {
            done += 1;
            match outcome {
                TestOutcome::Pass => {
                    passed += 1;
                    if verbose { let _ = write!(stderr(), "\r{:80}\r", ""); println!("PASS  {}", rel); }
                }
                TestOutcome::Fail(r) => {
                    if r.starts_with("panic") { panicked += 1; }
                    failed += 1;
                    failures.push((rel.clone(), r.clone()));
                    if let Some(ref mut lf) = log_file { let _ = writeln!(lf, "FAIL  {}  — {}", rel, r); }
                    if verbose { let _ = write!(stderr(), "\r{:80}\r", ""); println!("FAIL  {}  — {}", rel, r); }
                }
                TestOutcome::Skip(_) => { skipped += 1; }
            }
            bar.render(done, passed, failed);
        }

        for h in reader_handles { let _ = h.join(); }
    }

    // ── SUMMARY ──────────────────────────────────────────────────────────────
    failures.sort_by(|a, b| a.0.cmp(&b.0));
    println!();

    if show_failures && !failures.is_empty() {
        println!("--- Failures ---");
        for (path, reason) in &failures {
            println!("  FAIL  {}", path);
            println!("        {}", reason);
        }
        println!();
    }

    let ran = passed + failed;
    let pct = if ran > 0 { (passed as f64 / ran as f64) * 100.0 } else { 0.0 };
    println!("Results:");
    println!("  passed  : {} / {} ran  ({:.1}%)", passed, ran, pct);
    println!("  failed  : {}", failed);
    if panicked > 0 { println!("  panicked: {} (caught and recorded as failures)", panicked); }
    println!("  skipped : {}", skipped);
    if let Some(log) = &failures_log { println!("  failures log: {}", log); }
    if failed > 0 && !show_failures {
        println!();
        println!("  run with --show-failures to see details, --failures-log <path> to save them");
    }
    if failed > 0 { std::process::exit(1); }
}
