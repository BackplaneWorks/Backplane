#!/usr/bin/env python3
"""Compile the real Foundation sidebar models and real Project declaration."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
source = (root / "mobile/ios/Backplane/Screen.swift").read_text()
start = source.index("struct Swipe: Decodable")
end = source.index("// the folded shelf", start)
with tempfile.TemporaryDirectory(prefix="bp-native-sidebar-") as work:
    stub = Path(work) / "ProjectModels.swift"
    stub.write_text("import Foundation\n" + source[start:end])
    binary = Path(work) / "native-sidebar"
    subprocess.run(["swiftc", "-parse-as-library", str(stub),
                    str(root / "mobile/ios/Backplane/NativeSidebar.swift"),
                    str(root / "test/ios-connection/native-sidebar.swift"), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True, timeout=10)
