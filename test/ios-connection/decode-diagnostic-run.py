#!/usr/bin/env python3
"""Exercise the actual Foundation decoder diagnostic without app or live data."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="bp-ios-decode-diagnostic-") as work:
    binary = Path(work) / "decode-diagnostic"
    subprocess.run(["swiftc", "-parse-as-library", str(root / "mobile/ios/Backplane/WireDecodeDiagnostic.swift"),
                    str(root / "test/ios-connection/decode-diagnostic.swift"), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True, timeout=10)
