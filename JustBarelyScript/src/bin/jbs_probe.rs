/// jbs_probe: feed a JS file through JustBarelyScript and report what breaks.
///
/// Usage (from workspace root):
///   cargo run -p justbarelyscript --bin jbs_probe -- <path-to-script.js>
use std::env;
use std::fs;

use justbarelyscript::{BrowserExecutionState, DomExecutionState, TokenKind, lex, parse_script};

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).map(String::as_str).unwrap_or(
        "sample_pages/diagnostics/ecosia_hello_world_live_assets/entry-server-routing.CgpsvATT.2553fa3b0e.js",
    );

    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Cannot read '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    println!("=== JBS PROBE: {} ===", path);
    println!(
        "Size: {} bytes, {} lines",
        source.len(),
        source.lines().count()
    );
    println!();

    // ── 1. Lex ───────────────────────────────────────────────────────────────
    println!("── LEXER ──");
    let tokens = lex(&source);

    // Count premature Eof tokens (unknown chars produce Eof mid-stream)
    let premature_eofs: Vec<_> = tokens
        .iter()
        .enumerate()
        .filter(|(i, t)| t.kind == TokenKind::Eof && *i < tokens.len() - 1)
        .collect();

    let real_tokens = tokens.iter().filter(|t| t.kind != TokenKind::Eof).count();

    println!("Total tokens (incl Eof): {}", tokens.len());
    println!("Real (non-Eof) tokens : {}", real_tokens);
    println!(
        "Premature Eof tokens  : {} (each = one unrecognised char)",
        premature_eofs.len()
    );

    if !premature_eofs.is_empty() {
        println!();
        println!("First 10 premature Eof positions (byte offset, line, col):");
        for (_, tok) in premature_eofs.iter().take(10) {
            let byte = tok.span.start;
            let ch = source
                .as_bytes()
                .get(byte)
                .copied()
                .map(|b| b as char)
                .unwrap_or('?');
            println!(
                "  byte={:6}  line={:4}  col={:4}  char={:?}",
                byte, tok.span.line, tok.span.column, ch
            );
        }
    }

    // Identify which unrecognised chars appear and how often
    println!();
    println!("Unknown-char breakdown:");
    let mut char_counts: std::collections::HashMap<char, usize> = std::collections::HashMap::new();
    for (_, tok) in &premature_eofs {
        let byte = tok.span.start;
        let ch = source
            .as_bytes()
            .get(byte)
            .copied()
            .map(|b| b as char)
            .unwrap_or('?');
        *char_counts.entry(ch).or_insert(0) += 1;
    }
    let mut sorted: Vec<_> = char_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (ch, count) in &sorted {
        println!("  {:?}  ×{}", ch, count);
    }

    println!();

    // ── 2. Check for JS constructs JBS can't handle ──────────────────────────
    println!("── CONSTRUCT SCAN ──");
    let checks = [
        ("import ", "ES modules (import)"),
        ("export ", "ES modules (export)"),
        ("=>", "arrow functions"),
        ("?.(", "optional chaining ?.()"),
        ("?.[", "optional chaining ?.[]"),
        ("??", "nullish coalescing ??"),
        ("...", "spread/rest ..."),
        ("`", "template literals"),
        ("async ", "async/await"),
        ("await ", "await"),
        ("class ", "class syntax"),
        ("Symbol(", "Symbol"),
        ("WeakMap", "WeakMap"),
        ("WeakRef", "WeakRef"),
        ("Proxy(", "Proxy"),
        ("import(", "dynamic import()"),
        ("import.meta", "import.meta"),
        ("for(", "for loop"),
        ("for (", "for loop"),
        ("try{", "try/catch"),
        ("try {", "try/catch"),
        ("catch(", "catch"),
        ("catch (", "catch"),
        ("#", "private class fields #"),
        ("@", "decorators @"),
        ("+=", "compound assignment +="),
        ("-=", "compound assignment -="),
        ("++", "increment ++"),
        ("--", "decrement --"),
        ("switch(", "switch statement"),
        ("switch (", "switch statement"),
    ];

    for (needle, label) in &checks {
        let count = source.matches(needle).count();
        if count > 0 {
            println!("  {:4}×  {}", count, label);
        }
    }

    println!();

    // ── 3. Parse ─────────────────────────────────────────────────────────────
    println!("── PARSER ──");
    match parse_script(&source) {
        Ok(program) => {
            println!("Parse: OK  — {} top-level statements", program.body.len());
            println!();

            // ── 4. Execute ───────────────────────────────────────────────────
            println!("── EXECUTION ──");
            let mut state = BrowserExecutionState::default();
            state.execute_program(&program);
            let effects = state.drain_effects();
            println!("Execution: completed");
            println!("Effects  : {}", effects.len());
            for (i, eff) in effects.iter().take(10).enumerate() {
                println!("  [{}] {:?}", i, eff);
            }
        }
        Err(e) => {
            println!("Parse: FAILED");
            println!("  kind   : {:?}", e.kind);
            println!("  message: {}", e.message);
            if let Some(span) = &e.span {
                println!("  line   : {}  col: {}", span.line, span.column);
                if let Some(src_line) = source.lines().nth(span.line.saturating_sub(1)) {
                    let display: String = src_line.chars().take(200).collect();
                    println!("  source : {}", display);
                    println!("  caret  : {}^", " ".repeat(span.column.saturating_sub(1)));
                }
                let offset = span.start.min(source.len());
                let ctx: String = source[offset..].chars().take(80).collect();
                println!("  context: {:?}", ctx);
            }
        }
    }
}
