#!/usr/bin/env python3
"""Run the Foundation-only account-boundary policy on macOS."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="presentation-retirement-") as tmp:
    binary = Path(tmp) / "test"
    subprocess.run(["swiftc", "-parse-as-library",
                    str(root / "mobile/ios/Backplane/PresentationRetirement.swift"),
                    str(root / "test/ios-connection/presentation-retirement.swift"),
                    "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)
