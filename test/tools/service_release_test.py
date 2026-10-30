#!/usr/bin/env python3
"""Offline producer boundary tests; synthetic ELF headers are never executed."""
import copy
import gzip
import importlib.util
import json
import os
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[2] / "scripts/service-release.py"
spec = importlib.util.spec_from_file_location("service_release", SOURCE)
S = importlib.util.module_from_spec(spec)
spec.loader.exec_module(S)


class ProducerTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.release = {"version": S.VERSION, "tag": "v" + S.VERSION, "channel": "release", "commit": "a" * 40, "tree": "b" * 40}
        self.env = {"BACKPLANE_GOOGLE_CLIENT_ID": "test.invalid", "BACKPLANE_GOOGLE_CLIENT_SECRET": "test-secret",
                    "BACKPLANE_TELEMETRY_URL": S.ENDPOINT, "BACKPLANE_TELEMETRY_KID": "rel-test",
                    "BACKPLANE_TELEMETRY_KEY": "f" * 64}

    def write(self, path, value):
        path.parent.mkdir(parents=True, exist_ok=True)
        S.write(path, value)

    def elf(self, path, machine=62):
        b = bytearray(32)
        b[:6] = b"\x7fELF\x02\x01"
        struct.pack_into("<H", b, 18, machine)
        path.write_bytes(b)
        path.chmod(0o755)

    def pe(self, path, machine=0x8664, signature=b"PE\0\0"):
        b = bytearray(0x100)
        b[:2] = b"MZ"
        struct.pack_into("<I", b, 0x3c, 0x80)
        b[0x80:0x84] = signature
        struct.pack_into("<H", b, 0x84, machine)
        path.write_bytes(b)
        path.chmod(0o755)

    def fixture(self, target="linux-x64"):
        build, runtime = self.root / "build", self.root / "runtime"
        web = build / "web"
        web.mkdir(parents=True)
        runtime.mkdir()
        service = build / S.service_name(target)
        self.pe(service) if target == "windows-x64" else self.elf(service)
        (web / "index.html").write_text("<p>Unicode 日本語</p>")
        for sub, name in (("bun", "LICENSE.md"), ("bend", "LICENSE")):
            (build / "licenses" / sub).mkdir(parents=True)
            (build / "licenses" / sub / name).write_text("fixture notice")
        (web / "index.html.gz").write_bytes(gzip.compress((web / "index.html").read_bytes(), mtime=0))
        (web / "LICENSE.Backplane").write_text("fixture license")
        (build / "LICENSE.Backplane").write_text("fixture license")
        (build / "logs").mkdir()
        (build / "logs/source.log").write_text("fixture only")
        for name in ("SOURCE-QUALIFICATION.json", "SOURCE-QUALIFICATION-BOUND.json"):
            self.write(build / name, {"checks": [S.local_reference(build / "logs/source.log", build)]})
        self.write(build / "WEB-EMISSION.json", {"sourceQualification": S.local_reference(build / "SOURCE-QUALIFICATION-BOUND.json", build)})
        self.write(build / "WEB.json", S.web_manifest(web, self.release))
        self.write(build / "SOURCE.json", {"fixture": True})
        self.write(build / "BUILD.json", {
            "status": "VERIFIED_GENUINE_SERVICE_ONLY_CONFIGURED_BUILD", "target": target, "serviceRuntime": "bun",
            "service": service.name,
            "source_commit": self.release["commit"], "source_tree": self.release["tree"], "version": S.VERSION,
            "versionStamp": S.VERSION, "telemetryChannel": "release", "compileInputs": S.compile_inputs(),
            "binary_sha256": S.digest(service), "webManifestSha256": S.digest(build / "WEB.json"),
            "sourceManifestSha256": S.digest(build / "SOURCE.json")})
        (runtime / "lib").mkdir()
        (runtime / "lib/libfixture.so").write_bytes(b"fixture-runtime-not-executable")
        (runtime / "LICENSE.fixture").write_text("fixture license")
        packages = [{"name": "fixture", "version": "1", "sourceDisposition": "fixture only",
                     "licenses": [{"path": "LICENSE.fixture", "sha256": S.digest(runtime / "LICENSE.fixture")}]}]
        self.write(runtime / "RUNTIME.json", {"target": target, "inventory": [
            {"path": "lib/libfixture.so", "package": "fixture", "version": "1", "architecture": "x64",
             "sha256": S.digest(runtime / "lib/libfixture.so"), "bytes": (runtime / "lib/libfixture.so").stat().st_size}], "packages": packages})
        self.write(runtime / "CLOSURE.json", {"target": target, "measured": True, "status": "PASS", "unresolvedMandatory": [],
            "edges": [{"from": service.name, "kind": "dynamic", "name": "libfixture.so", "resolved": "lib/libfixture.so"}]})
        approval = {"schema": 1, "kind": "backplane-service-runtime-qualification", "status": "PASS", "target": target,
                    "sourceCommit": self.release["commit"], "sourceTree": self.release["tree"],
                    "binarySha256": S.digest(service), "runtimeManifestSha256": S.digest(runtime / "RUNTIME.json"),
                    "closureSha256": S.digest(runtime / "CLOSURE.json"), "members": S.inventory(runtime)}
        self.write(self.root / "approval.json", approval)
        return {"schema": 1, "kind": "backplane-service-component-inputs", "target": target, "release": self.release,
                "buildReceipt": S.reference(build / "BUILD.json"), "buildDirectory": str(build),
                "runtimeDirectory": str(runtime), "runtimeQualification": S.reference(self.root / "approval.json")}

    def test_js_targets_and_stable_identity(self):
        self.assertEqual(S.TARGETS, {"linux-x64", "linux-arm64", "darwin-x64", "darwin-arm64", "windows-x64"})
        self.assertEqual(S.service_name("windows-x64"), "backplane-serve.exe")
        self.assertEqual(S.service_name("linux-arm64"), "backplane-serve")
        self.assertEqual(S.identity({"schema": 1, "target": "linux-arm64", "release": self.release}), self.release)
        with self.assertRaises(ValueError):
            S.identity({"schema": 1, "target": "windows-arm64", "release": self.release})
        for version in ("0.11.2", "0.12.0", "0.12.2-dev.test", "v0.12.2"):
            with self.assertRaises(ValueError):
                S.identity({"schema": 1, "target": "linux-x64", "release": {**self.release, "version": version}})

    def test_missing_protected_configuration_is_failure(self):
        for key in self.env:
            env = dict(self.env)
            del env[key]
            with self.assertRaises(ValueError):
                S.configured_environment(env)

    def test_wrong_endpoint_key_and_inherited_compile_overrides(self):
        for key, val in [("BACKPLANE_TELEMETRY_URL", "https://backplane.works/api/v1/events"),
                         ("BACKPLANE_TELEMETRY_URL", "https://other.invalid"), ("BACKPLANE_TELEMETRY_KEY", "a"),
                         ("BACKPLANE_TELEMETRY_KID", "dev-test"), ("BACKPLANE_GOOGLE_CLIENT_SECRET", "$(bad)")]:
            with self.assertRaises(ValueError):
                S.configured_environment({**self.env, key: val})
        env = S.configured_environment({**self.env, "BACKPLANE_JS_TARGET": "linux-arm64", "BACKPLANE_JS_BUN_RUNTIME": "/x",
                                        "BACKPLANE_COMMIT_STAMP": "c" * 40, "BACKPLANE_GOOGLE_BAKE": "0", "BACKPLANE_EMIT_CACHE": "/c"})
        for name in ("BACKPLANE_JS_TARGET", "BACKPLANE_JS_BUN_RUNTIME", "BACKPLANE_COMMIT_STAMP", "BACKPLANE_CFLAGS"):
            self.assertNotIn(name, env)
        self.assertEqual(env["BACKPLANE_EMIT_CACHE"], "0")
        self.assertEqual(env["BACKPLANE_GOOGLE_BAKE"], "1")
        self.assertEqual(env["BACKPLANE_TELEMETRY_CHANNEL"], "release")

    def test_public_witness_never_contains_credentials(self):
        text = json.dumps(S.compile_inputs())
        for value in self.env.values():
            if value != S.ENDPOINT:
                self.assertNotIn(value, text)
        self.assertEqual(set(S.compile_inputs()["telemetry"]), {"endpoint", "requested", "requiredInputsPresent"})

    def test_wrong_kind_refuses_before_tools_or_build(self):
        req = {"schema": 1, "kind": "backplane-service-build-inputs", "target": "linux-x64", "release": self.release}
        with patch.object(S, "frozen") as frozen, patch.object(S, "run") as run:
            with self.assertRaisesRegex(ValueError, "kind"):
                S.js(req, self.root, self.root / "out")
            run.assert_not_called()
            frozen.assert_not_called()

    def test_bend_uses_version_command_bun_uses_version_flag(self):
        for name, command, output in [("bend", "version", "2.0.36"), ("bun", "--version", "1.4.3")]:
            tool = self.root / name
            tool.write_text("fixture tool")
            spec = {**S.reference(tool), "version": output}
            with patch.object(S.subprocess, "check_output", return_value=output) as invoke:
                self.assertEqual(S.check_tool(spec, self.root, name), tool)
                invoke.assert_called_once_with([str(tool), command], text=True, stderr=S.subprocess.STDOUT)
        with patch.object(S.subprocess, "check_output", return_value="2.0.0"):
            with self.assertRaisesRegex(ValueError, "Bun 1"):
                S.check_tool({**S.reference(self.root / "bun"), "version": "2.0.0"}, self.root, "bun")

    def test_elf_macho_and_pe_architecture(self):
        p = self.root / "native"
        for machine, target in [(62, "linux-x64"), (183, "linux-arm64")]:
            self.elf(p, machine)
            S.architecture(p, target)
            with self.assertRaises(ValueError):
                S.architecture(p, "linux-arm64" if machine == 62 else "linux-x64")
            with self.assertRaises(ValueError):
                S.architecture(p, "windows-x64")
        p.write_bytes(struct.pack("<III", 0xfeedfacf, 0x100000c, 0))
        S.architecture(p, "darwin-arm64")
        with self.assertRaises(ValueError):
            S.architecture(p, "windows-x64")
        self.pe(p)
        S.architecture(p, "windows-x64")
        for target in ("linux-x64", "darwin-x64"):
            with self.assertRaises(ValueError):
                S.architecture(p, target)
        self.pe(p, 0xaa64)
        with self.assertRaisesRegex(ValueError, "PE architecture"):
            S.architecture(p, "windows-x64")
        self.pe(p, signature=b"NE\0\0")
        with self.assertRaisesRegex(ValueError, "PE architecture"):
            S.architecture(p, "windows-x64")
        p.write_bytes(b"MZ")
        with self.assertRaisesRegex(ValueError, "not a PE"):
            S.architecture(p, "windows-x64")

    def test_output_is_never_reused(self):
        with self.assertRaises(ValueError):
            S.new_output(self.root)
        with self.assertRaises(ValueError):
            S.new_output(Path("relative"))

    def test_member_escape_private_font_and_privileged_modes_refused(self):
        for name in ("../x", "/x", "a\\x", "a//x", "a/./x"):
            with self.assertRaises(ValueError):
                S.relative(name)
        for name, data in [("token", b"x"), ("test.woff2", b"x"), ("font.data", b"wOF2xxxx")]:
            d = self.root / "payload"
            d.mkdir(exist_ok=True)
            p = d / name
            p.write_bytes(data)
            with self.assertRaises(ValueError):
                S.inventory(d)
            p.unlink()
        p = d / "setuid"
        p.write_bytes(b"x")
        p.chmod(0o4755)
        with self.assertRaises(ValueError):
            S.inventory(d)

    def test_escaping_and_dangling_runtime_symlinks_refused(self):
        d = self.root / "payload"
        d.mkdir()
        (d / "link").symlink_to("../outside")
        (self.root / "outside").write_bytes(b"x")
        with self.assertRaises(ValueError):
            S.inventory(d)

    def test_hash_size_binding(self):
        p = self.root / "artifact"
        p.write_bytes(b"original")
        ref = S.reference(p)
        self.assertEqual(S.bound(ref, self.root), p)
        p.write_bytes(b"changed!")
        with self.assertRaises(ValueError):
            S.bound(ref, self.root)

    def test_missing_or_corrupt_gzip_refused(self):
        req = self.fixture()
        p = self.root / "build/web/index.html.gz"
        p.write_bytes(gzip.compress(b"wrong"))
        with self.assertRaises(ValueError):
            S.web_manifest(p.parent, self.release)
        p.unlink()
        with self.assertRaises(ValueError):
            S.web_manifest(p.parent, self.release)

    def test_component_preserves_raw_build_and_exact_web(self):
        req = self.fixture()
        original = (self.root / "build/BUILD.json").read_bytes()
        out = self.root / "component"
        with patch.object(S, "run") as run:
            result = S.component(req, self.root, out)
            run.assert_not_called()
        self.assertEqual((out / "payload/BUILD.json").read_bytes(), original)
        receipt = S.read(out / "SERVICE.json")
        self.assertEqual(receipt["members"], S.inventory(out / "payload"))
        self.assertEqual(receipt["kind"], "backplane-bend-service")
        self.assertEqual(receipt["runtimeAcceptance"], "PENDING")
        self.assertFalse(receipt["published"])

    def test_dev_build_cannot_be_relabelled(self):
        req = self.fixture()
        p = self.root / "build/BUILD.json"
        data = S.read(p)
        data["version"] = "0.12.2-dev.test"
        S.write(p, data)
        req["buildReceipt"] = S.reference(p)
        with self.assertRaises(ValueError):
            S.component(req, self.root, self.root / "out")

    def test_component_refuses_a_build_of_another_runtime(self):
        req = self.fixture()
        p = self.root / "build/BUILD.json"
        S.write(p, {**S.read(p), "serviceRuntime": "native-c"})
        req["buildReceipt"] = S.reference(p)
        with self.assertRaises(ValueError):
            S.component(req, self.root, self.root / "out")

    def test_windows_component_carries_the_exe(self):
        req = self.fixture("windows-x64")
        out = self.root / "component"
        S.component(req, self.root, out)
        receipt = S.read(out / "SERVICE.json")
        self.assertEqual(receipt["roles"]["service"], "backplane-serve.exe")
        self.assertIn("backplane-serve.exe", receipt["members"])
        self.assertNotIn("backplane-serve", receipt["members"])
        p = self.root / "build/backplane-serve.exe"
        self.elf(p)
        with self.assertRaises(ValueError):
            S.component(req, self.root, self.root / "out2")

    def test_component_and_web_provenance_survive_cross_host_directory_move(self):
        req = self.fixture()
        out = self.root / "component"
        S.component(req, self.root, out)
        moved = self.root / "other-host/component"
        moved.parent.mkdir()
        out.rename(moved)
        receipt = S.read(moved / "SERVICE.json")
        self.assertEqual(S.bound(receipt["buildReceipt"], moved), moved / "payload/BUILD.json")
        emission = S.read(moved / "payload/WEB-EMISSION.json")
        qualified_path = S.bound(emission["sourceQualification"], moved / "payload")
        qualification = S.read(qualified_path)
        self.assertEqual(S.bound(qualification["checks"][0], qualified_path.parent), moved / "payload/logs/source.log")

    def test_original_build_cannot_gain_a_posthoc_bake_witness(self):
        req = self.fixture()
        p = self.root / "build/BUILD.json"
        data = S.read(p)
        del data["compileInputs"]["telemetry"]["endpoint"]
        S.write(p, data)
        req["buildReceipt"] = S.reference(p)
        with self.assertRaises(ValueError):
            S.component(req, self.root, self.root / "out")

    def test_changed_runtime_does_not_pass_old_approval(self):
        req = self.fixture()
        (self.root / "runtime/lib/libfixture.so").write_bytes(b"changed")
        with self.assertRaises(ValueError):
            S.component(req, self.root, self.root / "out")

    def test_unresolved_or_wrong_target_closure_refused(self):
        req = self.fixture()
        out = self.root / "component"
        S.component(req, self.root, out)
        closure = S.read(out / "payload/CLOSURE.json")
        for key, value in [("unresolvedMandatory", ["missing.so"]), ("target", "linux-arm64"), ("measured", False)]:
            S.write(out / "payload/CLOSURE.json", {**closure, key: value})
            with self.assertRaises(ValueError):
                S.runtime_closure(out / "payload", "linux-x64")

    def test_logs_redact_protected_values_and_client_prefix(self):
        completed = S.subprocess.CompletedProcess([], 0, ("baked Google client test-...\n" + self.env["BACKPLANE_GOOGLE_CLIENT_SECRET"] + "\n").encode())
        log = self.root / "log"
        with patch.object(S.subprocess, "run", return_value=completed):
            S.run(["unused"], log, self.env)
        self.assertNotIn("test-...", log.read_text())
        self.assertNotIn("test-secret", log.read_text())

    def inputs(self):
        source = self.root / "source"
        source.mkdir(exist_ok=True)
        (source / "LICENSE").write_text("fixture license")
        (source / "src/web").mkdir(parents=True, exist_ok=True)
        (source / "src/web/sw.js").write_text("// fixture worker")
        (source / "build/js/backplane-serve").mkdir(parents=True, exist_ok=True)
        for name in ("baked.js", "bin.js"):
            (source / "build/js/backplane-serve" / name).write_text("private fixture input")
        for name in ("bend", "bun"):
            (self.root / name).write_text("fixture tool")
        check_log = self.root / "source.log"
        check_log.write_text("fixture source pass, not product acceptance")
        self.write(self.root / "qualification.json", {"schema": 1, "kind": "backplane-web-source-qualification", "status": "PASS",
            "commit": self.release["commit"], "tree": self.release["tree"], "version": S.VERSION, "checks": [S.reference(check_log)]})
        self.write(self.root / "notices.json", {"schema": 1, "kind": "backplane-web-source-notices",
            "sourceCommit": self.release["commit"], "sourceTree": self.release["tree"], "licenses": {"LICENSE.Backplane": "LICENSE"},
            "packages": [{"name": "fixture", "version": "1", "sourceDisposition": "fixture", "licenses": [
                {"path": "LICENSE.Backplane", "sha256": S.digest(source / "LICENSE")}]}]})
        return source

    def js_request(self, target="linux-arm64", runtime_machine=183, runtime_target=None):
        self.inputs()
        runtime = self.root / "bun-runtime"
        self.pe(runtime, runtime_machine) if target == "windows-x64" else self.elf(runtime, runtime_machine)
        req = {"schema": 1, "kind": "backplane-service-js-inputs", "target": target, "release": self.release,
               "toolchain": {"bend": {**S.reference(self.root / "bend"), "version": "fixture"},
                             "bun": {**S.reference(self.root / "bun"), "version": "1.4.3"},
                             "bunRuntime": {**S.reference(runtime), "version": "1.4.3",
                                            "target": runtime_target or target}},
               "sourceQualification": S.reference(self.root / "qualification.json"),
               "webNotices": S.reference(self.root / "notices.json")}
        return req

    def mocked_build(self, request=None, outname="actual-output", host=("Linux", "x86_64"), extra=None):
        source = self.inputs()
        req = request or self.js_request("linux-x64", 62)
        out, calls = self.root / outname, []

        def fake_run(args, log, env, timeout=2400):
            args = [str(a) for a in args]
            calls.append(args)
            Path(log).write_text("offline mock operation\n")
            if args[0] == "scripts/js-licenses.sh":
                self.assertEqual(env["BACKPLANE_LICENSES_STRICT"], "1")
                for sub, name in (("bun", "LICENSE.md"), ("bend", "LICENSE")):
                    (Path(args[1]) / sub).mkdir(parents=True, exist_ok=True)
                    (Path(args[1]) / sub / name).write_text("fixture notice")
                return ""
            if args[0] == "scripts/build-js.sh":
                self.assertEqual(args[1], "src/server/main.bend")
                self.assertEqual(args[2], str(out / "backplane-serve"))
                self.assertEqual(env["BACKPLANE_GOOGLE_BAKE"], "1")
                self.assertEqual(env["BACKPLANE_TELEMETRY_BAKE"], "1")
                self.assertEqual(env["BACKPLANE_JS_TARGET"], req["target"])
                self.assertEqual(env["BACKPLANE_COMMIT_STAMP"], self.release["commit"])
                self.assertEqual(env["BACKPLANE_JS_BUN_RUNTIME"], str(self.root / "bun-runtime"))
                for name in ("CC", "BACKPLANE_CFLAGS", "BACKPLANE_OBJ_CACHE"):
                    self.assertNotIn(name, env)
                if req["target"] == "windows-x64":
                    self.pe(Path(args[2] + ".exe"))
                else:
                    self.elf(Path(args[2]), 183 if req["target"] == "linux-arm64" else 62)
                if extra:
                    (out / extra).write_text("stray")
                return "stages: " + req["target"] + " emit (fresh)\n"
            elif "src/web/index.html" in args:
                web = Path(args[-1])
                web.mkdir()
                (web / "index.html").write_text("fixture")
                (web / "chunk-0.js").write_text("// fixture")
            elif args[-1] == "--version":
                return f"backplane {S.VERSION}\n"
            return ""

        with patch.object(S, "ROOT", source), \
             patch.dict(os.environ, self.env, clear=True), patch.object(S, "frozen"), \
             patch.object(S, "check_tool", side_effect=lambda spec, base, name: self.root / name), \
             patch.object(S.subprocess, "check_output", return_value="1792559000000000000"), \
             patch.object(S, "git", return_value="LICENSE"), patch.object(S, "run", side_effect=fake_run), \
             patch.object(S.platform, "system", return_value=host[0]), patch.object(S.platform, "machine", return_value=host[1]):
            S.js(req, self.root, out)
        return out, calls, req

    def test_js_calls_only_server_and_production_web_then_writes_witness(self):
        out, calls, req = self.mocked_build()
        self.assertEqual([c[0] for c in calls if c[0].startswith("scripts/")], ["scripts/build-js.sh", "scripts/js-licenses.sh"])
        self.assertFalse(any("src/app/main.bend" in c for c in calls))
        build = S.read(out / "BUILD.json")
        self.assertEqual(build["compileInputs"], S.compile_inputs())
        self.assertEqual(build["serviceRuntime"], "bun")
        self.assertEqual(build["service"], "backplane-serve")
        self.assertEqual(S.read(out / "WEB.json"), S.web_manifest(out / "web", self.release))
        self.assertEqual(S.read(out / "WEB-EMISSION.json")["sourceCommit"], self.release["commit"])
        self.assertFalse(any((self.root / "source/build/js/backplane-serve" / n).exists() for n in ("baked.js", "bin.js")))

    def test_js_cross_builds_on_the_pinned_target_runtime(self):
        # an x64 host makes the arm64 service: bun links the JS onto the
        # arm64 runtime pinned in the request; nothing runs it here
        out, calls, req = self.mocked_build(request=self.js_request(), outname="js-output")
        self.assertEqual([c[0] for c in calls if c[0].startswith("scripts/")], ["scripts/build-js.sh", "scripts/js-licenses.sh"])
        self.assertFalse(any(c[-1] == "--version" for c in calls))
        build = S.read(out / "BUILD.json")
        self.assertEqual(build["serviceRuntime"], "bun")
        self.assertEqual(build["target"], "linux-arm64")
        self.assertEqual(set(build["toolchain"]), {"bend", "bun", "bunRuntime"})
        self.assertEqual(build["compileInputs"], S.compile_inputs())
        S.architecture(out / "backplane-serve", "linux-arm64")

    def test_js_on_its_own_host_checks_the_version(self):
        out, calls, req = self.mocked_build(outname="js-x64")
        self.assertTrue(any(c[-1] == "--version" for c in calls))

    def test_js_refuses_a_runtime_of_another_target(self):
        with self.assertRaises(ValueError):
            self.mocked_build(request=self.js_request("linux-arm64", 62, "linux-x64"), outname="js-bad")
        with self.assertRaises(ValueError):
            self.mocked_build(request=self.js_request("linux-arm64", 62), outname="js-bad2")
        with self.assertRaises(ValueError):
            self.mocked_build(request=self.js_request("windows-x64", 0xaa64), outname="js-bad3")

    def test_js_windows_writes_the_exe(self):
        out, calls, req = self.mocked_build(request=self.js_request("windows-x64", 0x8664), outname="js-win")
        self.assertFalse(any(c[-1] == "--version" for c in calls))
        self.assertFalse((out / "backplane-serve").exists())
        build = S.read(out / "BUILD.json")
        self.assertEqual(build["service"], "backplane-serve.exe")
        self.assertEqual(build["binary_sha256"], S.digest(out / "backplane-serve.exe"))
        S.architecture(out / "backplane-serve.exe", "windows-x64")
        out, calls, req = self.mocked_build(request=self.js_request("windows-x64", 0x8664), outname="js-win-host",
                                            host=("Windows", "AMD64"))
        self.assertEqual(calls[2][0], str(out / "backplane-serve.exe"))

    def test_js_refuses_stray_build_outputs(self):
        with self.assertRaisesRegex(ValueError, "Unexpected build outputs"):
            self.mocked_build(outname="js-stray", extra="backplane-serve.tmp")

    def test_darwin_x64_macho(self):
        b = bytearray(32)
        struct.pack_into("<II", b, 0, 0xfeedfacf, 0x1000007)
        (self.root / "m").write_bytes(b)
        S.architecture(self.root / "m", "darwin-x64")
        with self.assertRaises(ValueError):
            S.architecture(self.root / "m", "darwin-arm64")

    def test_tracked_notice_requires_exact_checkout_bytes(self):
        (self.root / "LICENSE").write_text("original")
        metadata = {"licenses": {"LICENSE.Backplane": "LICENSE"}, "packages": [{"licenses": [
                    {"path": "LICENSE.Backplane", "sha256": S.digest(self.root / "LICENSE")}]}]}
        with patch.object(S, "ROOT", self.root):
            S.license_sources(metadata)
            (self.root / "LICENSE").write_text("changed")
            with self.assertRaisesRegex(ValueError, "License content"):
                S.license_sources(metadata)

    def test_protected_configuration_required_before_any_build(self):
        req = self.js_request("linux-x64", 62)
        with patch.object(S, "frozen"), patch.dict(os.environ, {}, clear=True), patch.object(S, "run") as run:
            with self.assertRaisesRegex(ValueError, "Google"):
                S.js(req, self.root, self.root / "js-noconf")
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main(verbosity=2)
