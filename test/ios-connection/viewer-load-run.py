#!/usr/bin/env python3
import pathlib
import subprocess
import tempfile

root = pathlib.Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="viewer-load-") as tmp:
    binary = pathlib.Path(tmp) / "viewer-load"
    subprocess.run(["swiftc", "-parse-as-library", str(root / "mobile/ios/Backplane/ViewerLoadState.swift"),
                    str(root / "test/ios-connection/viewer-load.swift"), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)
