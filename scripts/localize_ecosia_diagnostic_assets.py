#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import mimetypes
import re
import sys
import urllib.parse
import urllib.request
from pathlib import Path


BASE_URL = "https://www.ecosia.org/"
CSS_URL_RE = re.compile(r"url\((['\"]?)(?!data:)([^)'\"\s]+)\1\)")
ATTR_RE = re.compile(r'\b(src|poster|href)=["\']([^"\']+)["\']')
SRCSET_RE = re.compile(r'\bsrcset=["\']([^"\']+)["\']')


def absolute_url(value: str, base: str) -> str | None:
    value = value.strip()
    if not value or value.startswith(("data:", "mailto:", "tel:", "#")):
        return None
    return urllib.parse.urljoin(base, value)


def local_name(url: str, content_type: str | None = None) -> str:
    parsed = urllib.parse.urlparse(url)
    basename = Path(parsed.path).name
    suffix = Path(basename).suffix
    if not suffix and content_type:
        suffix = mimetypes.guess_extension(content_type.split(";")[0].strip()) or ""
    digest = hashlib.sha256(url.encode("utf-8")).hexdigest()[:10]
    stem = Path(basename).stem or "asset"
    safe_stem = re.sub(r"[^A-Za-z0-9._-]+", "_", stem)[:80] or "asset"
    return f"{safe_stem}.{digest}{suffix}"


def fetch(url: str, asset_dir: Path) -> str:
    req = urllib.request.Request(
        url,
        headers={
            "User-Agent": "Mozilla/5.0 AlmostThere diagnostic asset capture",
            "Accept": "*/*",
        },
    )
    with urllib.request.urlopen(req, timeout=20) as response:
        data = response.read()
        content_type = response.headers.get("content-type")
    name = local_name(url, content_type)
    path = asset_dir / name
    if not path.exists():
        path.write_bytes(data)
    return name


def localize_css(css: str, css_url: str, asset_dir: Path) -> str:
    def replace(match: re.Match[str]) -> str:
        quote, value = match.groups()
        url = absolute_url(value, css_url)
        if not url:
            return match.group(0)
        try:
            name = fetch(url, asset_dir)
        except Exception as exc:
            print(f"asset warning: failed CSS asset {url}: {exc}", file=sys.stderr)
            return match.group(0)
        return f"url({quote}{name}{quote})"

    return CSS_URL_RE.sub(replace, css)


def localize_stylesheet(url: str, asset_dir: Path) -> str:
    req = urllib.request.Request(
        url,
        headers={
            "User-Agent": "Mozilla/5.0 AlmostThere diagnostic asset capture",
            "Accept": "text/css,*/*",
        },
    )
    with urllib.request.urlopen(req, timeout=20) as response:
        css = response.read().decode(response.headers.get_content_charset() or "utf-8", "replace")
    css = localize_css(css, url, asset_dir)
    name = local_name(url, "text/css")
    (asset_dir / name).write_text(css, encoding="utf-8")
    return name


def localize_srcset(value: str, html_base: str, asset_dir: Path) -> str:
    parts: list[str] = []
    for candidate in value.split(","):
        candidate = candidate.strip()
        if not candidate:
            continue
        fields = candidate.split()
        url = absolute_url(fields[0], html_base)
        if not url:
            parts.append(candidate)
            continue
        try:
            fields[0] = fetch(url, asset_dir)
        except Exception as exc:
            print(f"asset warning: failed srcset asset {url}: {exc}", file=sys.stderr)
        parts.append(" ".join(fields))
    return ", ".join(parts)


def localize_html(path: Path, asset_dir: Path) -> None:
    html = path.read_text(encoding="utf-8")

    def replace_srcset(match: re.Match[str]) -> str:
        return f'srcset="{localize_srcset(match.group(1), BASE_URL, asset_dir)}"'

    html = SRCSET_RE.sub(replace_srcset, html)

    def replace_attr(match: re.Match[str]) -> str:
        attr, value = match.groups()
        url = absolute_url(value, BASE_URL)
        if not url:
            return match.group(0)
        parsed = urllib.parse.urlparse(url)
        is_stylesheet = attr == "href" and ".css" in parsed.path
        is_asset_src = attr in {"src", "poster"}
        if not (is_stylesheet or is_asset_src):
            return match.group(0)
        try:
            name = localize_stylesheet(url, asset_dir) if is_stylesheet else fetch(url, asset_dir)
        except Exception as exc:
            print(f"asset warning: failed {attr} asset {url}: {exc}", file=sys.stderr)
            return match.group(0)
        return f'{attr}="{asset_dir.name}/{name}"'

    html = ATTR_RE.sub(replace_attr, html)
    path.write_text(html, encoding="utf-8")


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: localize_ecosia_diagnostic_assets.py ASSET_DIR HTML...", file=sys.stderr)
        return 2
    asset_dir = Path(sys.argv[1])
    asset_dir.mkdir(parents=True, exist_ok=True)
    for html_path in sys.argv[2:]:
        localize_html(Path(html_path), asset_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
