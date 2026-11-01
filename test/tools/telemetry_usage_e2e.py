#!/usr/bin/env python3
"""Actual signed alpha worker -> pinned receiver -> temporary SQLite.

Run with uv in the backend's environment. Uses generated development keys,
synthetic projection events and loopback HTTP only; never production data.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import hmac
import importlib
import io
import json
import os
from pathlib import Path
import secrets
import sqlite3
import subprocess
import sys
import tempfile
import threading
import urllib.request

from flask import Flask
from werkzeug.serving import WSGIRequestHandler, make_server
from werkzeug.wrappers import Response


def check(ok, name):
    if not ok:
        raise AssertionError(name)
    print("ok " + name, flush=True)


class Quiet(WSGIRequestHandler):
    def log(self, *args, **kwargs):
        pass


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--worker", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    backend = args.backend.resolve()
    revision = subprocess.check_output(["git", "-C", str(backend), "rev-parse", "HEAD"], text=True).strip()
    check(revision == args.revision, "backend checkout is pinned")
    check(not subprocess.check_output(["git", "-C", str(backend), "status", "--porcelain"], text=True), "pinned backend has no uncommitted changes")
    sys.path.insert(0, str(backend))
    telemetry = importlib.import_module("telemetry")
    worker = args.worker.resolve()
    with tempfile.TemporaryDirectory(prefix="bp-alpha-telemetry-") as temp:
        root = Path(temp)
        key, admin = secrets.token_hex(32), secrets.token_hex(32)
        database = root / "telemetry.sqlite3"
        app = Flask(__name__, template_folder=str(backend / "templates"))
        app.config.update(TESTING=True, SECRET_KEY=secrets.token_hex(32),
            TELEMETRY_DATABASE=str(database), TELEMETRY_GITHUB_DOWNLOADS=False,
            TELEMETRY_KEYS={"dev-test": {"secret": key, "channels": ["dev"], "versions": ["*"]}},
            TELEMETRY_ADMIN_TOKEN=admin, TELEMETRY_TRUST_PROXY=False)
        telemetry.init_app(app)
        requests = []
        mode = None

        def gateway(environ, start_response):
            raw = environ["wsgi.input"].read(int(environ.get("CONTENT_LENGTH", 0)))
            environ["wsgi.input"] = io.BytesIO(raw)
            body = json.loads(raw)
            signature = "sha256=" + hmac.new(bytes.fromhex(key), str(body["ts"]).encode() + b"." + raw, hashlib.sha256).hexdigest()
            check(hmac.compare_digest(signature, environ["HTTP_X_BACKPLANE_SIGNATURE"]), "real worker HMAC matches exact body")
            check(environ["PATH_INFO"] == "/api/v1/events" and environ["REQUEST_METHOD"] == "POST", "canonical loopback ingest request")
            check(0 < len(raw) <= 16384 and 0 < len(body["events"]) <= 50, "wire request stays bounded")
            check(b"private-" not in raw and b"/private/" not in raw, "no synthetic content, identifiers, paths or provider names on wire")
            requests.append(body)
            is_usage = any(event["e"] == "usage" for event in body["events"])
            if is_usage and mode == "ambiguous":
                # The server commits, but the client sees a transient proxy error.
                with app.test_client() as client:
                    result = client.post("/api/v1/events", data=raw, headers={
                        "Content-Type": "application/json", "X-Backplane-Key": "dev-test",
                        "X-Backplane-Timestamp": str(body["ts"]), "X-Backplane-Signature": signature})
                    check(result.status_code == 202, "ambiguous response follows a real server commit")
                return Response("{}", status=502)(environ, start_response)
            if is_usage and isinstance(mode, int):
                return Response("{}", status=mode, headers={"Retry-After": "7200"})(environ, start_response)
            return app(environ, start_response)

        server = make_server("127.0.0.1", 0, gateway, threaded=True, request_handler=Quiet)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        endpoint = f"http://127.0.0.1:{server.server_port}/api/v1/events"
        env = {key: value for key, value in os.environ.items() if not key.startswith("BACKPLANE_TELEMETRY")}
        env.update(HOME=str(root), CURL_HOME=str(root), CI="", DO_NOT_TRACK="", BACKPLANE_SANDBOX="",
            BACKPLANE_TELEMETRY="on", BACKPLANE_TELEMETRY_TEST="1", BACKPLANE_TELEMETRY_CHANNEL="dev",
            BACKPLANE_TELEMETRY_URL=endpoint, BACKPLANE_TELEMETRY_KID="dev-test", BACKPLANE_TELEMETRY_KEY=key,
            BEND_NO_TELEMETRY="1")
        now = int(datetime.now(timezone.utc).timestamp())
        day = datetime.fromtimestamp(now, timezone.utc).date().isoformat()

        def run(home, n=1, share="on", consent="1", reset="0", permit="yes", overrides=None):
            result = subprocess.run([str(worker), str(home), str(now), str(n), share, consent, reset, permit],
                env=env | (overrides or {}), capture_output=True, text=True, timeout=60)
            # Never echo captured output: credentials must remain environment-only.
            check(result.returncode == 0, "native telemetry worker completes")

        def state(home):
            return json.loads((home / "telemetry/state.json").read_text())

        def retry_now(home):
            # Exercise retry without waiting an hour or falsifying signed time.
            value = state(home)
            value["next"] = 0
            (home / "telemetry/state.json").write_text(json.dumps(value))

        def usage_rows():
            with sqlite3.connect(database) as db:
                return db.execute("SELECT install_id, day, counts FROM usage ORDER BY install_id, day").fetchall()

        def expected(n):
            return dict(zip(telemetry.USAGE_COUNTS, [n, n, n, n, n, 100*n, 20*n, 30*n, 40*n, n]))

        def counts(n, ident):
            rows = [row for row in usage_rows() if row[0] == ident and row[1] == day]
            check(len(rows) == 1 and json.loads(rows[0][2]) == expected(n), f"receiver stores cumulative snapshot {n} exactly once")

        def replay(events):
            stamp = int(datetime.now(timezone.utc).timestamp())
            raw = json.dumps({"v": 1, "ts": stamp, "nonce": secrets.token_hex(8), "events": events}, separators=(",", ":")).encode()
            signature = "sha256=" + hmac.new(bytes.fromhex(key), str(stamp).encode() + b"." + raw, hashlib.sha256).hexdigest()
            request = urllib.request.Request(endpoint, data=raw, headers={"Content-Type": "application/json",
                "X-Backplane-Key": "dev-test", "X-Backplane-Timestamp": str(stamp), "X-Backplane-Signature": signature})
            with urllib.request.urlopen(request) as response:
                check(response.status == 202, "signed older snapshot accepted idempotently")

        try:
            home = root / "main"
            run(home)
            ident = state(home)["id"]
            counts(1, ident)
            first = next(event for request in requests for event in request["events"] if event["e"] == "usage")
            check(set(first) == {"e", "day", "share", "id", "ver", "ch", "os", "arch", "counts"}, "usage metadata has exact allowlist")
            check(requests[0]["events"][0]["e"] == "install", "registration is sent before usage")
            check((home / "telemetry/usage.json").stat().st_mode & 0o777 == 0o600, "acknowledgements mode 0600")
            before = len(requests)
            run(home)
            check(len(requests) == before, "unchanged snapshot produces no HTTP")
            run(home, 3)
            counts(3, ident)
            replay([first])
            counts(3, ident)
            mode = 429
            run(home, 4)
            check(state(home)["next"] >= now + 7200, "usage respects Retry-After")
            before = len(requests)
            run(home, 5)
            check(len(requests) == before, "usage honors persisted backoff")
            mode = None
            retry_now(home)
            run(home, 5)
            counts(5, ident)
            mode = "ambiguous"
            run(home, 7)
            counts(7, ident)
            mode = None
            retry_now(home)
            run(home, 7)
            counts(7, ident)
            mode = 400
            run(home, 8)
            before = len(requests)
            run(home, 8)
            check(len(requests) == before, "permanent rejection suppresses identical snapshot")
            mode = None
            run(home, 9)
            counts(9, ident)
            before = len(requests)
            run(home, 10, permit="no")
            check(len(requests) == before, "revoked worker permission prevents usage HTTP")
            run(home, 1, reset="1")
            rotated = state(home)["id"]
            check(rotated != ident, "reset rotates install id")
            counts(1, rotated)
            run(home, 1, share="off", reset="1")
            check(state(home)["id"] == "" and not (home / "telemetry/usage.json").exists(), "opt-out clears id and usage acknowledgements")
            before = len(requests)
            run(home, 10, share="off", reset="1")
            check(len(requests) == before, "opt-out sends no aggregate activity")
            before = len(usage_rows())
            run(root / "legacy", consent="")
            check(len(usage_rows()) == before, "legacy consent sends no expanded counters")
            for name, overrides in [("dnt", {"DO_NOT_TRACK": "1"}), ("ci", {"CI": "1"}),
                ("sandbox", {"BACKPLANE_SANDBOX": "1"}), ("off", {"BACKPLANE_TELEMETRY": "off"})]:
                closed = root / name
                before = len(requests)
                run(closed, overrides=overrides)
                check(len(requests) == before and not closed.exists(), name + " gate prevents HTTP and state writes")
            with app.app_context():
                public = telemetry.aggregate(include_dev=False)["usage"]["totals"]
                development = telemetry.aggregate(include_dev=True)["usage"]["totals"]
            check(public["threads_created"] == 0 and development["threads_created"] == 10, "investor totals exclude dev by default")
            check(development["total_tokens"] == 1900, "token totals include caches and never double-count reasoning")
            with sqlite3.connect(database) as db:
                check(db.execute("SELECT count(*) FROM events WHERE e='active'").fetchone()[0] == 0, "bot-only traffic never becomes human DAU")
            args.receipt.parent.mkdir(parents=True, exist_ok=True)
            args.receipt.write_text(json.dumps({"backend_commit": revision,
                "worker_sha256": hashlib.sha256(worker.read_bytes()).hexdigest(),
                "usage_rows": len(usage_rows()), "requests": len(requests),
                "source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
                "source_diff_sha256": hashlib.sha256(subprocess.check_output(["git", "diff", "HEAD"])).hexdigest(),
                "passed": True}, indent=2) + "\n")
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)


if __name__ == "__main__":
    main()
