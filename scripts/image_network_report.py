#!/usr/bin/env python3
"""
Build an HTML/SVG report for image and asset loading events in an AlmostThere
telemetry session JSONL file.

Example:
    python3 scripts/image_network_report.py \
        appdata/telemetry/sessions/1780828203655_151465/session.jsonl \
        --page wikipedia \
        --out target/image_network_report.html
"""

from __future__ import annotations

import argparse
import collections
import html
import json
import pathlib
import sys
import urllib.parse
from dataclasses import dataclass, field


IMAGE_EVENTS = (
    "document.image.",
)

NETWORK_EVENT_SUFFIXES = (
    ".queued",
    ".started",
    ".response",
    ".completed",
    ".failed",
    ".backoff",
    ".requeued",
    ".delayed",
    ".applied",
    ".ignored",
    ".progress",
)

EVENT_COLORS = {
    "document.image.scheduler.queued": "#64748b",
    "document.image.scheduler.started": "#2563eb",
    "document.image.resource.fetch.started": "#0ea5e9",
    "document.image.resource.fetch.response": "#a855f7",
    "document.image.resource.fetch.completed": "#16a34a",
    "document.image.scheduler.completed": "#059669",
    "document.image.late.applied": "#22c55e",
    "document.image.resource.fetch.failed": "#dc2626",
    "document.image.scheduler.backoff": "#f97316",
    "document.image.scheduler.requeued": "#eab308",
    "document.image.scheduler.delayed": "#f59e0b",
    "document.image.late.progress": "#475569",
    "document.image.late.failed": "#b91c1c",
    "document.image.late.ignored": "#9333ea",
}


@dataclass
class Point:
    ts: int
    line: int
    event: str
    status: str = ""
    reason: str = ""
    detail: str = ""


@dataclass
class Resource:
    url: str
    origin: str
    points: list[Point] = field(default_factory=list)
    statuses: collections.Counter[str] = field(default_factory=collections.Counter)
    reasons: collections.Counter[str] = field(default_factory=collections.Counter)

    @property
    def first_ts(self) -> int:
        return min((point.ts for point in self.points), default=0)

    @property
    def first_request_ts(self) -> int | None:
        return min(
            (
                point.ts
                for point in self.points
                if point.event == "document.image.resource.fetch.started"
            ),
            default=None,
        )

    @property
    def sort_ts(self) -> int:
        first_request = self.first_request_ts
        if first_request is not None:
            return first_request
        return self.first_ts

    @property
    def last_ts(self) -> int:
        return max((point.ts for point in self.points), default=0)

    @property
    def failed(self) -> bool:
        return any(point.event.endswith(".failed") for point in self.points)

    @property
    def retrying(self) -> bool:
        return any(point.event.endswith(".requeued") for point in self.points)

    @property
    def applied(self) -> bool:
        return any(point.event == "document.image.late.applied" for point in self.points)


def event_ts(event: dict, fallback: int) -> int:
    for key in ("timestamp_ms", "ts", "time_ms", "time"):
        value = event.get(key)
        if value is None:
            continue
        try:
            return int(float(value))
        except (TypeError, ValueError):
            pass
    return fallback


def event_url(event: dict) -> str:
    for key in ("url", "src", "resolved_url", "image_url"):
        value = event.get(key)
        if value:
            return str(value)
    return ""


def event_origin(event: dict, url: str) -> str:
    if event.get("origin"):
        return str(event["origin"])
    try:
        parsed = urllib.parse.urlparse(url)
    except ValueError:
        return ""
    if parsed.scheme and parsed.netloc:
        return f"{parsed.scheme}://{parsed.netloc}"
    return ""


def include_event(event: dict, page_filter: str) -> bool:
    name = str(event.get("event", ""))
    if not name.startswith(IMAGE_EVENTS):
        return False
    if not page_filter:
        return True
    needle = page_filter.lower()
    haystack = " ".join(
        str(event.get(key, ""))
        for key in ("document", "url", "src", "resolved_url", "origin")
    ).lower()
    return needle in haystack


def is_download_timeline_event(name: str) -> bool:
    if name == "document.image.late.progress":
        return True
    return any(name.endswith(suffix) for suffix in NETWORK_EVENT_SUFFIXES)


def read_resources(path: pathlib.Path, page_filter: str) -> tuple[list[Resource], list[dict]]:
    resources: dict[str, Resource] = {}
    raw_events: list[dict] = []
    fallback_ts = 0
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        for line_no, line in enumerate(handle, 1):
            line = line.strip()
            if not line:
                continue
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if not include_event(event, page_filter):
                continue
            raw_events.append(event)
            name = str(event.get("event", ""))
            if not is_download_timeline_event(name):
                continue
            url = event_url(event)
            if not url:
                continue
            fallback_ts += 1
            ts = event_ts(event, fallback_ts)
            origin = event_origin(event, url)
            resource = resources.setdefault(url, Resource(url=url, origin=origin))
            status = str(event.get("status", ""))
            reason = str(event.get("reason", ""))
            detail = status or reason or str(event.get("error", ""))[:120]
            resource.points.append(
                Point(
                    ts=ts,
                    line=line_no,
                    event=name,
                    status=status,
                    reason=reason,
                    detail=detail,
                )
            )
            if status:
                resource.statuses[status] += 1
            if reason:
                resource.reasons[reason] += 1
    return sorted(
        resources.values(),
        key=lambda item: (
            item.first_request_ts is None,
            item.sort_ts,
            item.first_ts,
            item.url,
        ),
    ), raw_events


def short_url(url: str, max_len: int = 92) -> str:
    if len(url) <= max_len:
        return url
    parsed = urllib.parse.urlparse(url)
    tail = pathlib.PurePosixPath(parsed.path).name or url[-max_len // 2 :]
    prefix = f"{parsed.netloc}/" if parsed.netloc else ""
    shortened = f"{prefix}.../{tail}"
    if len(shortened) <= max_len:
        return shortened
    return shortened[: max_len - 1] + "…"


def classify(resource: Resource) -> str:
    if resource.applied:
        return "applied"
    if resource.retrying:
        return "retrying"
    if resource.failed:
        return "failed"
    return "pending"


def render_report(
    resources: list[Resource],
    raw_events: list[dict],
    source_path: pathlib.Path,
    page_filter: str,
) -> str:
    min_ts = min((resource.first_ts for resource in resources), default=0)
    max_ts = max((resource.last_ts for resource in resources), default=min_ts + 1)
    span = max(1, max_ts - min_ts)
    width = 1500
    left = 420
    right = 40
    row_h = 30
    top = 72
    plot_w = width - left - right
    height = top + max(1, len(resources)) * row_h + 80

    event_counts = collections.Counter(str(event.get("event", "")) for event in raw_events)
    status_counts = collections.Counter()
    reason_counts = collections.Counter()
    origin_counts = collections.Counter()
    state_counts = collections.Counter()
    for resource in resources:
        status_counts.update(resource.statuses)
        reason_counts.update(resource.reasons)
        origin_counts[resource.origin or "(unknown)"] += 1
        state_counts[classify(resource)] += 1

    def x_for(ts: int) -> float:
        return left + ((ts - min_ts) / span) * plot_w

    rows = []
    for idx, resource in enumerate(resources):
        y = top + idx * row_h
        state = classify(resource)
        bg = {
            "applied": "#ecfdf5",
            "retrying": "#fffbeb",
            "failed": "#fef2f2",
            "pending": "#f8fafc",
        }[state]
        rows.append(
            f'<rect x="0" y="{y - 18}" width="{width}" height="{row_h}" fill="{bg}"/>'
        )
        rows.append(
            f'<text x="14" y="{y}" class="url" title="{html.escape(resource.url)}">'
            f"{html.escape(short_url(resource.url))}</text>"
        )
        rows.append(
            f'<text x="{left - 76}" y="{y}" class="state state-{state}">{state}</text>'
        )
        rows.append(
            f'<line x1="{left}" y1="{y - 5}" x2="{left + plot_w}" y2="{y - 5}" '
            f'stroke="#e2e8f0" stroke-width="1"/>'
        )
        points = sorted(resource.points, key=lambda item: (item.ts, item.line))
        if points:
            rows.append(
                f'<line x1="{x_for(points[0].ts):.1f}" y1="{y - 5}" '
                f'x2="{x_for(points[-1].ts):.1f}" y2="{y - 5}" '
                f'stroke="#94a3b8" stroke-width="2"/>'
            )
        for point in points:
            x = x_for(point.ts)
            color = EVENT_COLORS.get(point.event, "#334155")
            label = (
                f"line {point.line}\\n{point.event}\\n"
                f"status={point.status or '-'} reason={point.reason or '-'}\\n"
                f"t=+{point.ts - min_ts}ms"
            )
            shape = "circle"
            if point.event.endswith(".failed"):
                shape = "cross"
            elif point.event.endswith(".requeued") or point.event.endswith(".backoff"):
                shape = "diamond"
            elif point.event.endswith(".response"):
                shape = "square"
            if shape == "circle":
                rows.append(
                    f'<circle cx="{x:.1f}" cy="{y - 5}" r="5" fill="{color}">'
                    f"<title>{html.escape(label)}</title></circle>"
                )
            elif shape == "square":
                rows.append(
                    f'<rect x="{x - 5:.1f}" y="{y - 10}" width="10" height="10" fill="{color}">'
                    f"<title>{html.escape(label)}</title></rect>"
                )
            elif shape == "diamond":
                rows.append(
                    f'<path d="M{x:.1f},{y - 13} L{x + 7:.1f},{y - 5} '
                    f'L{x:.1f},{y + 3} L{x - 7:.1f},{y - 5} Z" fill="{color}">'
                    f"<title>{html.escape(label)}</title></path>"
                )
            else:
                rows.append(
                    f'<line x1="{x - 6:.1f}" y1="{y - 11}" x2="{x + 6:.1f}" y2="{y + 1}" '
                    f'stroke="{color}" stroke-width="3"><title>{html.escape(label)}</title></line>'
                )
                rows.append(
                    f'<line x1="{x + 6:.1f}" y1="{y - 11}" x2="{x - 6:.1f}" y2="{y + 1}" '
                    f'stroke="{color}" stroke-width="3"><title>{html.escape(label)}</title></line>'
                )

    summary = "\n".join(
        [
            render_counter("Final states", state_counts),
            render_counter("HTTP/status values", status_counts),
            render_counter("Delay/backoff reasons", reason_counts),
            render_counter("Origins", origin_counts),
            render_counter("Image event counts", event_counts),
        ]
    )

    legend_items = [
        ("queued", EVENT_COLORS["document.image.scheduler.queued"]),
        ("started", EVENT_COLORS["document.image.resource.fetch.started"]),
        ("response", EVENT_COLORS["document.image.resource.fetch.response"]),
        ("completed/applied", EVENT_COLORS["document.image.late.applied"]),
        ("failed", EVENT_COLORS["document.image.resource.fetch.failed"]),
        ("backoff/requeued", EVENT_COLORS["document.image.scheduler.backoff"]),
        ("delayed", EVENT_COLORS["document.image.scheduler.delayed"]),
    ]
    legend = " ".join(
        f'<span class="legend"><span style="background:{color}"></span>{html.escape(name)}</span>'
        for name, color in legend_items
    )

    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>AlmostThere Image Network Report</title>
<style>
body {{ margin: 24px; font-family: system-ui, sans-serif; color: #111827; background: #f8fafc; }}
h1 {{ margin: 0 0 8px; font-size: 24px; }}
.meta {{ color: #475569; margin-bottom: 16px; }}
.panel {{ background: white; border: 1px solid #dbe3ef; border-radius: 8px; padding: 16px; margin: 16px 0; }}
.summary {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 12px; }}
table {{ width: 100%; border-collapse: collapse; font-size: 13px; }}
td, th {{ border-bottom: 1px solid #e5e7eb; padding: 4px 6px; text-align: left; }}
.legend {{ display: inline-flex; gap: 6px; align-items: center; margin-right: 16px; font-size: 13px; }}
.legend span {{ width: 11px; height: 11px; display: inline-block; border-radius: 2px; }}
svg {{ background: white; border: 1px solid #dbe3ef; border-radius: 8px; max-width: 100%; height: auto; }}
.url {{ font: 12px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; fill: #0f172a; }}
.axis {{ font: 12px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; fill: #475569; }}
.state {{ font: 12px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-weight: 700; }}
.state-applied {{ fill: #15803d; }}
.state-retrying {{ fill: #a16207; }}
.state-failed {{ fill: #b91c1c; }}
.state-pending {{ fill: #475569; }}
</style>
</head>
<body>
<h1>AlmostThere Image Network Report</h1>
<div class="meta">
Source: <code>{html.escape(str(source_path))}</code><br>
Filter: <code>{html.escape(page_filter or "(none)")}</code><br>
Resources: {len(resources)} | Image events: {len(raw_events)} | Window: {span} ms
</div>
<div class="panel">{legend}</div>
<div class="summary">{summary}</div>
<svg viewBox="0 0 {width} {height}" role="img" aria-label="Image loading timeline">
<text x="{left}" y="24" class="axis">time +0ms</text>
<text x="{left + plot_w - 100}" y="24" class="axis">+{span}ms</text>
<line x1="{left}" y1="42" x2="{left + plot_w}" y2="42" stroke="#94a3b8" stroke-width="2"/>
{''.join(rows)}
</svg>
</body>
</html>
"""


def render_counter(title: str, counter: collections.Counter[str]) -> str:
    if not counter:
        body = "<tr><td colspan='2'>(none)</td></tr>"
    else:
        body = "\n".join(
            f"<tr><td>{html.escape(str(key))}</td><td>{value}</td></tr>"
            for key, value in counter.most_common(16)
        )
    return f"<div class='panel'><h2>{html.escape(title)}</h2><table>{body}</table></div>"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate a graphical HTML/SVG image-network report from AlmostThere JSONL."
    )
    parser.add_argument("jsonl", type=pathlib.Path, help="Path to session.jsonl")
    parser.add_argument(
        "--page",
        default="",
        help="Case-insensitive substring matched against document/url/origin fields.",
    )
    parser.add_argument(
        "--out",
        type=pathlib.Path,
        default=pathlib.Path("target/image_network_report.html"),
        help="Output HTML path.",
    )
    args = parser.parse_args()

    resources, raw_events = read_resources(args.jsonl, args.page)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(
        render_report(resources, raw_events, args.jsonl, args.page),
        encoding="utf-8",
    )
    print(f"wrote {args.out}")
    print(f"resources={len(resources)} image_events={len(raw_events)}")
    if not resources:
        print("warning: no matching image resources found", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
