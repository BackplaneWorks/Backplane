#!/usr/bin/env python3
import pathlib
import subprocess
import tempfile

root = pathlib.Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="performance-trace-") as tmp:
    binary = pathlib.Path(tmp) / "performance-trace"
    subprocess.run(["swiftc", "-parse-as-library", str(root / "mobile/ios/Backplane/PerformanceTrace.swift"),
                    str(root / "test/ios-connection/performance-trace.swift"), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)
