#!/usr/bin/env python3
import pathlib
import subprocess
import tempfile

root = pathlib.Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="composer-drafts-") as tmp:
    binary = pathlib.Path(tmp) / "composer-drafts"
    subprocess.run(["swiftc", "-parse-as-library", str(root / "mobile/ios/Backplane/ComposerDrafts.swift"),
                    str(root / "test/ios-connection/composer-drafts.swift"), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)
