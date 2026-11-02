"""Offline unit checks for the explicit AWS throwaway-client test harness."""
import importlib.util
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


if __name__ == "__main__":
    unittest.main()
