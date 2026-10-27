#!/usr/bin/env python3
"""Compile and exercise project search lifetimes on a Swift host."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="bp-ios-project-search-") as work:
    binary = Path(work) / "project-search"
    subprocess.run(["swiftc", "-parse-as-library", str(root / "mobile/ios/Backplane/ProjectSearchIntent.swift"),
                    str(root / "test/ios-connection/project-search.swift"), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True, timeout=10)
