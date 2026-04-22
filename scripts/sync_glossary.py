#!/usr/bin/env python3
"""Sync glossary snippets from doc/glossary.md into model documents.

Usage:
  python3 scripts/sync_glossary.py

The script reads the table between:
  <!-- GLOSSARY_SYNC_SOURCE:START -->
  <!-- GLOSSARY_SYNC_SOURCE:END -->

Then it updates blocks like:
  <!-- GLOSSARY_SYNC:START terms=HostInventory,HostRuntimeState -->
  ...
  <!-- GLOSSARY_SYNC:END -->
"""

from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
GLOSSARY = ROOT / "doc" / "glossary.md"
DOC_ROOT = ROOT / "doc"

SOURCE_START = "<!-- GLOSSARY_SYNC_SOURCE:START -->"
SOURCE_END = "<!-- GLOSSARY_SYNC_SOURCE:END -->"
BLOCK_START_RE = re.compile(r"<!-- GLOSSARY_SYNC:START terms=([A-Za-z0-9_,]+) -->")
BLOCK_END = "<!-- GLOSSARY_SYNC:END -->"


def load_glossary_rows() -> dict[str, dict[str, str]]:
    text = GLOSSARY.read_text(encoding="utf-8")
    start = text.index(SOURCE_START) + len(SOURCE_START)
    end = text.index(SOURCE_END)
    block = text[start:end].strip().splitlines()
    rows: dict[str, dict[str, str]] = {}
    for line in block:
        line = line.strip()
        if not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) != 4:
            continue
        if cells[0] in {"Term", "---"}:
            continue
        term = cells[0].strip("`")
        rows[term] = {
            "term_md": cells[0],
            "zh_name": cells[1],
            "en_name": cells[2],
            "zh_desc": cells[3],
        }
    return rows


def build_block(terms: list[str], rows: dict[str, dict[str, str]]) -> str:
    lines = [
        "| 术语 | 中文名 | English | 中文说明 |",
        "| --- | --- | --- | --- |",
    ]
    for term in terms:
        row = rows.get(term)
        if row is None:
            raise KeyError(f"glossary term not found: {term}")
        lines.append(
            f"| {row['term_md']} | {row['zh_name']} | {row['en_name']} | {row['zh_desc']} |"
        )
    return "\n".join(lines)


def sync_file(path: Path, rows: dict[str, dict[str, str]]) -> bool:
    original = path.read_text(encoding="utf-8")
    text = original
    cursor = 0
    changed = False

    while True:
        match = BLOCK_START_RE.search(text, cursor)
        if match is None:
            break
        block_start = match.start()
        content_start = match.end()
        block_end = text.find(BLOCK_END, content_start)
        if block_end == -1:
            raise ValueError(f"missing {BLOCK_END} in {path}")
        terms = [term for term in match.group(1).split(",") if term]
        replacement = "\n" + build_block(terms, rows) + "\n"
        text = text[:content_start] + replacement + text[block_end:]
        cursor = content_start + len(replacement) + len(BLOCK_END)
        changed = True

    if changed and text != original:
        path.write_text(text, encoding="utf-8")
    return changed and text != original


def main() -> None:
    rows = load_glossary_rows()
    updated: list[Path] = []
    for path in DOC_ROOT.rglob("*.md"):
        if path == GLOSSARY:
            continue
        if "GLOSSARY_SYNC:START" not in path.read_text(encoding="utf-8"):
            continue
        if sync_file(path, rows):
            updated.append(path)

    print(f"Glossary rows loaded: {len(rows)}")
    if updated:
        print("Updated files:")
        for path in updated:
            print(f"- {path.relative_to(ROOT)}")
    else:
        print("No files updated.")


if __name__ == "__main__":
    main()
