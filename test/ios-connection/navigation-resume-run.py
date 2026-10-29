#!/usr/bin/env python3
"""Run Foundation-only navigation intent tests on a Mac."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="navigation-resume-") as tmp:
    binary = Path(tmp) / "test"
    subprocess.run(["swiftc", "-parse-as-library",
                    str(root / "mobile/ios/Backplane/NavigationResumeIntent.swift"),
                    str(root / "test/ios-connection/navigation-resume.swift"),
                    "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)
