#!/usr/bin/env python3
"""
AlmostThere debug socket client.

Usage:
    python3 debug_client.py [--port 9876] [--filter TEXT] [--raw] [--no-color]

Connect while the browser is running with --debug-socket:
    cargo run -p almostthere_browser -- --debug-socket --record-events
"""

import argparse
import json
import socket
import sys
import time

# ── ANSI colours ──────────────────────────────────────────────────────────────

RESET   = "\033[0m"
BOLD    = "\033[1m"
DIM     = "\033[2m"
RED     = "\033[31m"
GREEN   = "\033[32m"
YELLOW  = "\033[33m"
BLUE    = "\033[34m"
MAGENTA = "\033[35m"
CYAN    = "\033[36m"

_USE_COLOR = True

def c(*codes: str) -> str:
    return "".join(codes) if _USE_COLOR else ""


# ── Formatters ────────────────────────────────────────────────────────────────

def _label(ev: dict) -> str:
    return ev.get("label") or ev.get("url") or ev.get("detail") or ""


def fmt_event(ev: dict) -> str | None:
    event = ev.get("event", "")

    # ── navigation ────────────────────────────────────────────────────────────
    if event == "navigation.requested":
        return f"{c(BLUE, BOLD)}▶ navigate  {ev.get('url','')}{c(RESET)}"

    if event == "navigation.fetch.completed":
        return (
            f"{c(BLUE)}✓ fetched   {ev.get('final_url', ev.get('url',''))}"
            f"  ({ev.get('html_bytes','?')} bytes){c(RESET)}"
        )

    if event == "navigation.fetch.failed":
        return f"{c(RED)}✗ fetch failed  {ev.get('url','')}  {ev.get('error','')}{c(RESET)}"

    if event == "navigation.scripts.started":
        return f"{c(BLUE)}⚙ scripts started  {ev.get('url','')}{c(RESET)}"

    # ── resource cache ────────────────────────────────────────────────────────
    if event == "js.resource.cache_miss":
        return f"{c(DIM)}  cache-miss  {ev.get('url','')}{c(RESET)}"

    if event == "js.resource.cache_hit":
        return f"{c(DIM)}  cache-hit   {ev.get('url','')}{c(RESET)}"

    # ── parse ─────────────────────────────────────────────────────────────────
    if event == "js.script.parsed":
        return (
            f"{c(CYAN)}  parsed #{ev.get('index','?'):>2}  {ev.get('bytes','?'):>7} B"
            f"  {ev.get('statements','?')} stmts  {ev.get('label','')}{c(RESET)}"
        )

    # ── execute ───────────────────────────────────────────────────────────────
    if event == "js.script.execute.started":
        return f"{c(GREEN)}▸ exec #{ev.get('index','?') or '?':>2}  {ev.get('label','')}{c(RESET)}"

    if event == "js.script.execute.completed":
        elapsed  = ev.get("elapsed_ms", "?")
        effects  = ev.get("effects", "?")
        budget   = ev.get("budget_exhausted", "false") == "true"
        budget_s = f"  {c(RED, BOLD)}BUDGET EXHAUSTED{c(RESET)}" if budget else ""
        color    = RED if budget else GREEN
        return (
            f"{c(color)}✓ done #{ev.get('index','?') or '?':>2}"
            f"  {elapsed:>6} ms  effects={effects}{budget_s}"
            f"  {c(DIM)}{ev.get('label','')}{c(RESET)}"
        )

    # ── runtime traces ────────────────────────────────────────────────────────
    if event == "js.runtime.trace":
        kind   = ev.get("kind", "")
        detail = ev.get("detail", "")
        color  = RED if "warning" in kind or "unsupported" in kind else YELLOW
        return f"  {c(color)}⚠ trace  {kind}: {detail}{c(RESET)}"

    # ── network effects ───────────────────────────────────────────────────────
    if event.startswith("js.network"):
        url = ev.get("url") or ev.get("detail") or ""
        return f"  {c(MAGENTA, BOLD)}⚡ {event:<30}  {url}{c(RESET)}"

    # ── panic ─────────────────────────────────────────────────────────────────
    if event == "app.panic":
        return f"{c(RED, BOLD)}💥 PANIC  {ev.get('message','')}  @ {ev.get('location','')}{c(RESET)}"

    # ── live-js artifact ──────────────────────────────────────────────────────
    if event.startswith("live_js_debug"):
        return f"{c(DIM)}  {event}  {ev.get('path','')}{c(RESET)}"

    # ── session ───────────────────────────────────────────────────────────────
    if event.startswith("session."):
        extra = {k: v for k, v in ev.items()
                 if k not in ("schema_version", "session_id", "timestamp_ms", "event")}
        return f"{c(DIM)}  {event}  {extra}{c(RESET)}"

    # ── input: skip unless -v ─────────────────────────────────────────────────
    if event == "input.event":
        return None

    # ── generic fallback ──────────────────────────────────────────────────────
    extra = {k: v for k, v in ev.items()
             if k not in ("schema_version", "session_id", "timestamp_ms", "event")}
    return f"{c(DIM)}{event}  {extra}{c(RESET)}"


# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> None:
    global _USE_COLOR

    parser = argparse.ArgumentParser(
        description="Stream AlmostThere browser debug events from the debug socket."
    )
    parser.add_argument("--host",    default="127.0.0.1")
    parser.add_argument("--port",    type=int, default=9876)
    parser.add_argument("--filter",  help="Only show lines containing this text (case-sensitive)")
    parser.add_argument("--raw",     action="store_true", help="Print raw JSON instead of formatting")
    parser.add_argument("--no-color",action="store_true", help="Disable ANSI colours")
    parser.add_argument("--no-retry", action="store_true",
                        help="Exit instead of retrying when the browser is not running")
    args = parser.parse_args()

    if args.no_color or not sys.stdout.isatty():
        _USE_COLOR = False

    addr = (args.host, args.port)
    print(f"Connecting to {args.host}:{args.port}  (start browser with --debug-socket)")

    while True:
        try:
            sock = socket.create_connection(addr, timeout=5)
            sock.settimeout(None)
            print(f"Connected. Streaming events — Ctrl-C to quit.\n{'─'*60}")
            buf = sock.makefile("r", encoding="utf-8", errors="replace")
            for raw_line in buf:
                raw_line = raw_line.rstrip("\n")
                if not raw_line:
                    continue
                if args.filter and args.filter not in raw_line:
                    continue
                if args.raw:
                    print(raw_line)
                    continue
                try:
                    ev = json.loads(raw_line)
                    msg = fmt_event(ev)
                    if msg is not None:
                        print(msg)
                except json.JSONDecodeError:
                    print(f"{c(RED)}[bad json]{c(RESET)} {raw_line}")
            print("\nConnection closed by browser.")
        except ConnectionRefusedError:
            if args.no_retry:
                print(f"Connection refused — is the browser running with --debug-socket?")
                sys.exit(1)
            print(f"Waiting for browser on {args.host}:{args.port}…")
            time.sleep(2)
            continue
        except (BrokenPipeError, ConnectionResetError, OSError) as exc:
            print(f"\nConnection lost: {exc}")
        except KeyboardInterrupt:
            print("\nBye.")
            sys.exit(0)

        if args.no_retry:
            break
        print("Waiting for browser to restart…")
        time.sleep(2)


if __name__ == "__main__":
    main()
