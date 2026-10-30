#!/usr/bin/env python3
"""Require review when a reduced sidebar-image input's source dependencies change.

The module/import closure is deliberately more conservative than the original
991-function audit: even an unrelated edit in an imported module stops reuse
of this review. There is no automatic approval or baseline-update mode.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOTS = ("src/web/view.bend", "src/web/bots.bend")
BRIDGES = (
    "src/server/thin-actor.bend",
    "src/server/thin-view.bend",
    "src/server/thin-mirror.bend",
    "src/server/thin-sidebar.bend",
    "src/core/sidebar-drafts.bend",
    "src/core/native-sidebar.bend",
    "src/mobile/screen.bend",
    "src/mobile/hubs.bend",
    "src/core/thin-wire.bend",
    "src/server/thin-body.bend",
    "src/core/thinq.bend",
    "src/server/thin-splice.bend",
    "src/server/thinstore.bend",
    "src/server/thin-signals.bend",
    "src/server/thin-signal-text.bend",
    "src/server/thin-side-index.bend",
)
MANIFEST = "scripts/sidebar-draft-inputs.json"


class GuardError(RuntimeError):
    pass


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check(root):
    root = Path(root).resolve()
    try:
        reviewed = json.loads((root / MANIFEST).read_text())
        if (reviewed.get("schema") != 1 or reviewed.get("roots") != list(ROOTS)
                or reviewed.get("bridges") != list(BRIDGES)):
            raise GuardError("review scope changed")
        scanner = root / "scripts/bend-closure.py"
        if digest(scanner) != reviewed["closureScannerSha256"]:
            raise GuardError("import-closure scanner changed")
        closure = subprocess.check_output(
            [sys.executable, str(scanner), *ROOTS], cwd=root,
            text=True, stderr=subprocess.PIPE,
        ).splitlines()
        expected = set(closure) | set(BRIDGES)
        if not closure or set(reviewed["files"]) != expected:
            raise GuardError("sidebar source/import closure changed")
        changed = []
        for name, checksum in reviewed["files"].items():
            path = (root / name).resolve()
            if not path.is_relative_to(root) or digest(path) != checksum:
                changed.append(name)
        if changed:
            raise GuardError("reviewed sidebar dependencies changed: " + ", ".join(changed))
    except (OSError, KeyError, TypeError, ValueError, subprocess.CalledProcessError) as error:
        raise GuardError("cannot verify reviewed sidebar inputs: " + str(error)) from error
    return {"state": "passed", "sourceFiles": len(expected),
            "manifestSha256": digest(root / MANIFEST)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--entry", help="Only the full server build needs this source review")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    if args.entry and Path(args.entry).resolve() != root / "src/server/main.bend":
        return
    try:
        receipt = check(root)
    except GuardError as error:
        sys.exit("sidebar-draft-guard: " + str(error)
                 + "; review draft readers and requalify full output before updating " + MANIFEST)
    # --key callers need stdout to contain only the emit key.
    print("sidebar-draft-guard: " + json.dumps(receipt, sort_keys=True), file=sys.stderr)


if __name__ == "__main__":
    main()
