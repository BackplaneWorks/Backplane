#!/usr/bin/env python3
"""Run under Windows Cygwin. No accounts, network providers or user profiles."""
from pathlib import Path
import os, socket, subprocess, sys, tempfile, threading, urllib.request

exe, runtime = (str(Path(x).resolve()) for x in sys.argv[1:])
result = subprocess.run([runtime], capture_output=True, text=True, encoding="utf-8", timeout=30, check=True)
assert "WINDOWS_RUNTIME_OK 日本😀" in result.stdout, result.stdout
assert "WINDOWS_RANDOM_OK " in result.stdout, result.stdout
with tempfile.TemporaryDirectory(prefix="backplane-windows-ci-") as directory:
    base = Path(directory)
    home, standins = base / "home", base / "standins"
    standins.mkdir()
    for provider in ("claude", "codex", "grok", "muse"):
        script = standins / provider
        script.write_text("#!/bin/sh\nexit 127\n")
        script.chmod(0o755)
    env = dict(os.environ, BACKPLANE_AGENTS=str(standins), BACKPLANE_NO_UPDATE="1",
               BACKPLANE_PEERS="", BACKPLANE_KEYRING_LOCKED="0")
    args = [exe, "--home", str(home), "--no-tailscale", "--foreground", "--threads", "1"]
    for flag, expected in (("--help", 0), ("--bogus", 2)):
        r = subprocess.run(args + [flag], env=env, capture_output=True, timeout=30)
        assert r.returncode == expected, (flag, r.returncode, r.stderr)
        assert not home.exists(), "CLI validation created the home"
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    ready, output = threading.Event(), []
    process = subprocess.Popen(args + ["--headless", "--host", "127.0.0.1", "--port", str(port)],
                               env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               text=True, encoding="utf-8")
    def read_output():
        for line in process.stdout:
            output.append(line)
            if "listening on" in line:
                ready.set()
    reader = threading.Thread(target=read_output, daemon=True)
    reader.start()
    try:
        assert ready.wait(60), "".join(output)
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/hello", timeout=15) as response:
            assert response.status == 200 and b"backplane" in response.read()
        duplicate = subprocess.run(args + ["--headless", "--port", str(port)],
                                   env=env, capture_output=True, timeout=30)
        assert duplicate.returncode == 1, duplicate.stderr
    finally:
        process.terminate()
        process.wait(timeout=30)
        reader.join(timeout=5)
print("WINDOWS_RUNTIME_RNG_CLI_HUB_AND_EXCLUSION_PASS")
