"""Offline unit checks for the explicit AWS throwaway-client test harness."""
import importlib.util
import contextlib
import hashlib
import json
import os
import tempfile
import sys
import io
from pathlib import Path
import unittest
import urllib.error
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("telemetry_aws_probe", Path(__file__).with_name("telemetry_aws_probe.py"))
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class AwsProbe(unittest.TestCase):
    def test_exact_numeric_contract_and_tokens(self):
        self.assertEqual(list(probe.counts(5)), list(probe.COUNTERS))
        self.assertEqual(list(probe.counts(5).values()), [5, 5, 5, 5, 5, 500, 100, 150, 200, 5])
        row = probe.counts(5)
        self.assertEqual(sum(row[k] for k in ["input_tokens", "cache_read_tokens", "cache_write_tokens", "output_tokens"]), 950)

    def test_real_pow_vector_and_invalid_proof(self):
        ident = "7d0c2f6e-3b1a-4f7e-9a51-2c8e0d4b6f10"
        self.assertTrue(probe.proof_ok(ident, "186d7"))
        self.assertFalse(probe.proof_ok(ident, "0"))

    def test_hmac_known_vector(self):
        self.assertEqual(probe.signature(bytes(32), 1, b"{}"), "sha256=a07ac7cfc802ea7629f0933ebf00ef447832d56dcdcab022d121f329249e4ac3")

    def test_never_follows_receiver_redirect(self):
        self.assertIsNone(probe.NoRedirect().redirect_request(None, None, 302, "redirect", {}, "https://elsewhere.invalid/"))

    def test_canonical_https_endpoint_and_exact_bytes(self):
        class Reply:
            status = 202
            def __enter__(self): return self
            def __exit__(self, *args): pass
            def read(self, n): return b"{}"
        class Opener:
            def open(self, request, timeout):
                self.request, self.timeout = request, timeout
                return Reply()
        opener = Opener()
        raw = b'{"exact":"body"}'
        with patch.object(probe.urllib.request, "build_opener", return_value=opener):
            self.assertEqual(probe.forward(raw, {"Content-Type": "application/json"}), 202)
        self.assertEqual(opener.request.full_url, "https://backplaneworks.com/api/v1/events")
        self.assertEqual(opener.request.data, raw)
        self.assertEqual(opener.request.get_method(), "POST")
        self.assertEqual(opener.timeout, 20)

    def test_records_http_rejection_instead_of_acceptance(self):
        with patch.object(probe.urllib.request, "build_opener") as factory:
            factory.return_value.open.side_effect = urllib.error.HTTPError(probe.ENDPOINT, 409, "replay", {}, io.BytesIO(b"{}"))
            self.assertEqual(probe.forward(b"{}", {}), 409)

    def test_cleanup_ids_and_nonces_are_deduplicated(self):
        receipt = {"test_install_ids": [], "test_nonces": []}
        ident = "7d0c2f6e-3b1a-4f7e-9a51-2c8e0d4b6f10"
        probe.remember_fixtures(receipt, [{"id": ident}, {"id": ident}], "0011223344556677")
        probe.remember_fixtures(receipt, [{"id": ident}], "0011223344556677")
        self.assertEqual(receipt, {"test_install_ids": [ident], "test_nonces": ["0011223344556677"]})

    def test_partial_worker_failure_still_persists_cleanup_identity(self):
        # An isolated fake child submits a valid signed registration then fails.
        # The mocked forwarding endpoint is never contacted over the network.
        with tempfile.TemporaryDirectory() as temp:
            worker = Path(temp)/"fake-worker"
            worker.write_text("""#!/usr/bin/env python3
import hashlib,hmac,json,os,sys,urllib.request
key=bytes.fromhex(os.environ['BACKPLANE_TELEMETRY_KEY'])
stamp=int(sys.argv[2])
event={'e':'install','day':'2026-11-02','share':True,'id':'7d0c2f6e-3b1a-4f7e-9a51-2c8e0d4b6f10','ver':'0.12.0','ch':'dev','os':'linux','arch':'x64','pow':'186d7'}
raw=json.dumps({'v':1,'ts':stamp,'nonce':'0011223344556677','events':[event]},separators=(',',':')).encode()
sig='sha256='+hmac.new(key,str(stamp).encode()+b'.'+raw,hashlib.sha256).hexdigest()
req=urllib.request.Request(os.environ['BACKPLANE_TELEMETRY_URL'],data=raw,headers={'X-Backplane-Key':'dev-1','X-Backplane-Timestamp':str(stamp),'X-Backplane-Signature':sig})
with urllib.request.urlopen(req) as reply: reply.read()
sys.exit(1)
""")
            worker.chmod(0o700)
            receipt = Path(temp)/"receipt.json"
            args = ["probe", "--worker", str(worker), "--worker-sha256", hashlib.sha256(worker.read_bytes()).hexdigest(), "--receipt", str(receipt)]
            observed_before_forward = []
            def forward(raw, headers):
                stored = json.loads(receipt.read_text())
                observed_before_forward.append(stored["test_install_ids"])
                return 202
            with patch.dict(os.environ, {"BACKPLANE_TELEMETRY_KEY": "ab"*32}), patch.object(sys, "argv", args), patch.object(probe, "forward", side_effect=forward), contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises(AssertionError):
                    probe.main()
            stored = json.loads(receipt.read_text())
            self.assertEqual(observed_before_forward, [["7d0c2f6e-3b1a-4f7e-9a51-2c8e0d4b6f10"]])
            self.assertEqual(stored["test_install_ids"], ["7d0c2f6e-3b1a-4f7e-9a51-2c8e0d4b6f10"])
            self.assertEqual(stored["test_nonces"], ["0011223344556677"])
            self.assertEqual(receipt.stat().st_mode & 0o777, 0o600)


if __name__ == "__main__":
    unittest.main()
