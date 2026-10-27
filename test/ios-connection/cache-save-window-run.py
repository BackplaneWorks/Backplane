#!/usr/bin/env python3
"""Run actual Foundation scheduler and cache against temporary files on macOS."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
src = root / "mobile/ios/Backplane"
with tempfile.TemporaryDirectory(prefix="cache-save-window-") as tmp:
    # Compile the actual standalone Pairing declaration without unrelated Hub IO.
    pairing = Path(tmp) / "Pairing.swift"
    hub = (src / "Hub.swift").read_text()
    pairing.write_text(hub.split("// Only typed, secret-free causes", 1)[0])
    binary = Path(tmp) / "test"
    subprocess.run(["swiftc", "-parse-as-library", str(src / "CacheSaveWindow.swift"),
                    str(src / "ScreenCache.swift"), str(pairing),
                    str(root / "test/ios-connection/cache-save-window.swift"),
                    "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)
