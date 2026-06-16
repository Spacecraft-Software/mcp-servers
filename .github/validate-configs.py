# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
"""Validate that every JSON / JSONC / TOML / YAML config template parses cleanly.

Run from the repo root:  python3 .github/validate-configs.py

Walks the tree (skipping .git), parses each config by extension, and exits
non-zero listing every malformed file. New host templates are picked up
automatically — no list to maintain.
"""
from __future__ import annotations

import json
import os
import re
import sys
import tomllib

import yaml

SKIP_DIRS = {".git"}
EXTS = (".json", ".jsonc", ".toml", ".yaml", ".yml")


def strip_jsonc(text: str) -> str:
    """Drop /* */ blocks and whole-line // comments.

    Only *whole-line* // comments are removed, so URLs such as
    https://example inside string values are left untouched.
    """
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return "\n".join(l for l in text.splitlines() if not l.lstrip().startswith("//"))


def validate(path: str) -> str | None:
    try:
        if path.endswith(".jsonc"):
            with open(path, encoding="utf-8") as f:
                json.loads(strip_jsonc(f.read()))
        elif path.endswith(".json"):
            with open(path, encoding="utf-8") as f:
                json.load(f)
        elif path.endswith(".toml"):
            with open(path, "rb") as f:
                tomllib.load(f)
        elif path.endswith((".yaml", ".yml")):
            with open(path, encoding="utf-8") as f:
                yaml.safe_load(f)
    except Exception as exc:  # report any parse error, keep checking the rest
        return f"{path}: {type(exc).__name__}: {exc}"
    return None


def main() -> int:
    failures: list[str] = []
    checked = 0
    for root, dirs, files in os.walk("."):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for name in files:
            if not name.endswith(EXTS):
                continue
            checked += 1
            if (err := validate(os.path.join(root, name))) is not None:
                failures.append(err)

    for err in failures:
        print(f"FAIL  {err}", file=sys.stderr)
    print(f"Checked {checked} config files, {len(failures)} failed.")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
