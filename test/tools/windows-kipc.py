#!/usr/bin/env python3
"""Exercise a compiled Windows helper's bundled Worker and missing-DLL reply.

Run with Cygwin Python. This is a transport failure test, not KiCad acceptance.
"""
import json
from pathlib import Path
import select
import subprocess
import sys
import tempfile

if sys.platform != "cygwin":
    raise SystemExit("Run with Windows Cygwin Python")
exe = str(Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix="bp-windows-kipc-") as directory:
    with subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, text=True, encoding="utf-8") as child:
        try:
            message = {"id": "worker", "sock": directory + "/missing.sock",
                       "lib": directory + "/no-runtime",
                       "type": "kiapi.common.commands.Ping", "message": {}, "timeout": 1000}
            child.stdin.write(json.dumps(message) + "\n")
            child.stdin.flush()
            readable, _, _ = select.select([child.stdout], [], [], 15)
            assert readable, "Bundled Windows Worker did not reply"
            line = child.stdout.readline()
            assert line, "Helper exited before its Worker replied"
            reply = json.loads(line)
            assert reply["id"] == "worker" and reply["ok"] is False, reply
            assert "libnng not found" in reply.get("error", ""), reply
            child.stdin.close()
            assert child.wait(timeout=10) == 0
            print("WINDOWS_KIPC_COMPILED_WORKER_AND_ERROR_REPLY_PASS")
        finally:
            if child.poll() is None:
                child.terminate()
                child.wait(timeout=10)
            error = child.stderr.read()
            if error: print(error, file=sys.stderr)
