#!/usr/bin/env python3
"""Exercise the actual display-readiness predicate, without an app or hub."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="bp-ios-presentation-readiness-") as work:
    binary = Path(work) / "checks"
    subprocess.run(["swiftc", "-parse-as-library",
                    str(root / "mobile/ios/Backplane/NavigationPresentationReadiness.swift"),
                    str(root / "test/ios-connection/presentation-readiness.swift"),
                    "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True, timeout=10)
