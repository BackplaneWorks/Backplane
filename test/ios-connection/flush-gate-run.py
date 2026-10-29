#!/usr/bin/env python3
"""Compile and exercise the actual Foundation flush gate; no app or hub needed."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="bp-ios-flush-gate-") as work:
    binary = Path(work) / "flush-gate"
    subprocess.run(["swiftc", "-parse-as-library", str(root / "mobile/ios/Backplane/PairingFlushGate.swift"),
                    str(root / "test/ios-connection/flush-gate.swift"), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True, timeout=10)
