#!/usr/bin/env python3
"""
sync_glossary.py

Read canonical term definitions from doc/glossary.md (GLOSSARY_SYNC_SOURCE block)
and sync them into all model documents that have GLOSSARY_SYNC markers.

Usage:
    python3 scripts/sync_glossary.py

The script is idempotent: running it multiple times produces the same result
as long as the glossary source table hasn't changed.
"""

import glob
import os
import re
import sys
from typing import Dict

START_MARKER = "<!-- GLOSSARY_SYNC:START"
END_MARKER = "<!-- GLOSSARY_SYNC:END -->"
SOURCE_MARKER = "<!-- GLOSSARY_SYNC_SOURCE:START -->"
SOURCE_END_MARKER = "<!-- GLOSSARY_SYNC_SOURCE:END -->"

DOC_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "doc")


def parse_glossary_source() -> Dict[str, str]:
    """Parse glossary.md GLOSSARY_SYNC_SOURCE block into {term: row_string} dict."""
    glossary_path = os.path.join(DOC_DIR, "glossary.md")
    if not os.path.exists(glossary_path):
        print(f"Error: glossary.md not found at {glossary_path}")
        sys.exit(1)

    with open(glossary_path, "r", encoding="utf-8") as f:
        content = f.read()

    # Extract source block
    pattern = re.escape(SOURCE_MARKER) + r"(.*?)" + re.escape(SOURCE_END_MARKER)
    match = re.search(pattern, content, re.DOTALL)
    if not match:
        print("Error: GLOSSARY_SYNC_SOURCE block not found in glossary.md")
        sys.exit(1)

    block = match.group(1).strip()
    lines = block.split("\n")

    terms = {}
    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith("| -") or stripped.startswith("| T"):
            continue
        # Parse row: | `Term` | 中文名 | English | 中文说明 |
        cols = [c.strip() for c in stripped.strip("|").split("|")]
        if len(cols) >= 4:
            term_key = cols[0].strip("`")
            terms[term_key] = stripped

    print(f"Parsed {len(terms)} terms from glossary source")
    return terms


def sync_file(filepath: str, glossary: Dict[str, str]) -> bool:
    """Sync GLOSSARY_SYNC blocks in a single file. Returns True if changed."""
    with open(filepath, "r", encoding="utf-8") as f:
        content = f.read()

    changed = False
    result = content

    # Find all GLOSSARY_SYNC blocks
    block_pattern = re.compile(
        r"(" + re.escape(START_MARKER) + r"\s+terms=([^>]+)\s*-->\s*\n)"
        r"(.*?)"
        r"(\s*" + re.escape(END_MARKER) + r")",
        re.DOTALL,
    )

    def replace_block(match):
        nonlocal changed
        prefix = match.group(1)
        terms_str = match.group(2)
        suffix = match.group(4)

        requested_terms = [t.strip() for t in terms_str.split(",")]

        # Build synced rows
        synced_rows = []
        for term in requested_terms:
            if term in glossary:
                synced_rows.append(glossary[term])
            else:
                print(f"  Warning: term '{term}' not found in glossary source")

        # Reconstruct block with header from original
        old_block = match.group(3)
        old_lines = old_block.strip().split("\n")
        header_row = old_lines[0] if old_lines else "| 术语 | 中文名 | English | 中文说明 |"

        new_body = header_row + "\n" + "\n".join(synced_rows)
        new_block = prefix + new_body + suffix

        if match.group(0) != new_block:
            changed = True

        return new_block

    result = block_pattern.sub(replace_block, result)

    if changed:
        with open(filepath, "w", encoding="utf-8") as f:
            f.write(result)
        return True
    return False


def main():
    glossary = parse_glossary_source()

    md_files = glob.glob(os.path.join(DOC_DIR, "**/*.md"), recursive=True)
    # Exclude glossary.md itself
    md_files = [f for f in md_files if os.path.basename(f) != "glossary.md"]

    updated = 0
    for filepath in sorted(md_files):
        relpath = os.path.relpath(filepath, DOC_DIR)
        if sync_file(filepath, glossary):
            print(f"  Updated: doc/{relpath}")
            updated += 1

    if updated == 0:
        print("All GLOSSARY_SYNC blocks are up to date.")
    else:
        print(f"\nSynced {updated} file(s).")


if __name__ == "__main__":
    main()
