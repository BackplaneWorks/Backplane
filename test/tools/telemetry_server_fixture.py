#!/usr/bin/env python3
"""Run the existing Backplane.Works application on loopback with temporary data."""
import importlib
import json
import os
from pathlib import Path
import subprocess
import sys

from werkzeug.serving import WSGIRequestHandler, make_server

backend = Path(sys.argv[1]).resolve()
head = subprocess.check_output(["git", "-C", str(backend), "rev-parse", "HEAD"], text=True).strip()
expected = os.environ["TM_SERVER_HEAD"]
if head != expected:
    sys.exit("server source does not match pinned head")
if subprocess.check_output(["git", "-C", str(backend), "status", "--porcelain"], text=True):
    sys.exit("server fixture requires an unchanged backend checkout")
sys.path.insert(0, str(backend))
app = importlib.import_module("app").app
app.config["TELEMETRY_GITHUB_DOWNLOADS"] = False


class Quiet(WSGIRequestHandler):
    def log(self, *args, **kwargs):
        pass


server = make_server("127.0.0.1", 0, app, threaded=True, request_handler=Quiet)
print(json.dumps({"port": server.server_port, "source": head}), flush=True)
server.serve_forever()
