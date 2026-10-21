#!/usr/bin/env python3
"""Print the newest glibc required by every ELF in a Linux package."""
import argparse
import os
from pathlib import Path
import re
import subprocess
import sys


def required_glibc(root):
    newest = (0,)
    for path in sorted(Path(root).rglob("*")):
        if not path.is_file():
            continue
        with path.open("rb") as stream:
            if stream.read(4) != b"\x7fELF":
                continue
        result = subprocess.run(["readelf", "--version-info", "--wide", str(path)],
                                capture_output=True, text=True, check=True,
                                env={**os.environ, "LC_ALL": "C"})
        # Only imported versions, not versions a bundled library provides.
        needs = False
        for line in result.stdout.splitlines():
            if line.startswith("Version "):
                needs = line.startswith("Version needs section")
            if not needs:
                continue
            for version in re.findall(r"Name: GLIBC_([\w.]+)", line):
                if not re.fullmatch(r"\d+(?:\.\d+)+", version):
                    raise ValueError(f"{path}: unsupported glibc requirement {version}")
                value = tuple(map(int, version.split(".")))
                if value > newest:
                    newest = value
                    print(f"{path}: GLIBC_{version}", file=sys.stderr)
    if newest == (0,):
        raise ValueError(f"{root}: no versioned glibc requirements found")
    return newest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root")
    parser.add_argument("--max", dest="maximum")
    args = parser.parse_args()
    try:
        required = required_glibc(args.root)
        if args.maximum and required > tuple(map(int, args.maximum.split("."))):
            raise ValueError(f"package requires glibc {'.'.join(map(str, required))}; "
                             f"supported maximum is {args.maximum}")
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"linux-abi: {error}\n")
    print(".".join(map(str, required)))


if __name__ == "__main__":
    main()
