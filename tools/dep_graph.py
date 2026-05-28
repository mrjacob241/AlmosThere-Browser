#!/usr/bin/env python3
"""
dep_graph.py — Rust codebase class/function dependency mapper for JustBarelyScript.

Every run FIRST rebuilds the graph from source, saves it to dep_graph_cache.json,
then answers the query (if any).

Usage:
  python3 tools/dep_graph.py                  # rebuild + print summary
  python3 tools/dep_graph.py --query JsValue  # rebuild + show what references JsValue
  python3 tools/dep_graph.py --dot            # rebuild + emit DOT for graphviz
  python3 tools/dep_graph.py --json           # rebuild + print full graph JSON
"""

import re
import sys
import json
import argparse
from pathlib import Path
from collections import defaultdict

CACHE_FILE = Path(__file__).parent / "dep_graph_cache.json"

# ── Regex patterns ────────────────────────────────────────────────────────────

RE_STRUCT   = re.compile(r'^(?:pub(?:\([^)]*\))?\s+)?struct\s+(\w+)', re.M)
RE_ENUM     = re.compile(r'^(?:pub(?:\([^)]*\))?\s+)?enum\s+(\w+)',   re.M)
RE_FN_SIG   = re.compile(
    r'(?m)^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)\s*[<(]'
)
RE_IMPL_HDR = re.compile(
    r'(?m)^impl(?:<[^>]*>)?\s+(?:\w+\s+for\s+)?(\w+)'
)
RE_CALL      = re.compile(r'\b([a-z_]\w{2,})\s*\(')   # lowercase fn calls
RE_TYPE_USE  = re.compile(r'\b([A-Z][A-Za-z0-9_]+)\b') # type references (CamelCase)


# ── Low-level helpers ─────────────────────────────────────────────────────────

def strip_comments(src: str) -> str:
    src = re.sub(r'/\*.*?\*/', ' ', src, flags=re.DOTALL)
    src = re.sub(r'//[^\n]*', '', src)
    return src


def find_brace_body(src: str, search_from: int) -> tuple[str, int]:
    """Return (body_without_braces, pos_after_closing_brace)."""
    depth = 0
    start = -1
    i = search_from
    while i < len(src):
        ch = src[i]
        if ch == '{':
            depth += 1
            if depth == 1:
                start = i + 1
        elif ch == '}':
            depth -= 1
            if depth == 0:
                return src[start:i], i + 1
        i += 1
    return src[start:] if start >= 0 else '', len(src)


# ── Graph builder ─────────────────────────────────────────────────────────────

class DepGraph:
    def __init__(self):
        # name → {kind, file, line, owner}
        self.nodes: dict[str, dict] = {}
        # [{from, to, kind}]
        self.edges: list[dict] = []
        # reverse index: name → {fn_id, ...}
        self.refs: dict[str, set[str]] = defaultdict(set)

    # ── Node / edge registration ──────────────────────────────────────────────

    def _node(self, name: str, kind: str, file: str, line: int, owner: str | None = None):
        if name not in self.nodes:
            self.nodes[name] = {"kind": kind, "file": file, "line": line, "owner": owner}

    def _edge(self, frm: str, to: str, kind: str):
        self.edges.append({"from": frm, "to": to, "kind": kind})
        self.refs[to].add(frm)

    # ── File scanning ─────────────────────────────────────────────────────────

    def process_file(self, path: Path):
        rel = path.as_posix()
        raw = path.read_text(errors='replace')
        src = strip_comments(raw)

        # 1. top-level struct / enum declarations
        for m in RE_STRUCT.finditer(src):
            self._node(m.group(1), 'struct', rel, raw[:m.start()].count('\n') + 1)
        for m in RE_ENUM.finditer(src):
            self._node(m.group(1), 'enum', rel, raw[:m.start()].count('\n') + 1)

        # 2. impl blocks → gather methods with their owner
        for m in RE_IMPL_HDR.finditer(src):
            owner = m.group(1)
            brace = src.find('{', m.end())
            if brace == -1:
                continue
            impl_body, _ = find_brace_body(src, brace)
            impl_offset = brace + 1
            for fm in RE_FN_SIG.finditer(impl_body):
                fn_name = fm.group(1)
                abs_offset = impl_offset + fm.start()
                line = raw[:abs_offset].count('\n') + 1
                qual = f'{owner}::{fn_name}'
                self._node(qual, 'fn', rel, line, owner=owner)
                self._edge(qual, owner, 'method_of')
                fn_brace = impl_body.find('{', fm.end() - 1)
                if fn_brace != -1:
                    body, _ = find_brace_body(impl_body, fn_brace)
                    self._analyze_body(qual, body)

        # 3. free functions (top-level, zero-indent)
        for m in RE_FN_SIG.finditer(src):
            if not m.group(0).lstrip().startswith(('fn ', 'pub fn ', 'async fn ', 'pub async')):
                continue
            fn_name = m.group(1)
            line = raw[:m.start()].count('\n') + 1
            brace = src.find('{', m.end() - 1)
            if brace == -1:
                continue
            body, _ = find_brace_body(src, brace)
            self._node(fn_name, 'fn', rel, line)
            self._analyze_body(fn_name, body)

    def _analyze_body(self, fn_id: str, body: str):
        self_name = fn_id.split('::')[-1]
        for m in RE_TYPE_USE.finditer(body):
            sym = m.group(1)
            if sym != self_name:
                self._edge(fn_id, sym, 'uses')
        for m in RE_CALL.finditer(body):
            sym = m.group(1)
            if sym != self_name and sym not in ('let', 'if', 'for', 'while', 'match', 'return'):
                self._edge(fn_id, sym, 'calls')

    # ── Persistence ───────────────────────────────────────────────────────────

    def save(self, path: Path):
        data = {
            'nodes': self.nodes,
            'edges': self.edges,
            'refs':  {k: sorted(v) for k, v in self.refs.items()},
        }
        path.write_text(json.dumps(data, indent=2))

    @classmethod
    def load(cls, path: Path) -> 'DepGraph':
        g = cls()
        data = json.loads(path.read_text())
        g.nodes = data['nodes']
        g.edges = data['edges']
        g.refs  = defaultdict(set, {k: set(v) for k, v in data['refs'].items()})
        return g

    # ── Query helpers ─────────────────────────────────────────────────────────

    def impact(self, symbol: str) -> dict:
        all_refs: set[str] = set()
        for e in self.edges:
            if e['to'] == symbol:
                all_refs.add(e['from'])
        return {
            'symbol': symbol,
            'node': self.nodes.get(symbol),
            'referenced_by': sorted(all_refs),
            'files': sorted({self.nodes[r]['file'] for r in all_refs if r in self.nodes}),
        }

    # ── Output formats ────────────────────────────────────────────────────────

    def to_json(self) -> str:
        return json.dumps({'nodes': self.nodes, 'edges': self.edges}, indent=2)

    def to_dot(self) -> str:
        seen: set[str] = set()
        for e in self.edges:
            seen.add(e['from']); seen.add(e['to'])
        lines = ['digraph dep {', '  rankdir=LR;', '  node [fontname=monospace fontsize=10];']
        colours = {'struct': '#d0e8ff', 'enum': '#ffe0d0', 'fn': '#d0ffd0'}
        for n, info in self.nodes.items():
            if n not in seen:
                continue
            c = colours.get(info['kind'], '#ffffff')
            shape = 'box' if info['kind'] in ('struct', 'enum') else 'ellipse'
            label = n.replace('::', r'\n::')
            lines.append(f'  "{n}" [label="{label}" shape={shape} style=filled fillcolor="{c}"];')
        style_map = {'uses': 'dashed', 'calls': 'solid', 'method_of': 'dotted'}
        for e in self.edges:
            s = style_map.get(e['kind'], 'solid')
            lines.append(f'  "{e["from"]}" -> "{e["to"]}" [style={s}];')
        lines.append('}')
        return '\n'.join(lines)

    def summary(self) -> str:
        kinds: dict[str, int] = defaultdict(int)
        for n in self.nodes.values():
            kinds[n['kind']] += 1
        lines = [
            f"\nJBS dependency graph — {len(self.nodes)} symbols, {len(self.edges)} edges",
            "─" * 60,
        ]
        for k in ('struct', 'enum', 'fn'):
            lines.append(f"  {k:10s}: {kinds.get(k, 0)}")
        lines.append("\nTop 20 most-referenced symbols:")
        top = sorted(self.refs.items(), key=lambda x: -len(x[1]))[:20]
        for sym, refs in top:
            kind = self.nodes.get(sym, {}).get('kind', '?')
            lines.append(f"  {sym:45s} {kind:6s} ← {len(refs):4d} refs")
        lines.append("\nRun with --query <symbol> to see full impact.\n")
        return '\n'.join(lines)


# ── Main ──────────────────────────────────────────────────────────────────────

def build_graph(src_dir: Path) -> DepGraph:
    g = DepGraph()
    for f in sorted(src_dir.rglob('*.rs')):
        g.process_file(f)
    return g


def print_impact(graph: DepGraph, symbol: str):
    info = graph.impact(symbol)
    node = info['node']
    print(f"\n{'─'*60}")
    print(f"  Symbol  : {symbol}")
    if node:
        print(f"  Kind    : {node['kind']}")
        print(f"  Defined : {node['file']} : line {node['line']}")
        if node.get('owner'):
            print(f"  Owner   : {node['owner']}")
    else:
        print("  (no top-level definition found — may be referenced without own declaration)")

    refs = info['referenced_by']
    if refs:
        print(f"\n  Referenced by {len(refs)} symbol(s):")
        for r in sorted(refs):
            rn = graph.nodes.get(r, {})
            loc = f"{rn.get('file','?')}:{rn.get('line','?')}" if rn else '?'
            owner = f"  (in {rn['owner']})" if rn.get('owner') else ''
            print(f"    {r:55s} {loc}{owner}")
    else:
        print("\n  No references found.")

    files = info['files']
    if files:
        print(f"\n  Affected files ({len(files)}):")
        for f in files:
            print(f"    {f}")
    print()


def main():
    ap = argparse.ArgumentParser(description='JBS Rust dependency graph (always rebuilds first)')
    ap.add_argument('--src', default='JustBarelyScript/src',
                    help='Source directory (default: JustBarelyScript/src)')
    ap.add_argument('--query', '-q', metavar='SYMBOL',
                    help='Show what references SYMBOL after rebuild')
    ap.add_argument('--json', action='store_true', help='Print full graph as JSON after rebuild')
    ap.add_argument('--dot',  action='store_true', help='Print DOT graph after rebuild')
    ap.add_argument('--cache', default=str(CACHE_FILE),
                    help=f'Cache file path (default: {CACHE_FILE})')
    args = ap.parse_args()

    src_dir = Path(args.src)
    cache_path = Path(args.cache)

    if not src_dir.exists():
        print(f'ERROR: source dir {src_dir} not found', file=sys.stderr)
        sys.exit(1)

    # ── Step 1: always rebuild ─────────────────────────────────────────────
    print(f"[dep_graph] scanning {src_dir} ...", file=sys.stderr)
    graph = build_graph(src_dir)
    graph.save(cache_path)
    print(f"[dep_graph] graph saved → {cache_path}  "
          f"({len(graph.nodes)} nodes, {len(graph.edges)} edges)", file=sys.stderr)

    # ── Step 2: answer query ───────────────────────────────────────────────
    if args.json:
        print(graph.to_json())
    elif args.dot:
        print(graph.to_dot())
    elif args.query:
        print_impact(graph, args.query)
    else:
        print(graph.summary())


if __name__ == '__main__':
    main()
