#!/usr/bin/env python3
"""Exercise actual stale-navigation command ownership without an app or hub."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="bp-ios-navigation-command-policy-") as work:
    binary = Path(work) / "checks"
    subprocess.run(["swiftc", "-parse-as-library",
                    str(root / "mobile/ios/Backplane/NavigationCommandPolicy.swift"),
                    str(root / "test/ios-connection/navigation-command-policy.swift"),
                    "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True, timeout=10)
