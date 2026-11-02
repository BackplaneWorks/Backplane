#!/usr/bin/env python3
"""Disposable native sender -> exact signed forwarding -> canonical AWS receiver.

Run only for an explicitly authorized AWS test, with the existing dev key supplied
through a secret environment. Never uses an installed hub, its home, or real data.
HTTP 202 is transport evidence; the receiver owner must verify stored rows and
aggregation for the generated UUIDs before reporting acceptance as PASS.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import hmac
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request
import uuid

ENDPOINT = "https://backplaneworks.com/api/v1/events"
COUNTERS = ("threads_created", "user_messages", "assistant_messages", "bot_messages", "bot_wakes",
            "input_tokens", "cache_read_tokens", "cache_write_tokens", "output_tokens", "usage_turns")


def counts(n):
    return dict(zip(COUNTERS, (n, n, n, n, n, 100*n, 20*n, 30*n, 40*n, n)))


def proof_ok(ident, proof):
    digest = hashlib.sha256(f"bp-pow1:{ident}:{proof}".encode()).digest()
    return digest[0] == digest[1] == 0 and digest[2] & 0xC0 == 0


def signature(key, stamp, raw):
    return "sha256=" + hmac.new(key, str(stamp).encode()+b"."+raw, hashlib.sha256).hexdigest()


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        return None


def forward(raw, headers):
    request = urllib.request.Request(ENDPOINT, data=raw, method="POST", headers=headers)
    try:
        with urllib.request.build_opener(NoRedirect()).open(request, timeout=20) as reply:
            reply.read(16384)
            return reply.status
    except urllib.error.HTTPError as reply:
        reply.read(16384)
        return reply.code


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--worker", type=Path, required=True)
    parser.add_argument("--worker-sha256", required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--key-id", default="dev-1")
    args = parser.parse_args()
    if not re.fullmatch(r"dev-[a-z0-9-]{1,28}", args.key_id):
        parser.error("only an existing development key id is allowed")
    key_text = os.environ.get("BACKPLANE_TELEMETRY_KEY", "")
    if not re.fullmatch(r"[0-9a-f]{64}", key_text):
        parser.error("a dev key must be supplied through the secret environment")
    key = bytes.fromhex(key_text)
    worker = args.worker.resolve()
    actual_sha = hashlib.sha256(worker.read_bytes()).hexdigest()
    if actual_sha != args.worker_sha256:
        parser.error("native worker hash does not match its reviewed artifact")
    receipt = {"endpoint": ENDPOINT, "channel": "dev", "key_id": args.key_id,
               "worker_sha256": actual_sha, "uses_real_hub_home": False,
               "production_default_totals_expected_unchanged": True,
               "receiver_storage_and_aggregation": "PENDING_OWNER_READBACK",
               "requests": [], "stages": {}, "test_install_ids": []}
    events = []
    failures = []

    def stage(name, ok):
        receipt["stages"][name] = "PASS" if ok else "FAIL"
        print(("PASS " if ok else "FAIL ")+name, flush=True)
        if not ok:
            raise AssertionError(name)

    def post_events(batch, name):
        stamp = int(time.time())
        raw = json.dumps({"v": 1, "ts": stamp, "nonce": secrets.token_hex(8), "events": batch}, separators=(",", ":")).encode()
        status = forward(raw, {"Content-Type": "application/json", "X-Backplane-Key": args.key_id,
                              "X-Backplane-Timestamp": str(stamp), "X-Backplane-Signature": signature(key, stamp, raw)})
        receipt["requests"].append({"stage": name, "status": status,
                                    "body_sha256": hashlib.sha256(raw).hexdigest(), "events": [e["e"] for e in batch]})
        return status

    class Recorder(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_POST(self):
            try:
                length = int(self.headers.get("Content-Length", "0"))
                if self.path != "/api/v1/events" or not 0 < length <= 16384:
                    raise ValueError("bounded canonical ingest path required")
                raw = self.rfile.read(length)
                body = json.loads(raw)
                if not hmac.compare_digest(signature(key, body["ts"], raw), self.headers.get("X-Backplane-Signature", "")):
                    raise ValueError("native HMAC did not match exact body")
                if self.headers.get("X-Backplane-Key") != args.key_id or self.headers.get("X-Backplane-Timestamp") != str(body["ts"]):
                    raise ValueError("native signing metadata mismatch")
                if b"private-" in raw or b"/private/" in raw:
                    raise ValueError("private fixture data reached wire")
                if any(e["ch"] != "dev" or e["share"] is not True for e in body["events"]):
                    raise ValueError("only identified dev test events allowed")
                for event in body["events"]:
                    if event["e"] == "install" and not proof_ok(event["id"], event.get("pow", "")):
                        raise ValueError("native registration proof invalid")
                headers = {name: self.headers[name] for name in ("X-Backplane-Key", "X-Backplane-Timestamp", "X-Backplane-Signature")}
                headers["Content-Type"] = "application/json"
                # Forward exact body and signatures, with redirects disabled.
                status = forward(raw, headers)
                receipt["requests"].append({"stage": "native-forward", "status": status,
                                            "body_sha256": hashlib.sha256(raw).hexdigest(),
                                            "hmac_exact": True, "events": [e["e"] for e in body["events"]]})
                events.extend(body["events"])
                self.send_response(status)
                self.send_header("Content-Length", "2")
                self.end_headers()
                self.wfile.write(b"{}")
            except Exception as error:
                # Exception type only: never print credentials/body/raw headers.
                failures.append(type(error).__name__)
                self.send_response(502)
                self.send_header("Content-Length", "2")
                self.end_headers()
                self.wfile.write(b"{}")

    server = ThreadingHTTPServer(("127.0.0.1", 0), Recorder)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with tempfile.TemporaryDirectory(prefix="bp-aws-usage-test-") as temp:
            home = Path(temp)/"throwaway"
            env = {k: v for k, v in os.environ.items() if not k.startswith("BACKPLANE_TELEMETRY")}
            env.update(HOME=temp, CURL_HOME=temp, CI="", DO_NOT_TRACK="", BACKPLANE_SANDBOX="",
                       BACKPLANE_TELEMETRY="on", BACKPLANE_TELEMETRY_TEST="1", BACKPLANE_TELEMETRY_CHANNEL="dev",
                       BACKPLANE_TELEMETRY_URL=f"http://127.0.0.1:{server.server_port}/api/v1/events",
                       BACKPLANE_TELEMETRY_KID=args.key_id, BACKPLANE_TELEMETRY_KEY=key_text,
                       BEND_NO_TELEMETRY="1", BACKPLANE_KISTACK_SYNC="0")
            stamp = int(time.time())

            def run(n):
                result = subprocess.run([str(worker), str(home), str(stamp), str(n), "on", "1", "0", "yes"],
                                        env=env, capture_output=True, timeout=90)
                stage("native_worker_run_"+str(n), result.returncode == 0 and not failures)

            run(3)
            installs = [e for e in events if e["e"] == "install"]
            usage = [e for e in events if e["e"] == "usage"]
            receipt["test_install_ids"] = list(dict.fromkeys(e["id"] for e in events))
            stage("native_registration_before_usage", bool(installs) and bool(usage) and events[0]["e"] == "install")
            stage("native_registration_18_bit_pow", all(proof_ok(e["id"], e["pow"]) for e in installs))
            stage("initial_all_ten_counters", usage[-1]["counts"] == counts(3))
            stage("aws_https_transport_initial_202", all(r["status"] == 202 for r in receipt["requests"]))
            original = usage[-1]
            before = len(receipt["requests"])
            run(3)
            stage("identical_snapshot_sends_nothing", len(receipt["requests"]) == before)
            run(5)
            usage = [e for e in events if e["e"] == "usage"]
            stage("updated_all_ten_counters", usage[-1]["counts"] == counts(5))
            stage("aws_https_transport_updated_202", receipt["requests"][-1]["status"] == 202)
            stage("older_snapshot_transport_202", post_events([original], "older_snapshot") == 202)
            invalid = dict(installs[0], id=str(uuid.uuid4()), pow="0")
            while proof_ok(invalid["id"], invalid["pow"]):
                invalid["pow"] += "0"
            receipt["test_install_ids"].append(invalid["id"])
            invalid_status = post_events([invalid], "invalid_registration_pow")
            receipt["invalid_registration"] = {"id": invalid["id"], "pow_valid": False, "http_status": invalid_status,
                                                "storage_verification": "PENDING_OWNER_READBACK"}
            # Some receivers return 202 while discarding an invalid registration;
            # only storage verification can establish cryptographic rejection.
            stage("invalid_pow_no_server_error", 200 <= invalid_status < 500)
            receipt["expected"] = {"verified_install_id": installs[0]["id"], "day": original["day"],
                                   "counters_after_older_replay": counts(5), "total_tokens": 950,
                                   "invalid_pow_install_not_verified": True,
                                   "default_production_totals_include_no_test_ids": True}
            receipt["test_only_payload"] = {"version": original["ver"], "os": original["os"], "arch": original["arch"]}
    finally:
        server.shutdown()
        server.server_close()
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        fd = os.open(args.receipt, os.O_WRONLY|os.O_CREAT|os.O_TRUNC, 0o600)
        with os.fdopen(fd, "w") as output:
            json.dump(receipt, output, indent=2)
            output.write("\n")
        print("Receiver storage/aggregation verification: PENDING OWNER READBACK", flush=True)


if __name__ == "__main__":
    main()
