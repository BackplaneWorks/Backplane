#!/usr/bin/env python3
import pathlib
import subprocess
import tempfile

root = pathlib.Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="viewer-resume-") as tmp:
    binary = pathlib.Path(tmp) / "viewer-resume"
    subprocess.run(["swiftc", "-parse-as-library", str(root / "mobile/ios/Backplane/ViewerCloseIntent.swift"),
                    str(root / "mobile/ios/Backplane/ViewerResumeIntent.swift"),
                    str(root / "test/ios-connection/viewer-resume.swift"), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)
