#!/usr/bin/env python3
import pathlib
import subprocess
import sys
import tempfile

if len(sys.argv) != 2:
    raise SystemExit("usage: python3 plot-store-run.py /absolute/path/to/Plot.swift")
source = pathlib.Path(sys.argv[1]).resolve(strict=True)
test = pathlib.Path(__file__).with_name("plot-store.swift")
with tempfile.TemporaryDirectory(prefix="plot-store-test-") as tmp:
    binary = pathlib.Path(tmp) / "plot-store"
    subprocess.run(["swiftc", "-parse-as-library", str(source), str(test), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)
