#!/usr/bin/env python3
"""Compile the real Out decoder and check local navigation admission metadata."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="bp-ios-navigation-admission-decode-") as work:
    binary = Path(work) / "checks"
    subprocess.run(["swiftc", "-parse-as-library",
                    str(root / "mobile/ios/Backplane/Screen.swift"),
                    str(root / "mobile/ios/Backplane/WireDecodeDiagnostic.swift"),
                    str(root / "test/ios-connection/navigation-admission-decode.swift"),
                    "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True, timeout=10)
