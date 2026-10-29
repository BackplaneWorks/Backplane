#!/usr/bin/env python3
"""Compile the actual Foundation-free presentation intent on a Swift host."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="bp-ios-presentation-intent-") as work:
    binary = Path(work) / "intent"
    subprocess.run(["swiftc", "-parse-as-library", str(root / "mobile/ios/Backplane/PresentationIntent.swift"),
                    str(root / "test/ios-connection/presentation-intent.swift"), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True, timeout=10)
