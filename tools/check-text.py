#!/usr/bin/env python3
"""Check first-party source hygiene without rewriting literals or vendor files."""

import pathlib
import subprocess
import sys


def main():
    names = subprocess.check_output(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"]
    ).split(b"\0")
    failed = False
    for name in names:
        if not name:
            continue
        path = pathlib.Path(name.decode())
        if path.parts[0] == "vendor" or "fonts" in path.parts or "vendor" in path.parts:
            continue
        if path.suffix in {".png", ".woff2", ".ttf", ".patch"} or not path.is_file():
            continue
        data = path.read_bytes()
        try:
            text = data.decode("utf-8")
        except UnicodeDecodeError:
            print(f"{path}: first-party text must be UTF-8", file=sys.stderr)
            failed = True
            continue
        if "\r" in text or (text and not text.endswith("\n")):
            print(f"{path}: use LF and a final newline", file=sys.stderr)
            failed = True
        for number, line in enumerate(text.splitlines(), 1):
            if "\t" in line or line.rstrip() != line:
                print(f"{path}:{number}: tab or trailing whitespace", file=sys.stderr)
                failed = True
    return int(failed)


if __name__ == "__main__":
    sys.exit(main())
