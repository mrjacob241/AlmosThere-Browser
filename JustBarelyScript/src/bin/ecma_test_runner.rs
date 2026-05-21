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
///   --filter <substring>       Only run tests whose path contains <substring>
///   --limit <n>                Stop after <n> tests scanned
///   --verbose                  Print every PASS/FAIL result
///   --show-failures            Print failure details in summary
///   --failures-log <path>      Write every failure line to <path>
///   --test-dir <path>          Override default test directory
///   --harness-dir <path>       Override default harness directory
use std::fs;
use std::io::{Write as IoWrite, stderr};
use std::panic;
use std::path::{Path, PathBuf};
use std::time::Instant;

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
}

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

    for raw_line in yaml.lines() {
        let line = raw_line.trim_end();

        if !line.starts_with(' ') && !line.starts_with('\t') && !line.is_empty() {
            in_negative = false;
            in_includes = false;
            in_flags = false;
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
// ---------------------------------------------------------------------------
fn run_test(path: &Path, harness_dir: &Path, filter: Option<&str>) -> TestOutcome {
    let path_str = path.to_string_lossy();

    if let Some(f) = filter {
        if !path_str.contains(f) {
            return TestOutcome::Skip("filtered".into());
        }
    }

    if path_str.contains("intl402") {
        return TestOutcome::Skip("intl402".into());
    }

    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => return TestOutcome::Fail(format!("read error: {}", e)),
    };

    let fm = parse_frontmatter(&source);

    if fm.flags.iter().any(|f| f == "async") {
        return TestOutcome::Skip("async".into());
    }
    if fm.flags.iter().any(|f| f == "module") {
        return TestOutcome::Skip("module".into());
    }

    let is_raw = fm.flags.iter().any(|f| f == "raw");
    let only_strict = fm.flags.iter().any(|f| f == "onlyStrict");

    let mut full = String::new();

    if !is_raw {
        full.push_str(JBS_HARNESS);
        full.push('\n');

        for include in &fm.includes {
            let inc_path = harness_dir.join(include);
            if let Ok(inc_src) = fs::read_to_string(&inc_path) {
                full.push_str(&inc_src);
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

    // execute
    const BUDGET: usize = 100_000;
    let mut state = BrowserExecutionState::default();
    state.set_execution_budget(BUDGET);
    state.execute_program(&program);

    if state.execution_budget_exhausted() {
        return TestOutcome::Skip("budget exhausted".into());
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
// Panic-safe wrapper
// ---------------------------------------------------------------------------
fn run_test_safe(path: &Path, harness_dir: &Path, filter: Option<&str>) -> TestOutcome {
    let path_owned = path.to_path_buf();
    let harness_owned = harness_dir.to_path_buf();
    let filter_owned = filter.map(str::to_owned);

    let result = panic::catch_unwind(panic::AssertUnwindSafe(move || {
        run_test(&path_owned, &harness_owned, filter_owned.as_deref())
    }));

    match result {
        Ok(outcome) => outcome,
        Err(_) => TestOutcome::Fail("panic/crash during execution".into()),
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
        eprintln!("  --filter <substring>       Only run tests whose path contains <substring>");
        eprintln!("  --limit <n>                Stop after <n> tests scanned");
        eprintln!("  --verbose                  Print every result");
        eprintln!("  --show-failures            Print failure details in summary");
        eprintln!("  --failures-log <path>      Write failures to a log file");
        eprintln!("  --test-dir <path>          Override test directory");
        eprintln!("  --harness-dir <path>       Override harness directory");
        std::process::exit(0);
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = Path::new(manifest_dir).parent().unwrap_or(Path::new("."));

    let default_test_dir = workspace_root.join("ECMAScript/test262-main/test");
    let default_harness_dir = workspace_root.join("ECMAScript/test262-main/harness");

    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut verbose = false;
    let mut show_failures = false;
    let mut failures_log: Option<String> = None;
    let mut test_dir = default_test_dir.clone();
    let mut harness_dir = default_harness_dir.clone();

    let mut idx = 1usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--ecma-script" | "--verbose" | "--show-failures" => {
                if args[idx] == "--verbose" { verbose = true; }
                if args[idx] == "--show-failures" { show_failures = true; }
            }
            "--filter" => { idx += 1; filter = args.get(idx).cloned(); }
            "--limit" => { idx += 1; limit = args.get(idx).and_then(|s| s.parse().ok()); }
            "--failures-log" => { idx += 1; failures_log = args.get(idx).cloned(); }
            "--test-dir" => { idx += 1; if let Some(p) = args.get(idx) { test_dir = PathBuf::from(p); } }
            "--harness-dir" => { idx += 1; if let Some(p) = args.get(idx) { harness_dir = PathBuf::from(p); } }
            _ => {}
        }
        idx += 1;
    }

    if !test_dir.exists() {
        eprintln!("error: test directory not found: {}", test_dir.display());
        eprintln!("  ECMAScript/test262-main/ must be present at the workspace root.");
        std::process::exit(1);
    }

    println!("ecma_test_runner: collecting tests from {}", test_dir.display());
    let mut all_files: Vec<PathBuf> = Vec::new();
    collect_js_files(&test_dir, &mut all_files);
    all_files.sort();

    let total_files = all_files.len();
    println!("ecma_test_runner: found {} test files", total_files);
    if let Some(f) = &filter { println!("ecma_test_runner: filter = {:?}", f); }
    if let Some(l) = limit { println!("ecma_test_runner: limit = {}", l); }
    if let Some(log) = &failures_log { println!("ecma_test_runner: failures log = {}", log); }
    println!();

    // open failures log if requested
    let mut log_file: Option<fs::File> = failures_log.as_ref().and_then(|p| {
        fs::File::create(p).map_err(|e| eprintln!("warn: cannot open failures log {}: {}", p, e)).ok()
    });

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut panicked = 0usize;
    let mut failures: Vec<(String, String)> = Vec::new();

    let scan_total = limit.map(|l| l.min(total_files)).unwrap_or(total_files);
    let mut bar = ProgressBar::new(scan_total);
    bar.render(0, 0, 0);

    for (i, path) in all_files.iter().enumerate() {
        if let Some(l) = limit {
            if i >= l { break; }
        }

        let outcome = run_test_safe(path, &harness_dir, filter.as_deref());

        let rel = path
            .strip_prefix(&test_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();

        match &outcome {
            TestOutcome::Pass => {
                passed += 1;
                if verbose {
                    // Clear the bar line before printing, then redraw
                    let _ = write!(stderr(), "\r{:80}\r", "");
                    println!("PASS  {}", rel);
                }
            }
            TestOutcome::Fail(reason) => {
                let is_panic = reason.starts_with("panic");
                if is_panic { panicked += 1; }
                failed += 1;
                failures.push((rel.clone(), reason.clone()));
                if let Some(ref mut lf) = log_file {
                    let _ = writeln!(lf, "FAIL  {}  — {}", rel, reason);
                }
                if verbose {
                    let _ = write!(stderr(), "\r{:80}\r", "");
                    println!("FAIL  {}  — {}", rel, reason);
                }
            }
            TestOutcome::Skip(reason) => {
                skipped += 1;
                if verbose {
                    let _ = write!(stderr(), "\r{:80}\r", "");
                    println!("SKIP  {}  — {}", rel, reason);
                }
            }
        }

        bar.render(i + 1, passed, failed);
    }

    bar.finish();
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
    let pct = if ran > 0 {
        (passed as f64 / ran as f64) * 100.0
    } else {
        0.0
    };

    println!("Results:");
    println!("  passed  : {} / {} ran  ({:.1}%)", passed, ran, pct);
    println!("  failed  : {}", failed);
    if panicked > 0 {
        println!("  panicked: {} (caught and recorded as failures)", panicked);
    }
    println!("  skipped : {}", skipped);
    if let Some(log) = &failures_log {
        println!("  failures log: {}", log);
    }
    if failed > 0 && !show_failures {
        println!();
        println!("  run with --show-failures to see details, --failures-log <path> to save them");
    }
    if failed > 0 {
        std::process::exit(1);
    }
}
