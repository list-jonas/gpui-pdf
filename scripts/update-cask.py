#!/usr/bin/env python3
"""Rewrite Casks/gpui-pdf.rb with a released version and its tarball hashes."""

from __future__ import annotations

import pathlib
import re
import sys

CASK = pathlib.Path(__file__).resolve().parent.parent / "Casks" / "gpui-pdf.rb"
SHA_PATTERN = re.compile(r"\A[0-9a-f]{64}\Z")


def main(argv: list[str]) -> int:
    if len(argv) != 4:
        print("usage: update-cask.py VERSION ARM_SHA256 INTEL_SHA256", file=sys.stderr)
        return 2

    version, arm_sha, intel_sha = argv[1], argv[2].lower(), argv[3].lower()
    for sha in (arm_sha, intel_sha):
        if not SHA_PATTERN.match(sha):
            print(f"not a sha256 digest: {sha}", file=sys.stderr)
            return 1

    text = CASK.read_text()
    text, count = re.subn(r'version "[^"]*"', f'version "{version}"', text, count=1)
    if count != 1:
        print("no version stanza in cask", file=sys.stderr)
        return 1

    text, count = re.subn(
        r'sha256 arm:\s+"[0-9a-fA-F]{64}",\s*\n\s*intel: "[0-9a-fA-F]{64}"',
        f'sha256 arm:   "{arm_sha}",\n         intel: "{intel_sha}"',
        text,
        count=1,
    )
    if count != 1:
        print("no sha256 stanza in cask", file=sys.stderr)
        return 1

    CASK.write_text(text)
    print(f"cask set to {version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
