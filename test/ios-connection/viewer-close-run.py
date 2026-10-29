#!/usr/bin/env python3
"""Compile and exercise local viewer close ownership on a Swift host."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="bp-ios-viewer-close-") as work:
    binary = Path(work) / "viewer-close"
    subprocess.run(["swiftc", "-parse-as-library", str(root / "mobile/ios/Backplane/ViewerCloseIntent.swift"),
                    str(root / "test/ios-connection/viewer-close.swift"), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True, timeout=10)
