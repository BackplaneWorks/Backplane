#!/usr/bin/env python3
"""Seal or verify a finished portable macOS bundle; never launches its code.

Ad-hoc integrity is not Developer ID signing or notarization. Sign nested
Electron code inside out, preserving entitlements, and leave the immutable
service component alone (its Mach-O signatures must already verify).
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys


MACHO = {bytes.fromhex(h) for h in (
    "feedface", "cefaedfe", "feedfacf", "cffaedfe", "cafebabe", "bebafeca",
    "cafebabf", "bfbafeca")}


def run(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True,
                          timeout=120).stdout


def contents(app):
    binaries, bundles = [], [app]
    for folder, dirs, files in os.walk(app, followlinks=False):
        root = Path(folder)
        dirs[:] = sorted(d for d in dirs if not (root / d).is_symlink())
        for name in dirs:
            p = root / name
            if p.suffix in (".app", ".framework", ".xpc"):
                bundles.append(p)
        for name in sorted(files):
            p = root / name
            if p.is_symlink():
                continue
            with p.open("rb") as f:
                if f.read(4) in MACHO:
                    binaries.append(p)
    return binaries, sorted(bundles, key=lambda p: (-len(p.parts), str(p)))


def bundle(app, seal=False):
    if sys.platform != "darwin":
        raise ValueError("macOS codesign verification requires a native Mac")
    app = Path(app).absolute()
    if app.is_symlink() or not (app / "Contents/Info.plist").is_file():
        raise ValueError("Expected a real .app with Contents/Info.plist")
    binaries, bundles = contents(app)
    if not binaries:
        raise ValueError("Bundle has no Mach-O code")
    if seal:
        # Build inputs may have been downloaded by a browser. Only this explicit
        # assembled bundle is affected; no machine-wide Gatekeeper settings.
        run("/usr/bin/xattr", "-dr", "com.apple.quarantine", str(app))
        for p in binaries:
            if p.is_relative_to(app / "Contents/Resources"):
                run("/usr/bin/codesign", "--verify", "--strict", str(p))
            else:
                run("/usr/bin/codesign", "--force", "--sign", "-",
                    "--options=0", "--timestamp=none",
                    "--preserve-metadata=entitlements", str(p))
        for p in bundles:
            run("/usr/bin/codesign", "--force", "--sign", "-",
                "--options=0", "--timestamp=none",
                "--preserve-metadata=entitlements", str(p))
    # --deep alone does not discover arbitrary executables in Resources.
    for p in binaries:
        run("/usr/bin/codesign", "--verify", "--strict", str(p))
    for p in bundles:
        run("/usr/bin/codesign", "--verify", "--deep", "--strict", str(p))
    return {"schema": 1, "status": "PASS", "kind": "macos-bundle-integrity",
            "sealed": True, "nestedCodeVerified": True,
            "machOFiles": len(binaries), "bundles": len(bundles),
            "notarized": False, "scope": "Code integrity only; no Gatekeeper/notarization acceptance claim"}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("seal", "verify"))
    parser.add_argument("app")
    args = parser.parse_args()
    try:
        print(json.dumps(bundle(args.app, args.operation == "seal")))
    except (ValueError, OSError, subprocess.SubprocessError) as e:
        print("macOS bundle: " + str(e), file=sys.stderr)
        if isinstance(e, subprocess.CalledProcessError):
            print(e.stderr, file=sys.stderr)
        sys.exit(1)
