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
        self.release = {"version": "0.12.0", "tag": "v0.12.0", "channel": "release", "commit": "a" * 40, "tree": "b" * 40}
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

    def fixture(self):
        build, runtime = self.root / "build", self.root / "runtime"
        web = build / "web"
        web.mkdir(parents=True)
        runtime.mkdir()
        self.elf(build / "backplane-serve")
        (web / "index.html").write_text("<p>Unicode 日本語</p>")
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
            "status": "VERIFIED_GENUINE_SERVICE_ONLY_CONFIGURED_BUILD", "target": "linux-x64",
            "source_commit": self.release["commit"], "source_tree": self.release["tree"], "version": S.VERSION,
            "versionStamp": S.VERSION, "telemetryChannel": "release", "compileInputs": S.compile_inputs(),
            "binary_sha256": S.digest(build / "backplane-serve"), "webManifestSha256": S.digest(build / "WEB.json"),
            "sourceManifestSha256": S.digest(build / "SOURCE.json")})
        (runtime / "lib").mkdir()
        (runtime / "lib/libfixture.so").write_bytes(b"fixture-runtime-not-executable")
        (runtime / "LICENSE.fixture").write_text("fixture license")
        packages = [{"name": "fixture", "version": "1", "sourceDisposition": "fixture only",
                     "licenses": [{"path": "LICENSE.fixture", "sha256": S.digest(runtime / "LICENSE.fixture")}]}]
        self.write(runtime / "RUNTIME.json", {"target": "linux-x64", "inventory": [
            {"path": "lib/libfixture.so", "package": "fixture", "version": "1", "architecture": "x64",
             "sha256": S.digest(runtime / "lib/libfixture.so"), "bytes": (runtime / "lib/libfixture.so").stat().st_size}], "packages": packages})
        self.write(runtime / "CLOSURE.json", {"target": "linux-x64", "measured": True, "status": "PASS", "unresolvedMandatory": [],
            "edges": [{"from": "backplane-serve", "kind": "dynamic", "name": "libfixture.so", "resolved": "lib/libfixture.so"}]})
        approval = {"schema": 1, "kind": "backplane-service-runtime-qualification", "status": "PASS", "target": "linux-x64",
                    "sourceCommit": self.release["commit"], "sourceTree": self.release["tree"],
                    "binarySha256": S.digest(build / "backplane-serve"), "runtimeManifestSha256": S.digest(runtime / "RUNTIME.json"),
                    "closureSha256": S.digest(runtime / "CLOSURE.json"), "members": S.inventory(runtime)}
        self.write(self.root / "approval.json", approval)
        return {"schema": 1, "kind": "backplane-service-component-inputs", "target": "linux-x64", "release": self.release,
                "buildReceipt": S.reference(build / "BUILD.json"), "buildDirectory": str(build),
                "runtimeDirectory": str(runtime), "runtimeQualification": S.reference(self.root / "approval.json")}

    def test_three_explicit_targets_and_stable_identity(self):
        self.assertEqual(S.TARGETS, {"linux-x64", "linux-arm64", "darwin-arm64"})
        self.assertEqual(S.identity({"schema": 1, "target": "linux-arm64", "release": self.release}), self.release)
        for version in ("0.11.2", "0.12.0-dev.test", "v0.12.0"):
            with self.assertRaises(ValueError):
                S.identity({"schema": 1, "target": "linux-x64", "release": {**self.release, "version": version}})

    def test_missing_protected_configuration_is_failure(self):
        for key in self.env:
            env = dict(self.env)
            del env[key]
            with self.assertRaises(ValueError):
                S.configured_environment(env)

    def test_wrong_endpoint_key_and_inherited_compile_overrides(self):
        for key, val in [("BACKPLANE_TELEMETRY_URL", "https://other.invalid"), ("BACKPLANE_TELEMETRY_KEY", "a"),
                         ("BACKPLANE_TELEMETRY_KID", "dev-test"), ("BACKPLANE_GOOGLE_CLIENT_SECRET", "$(bad)")]:
            with self.assertRaises(ValueError):
                S.configured_environment({**self.env, key: val})
        env = S.configured_environment({**self.env, "BACKPLANE_PREBUILT": "1", "BP_BUILD_LOCKED": "1", "BACKPLANE_GOOGLE_BAKE": "0"})
        self.assertNotIn("BACKPLANE_PREBUILT", env)
        self.assertNotIn("BP_BUILD_LOCKED", env)
        self.assertEqual(env["BACKPLANE_GOOGLE_BAKE"], "1")
        self.assertEqual(env["BACKPLANE_TELEMETRY_CHANNEL"], "release")

    def test_public_witness_never_contains_credentials(self):
        text = json.dumps(S.compile_inputs())
        for value in self.env.values():
            if value != S.ENDPOINT:
                self.assertNotIn(value, text)
        self.assertEqual(set(S.compile_inputs()["telemetry"]), {"endpoint", "requested", "requiredInputsPresent"})

    def test_target_host_refuses_before_tools_or_compile(self):
        req = {"schema": 1, "kind": "backplane-service-build-inputs", "target": "darwin-arm64", "release": self.release}
        with patch.object(S.platform, "system", return_value="Linux"), patch.object(S.platform, "machine", return_value="x86_64"), patch.object(S, "run") as run:
            with self.assertRaisesRegex(ValueError, "target host"):
                S.build(req, self.root, self.root / "out")
            run.assert_not_called()

    def test_bend_uses_version_command_clang_uses_version_flag(self):
        for name, command, output in [("bend", "version", "2.0.36"), ("clang", "--version", "clang version 21.1.0")]:
            tool = self.root / name
            tool.write_text("fixture tool")
            spec = {**S.reference(tool), "version": output}
            with patch.object(S.subprocess, "check_output", return_value=output) as invoke:
                self.assertEqual(S.check_tool(spec, self.root, name), tool)
                invoke.assert_called_once_with([str(tool), command], text=True, stderr=S.subprocess.STDOUT)

    def test_elf_and_macho_architecture(self):
        p = self.root / "native"
        for machine, target in [(62, "linux-x64"), (183, "linux-arm64")]:
            self.elf(p, machine)
            S.architecture(p, target)
            with self.assertRaises(ValueError):
                S.architecture(p, "linux-arm64" if machine == 62 else "linux-x64")
        p.write_bytes(struct.pack("<III", 0xfeedfacf, 0x100000c, 0))
        S.architecture(p, "darwin-arm64")

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
        data["version"] = "0.12.0-dev.test"
        S.write(p, data)
        req["buildReceipt"] = S.reference(p)
        with self.assertRaises(ValueError):
            S.component(req, self.root, self.root / "out")

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

    def mocked_build(self, phase="build", request=None, outname="actual-output"):
        source = self.root / "source"
        source.mkdir(exist_ok=True)
        (source / "LICENSE").write_text("fixture license")
        (source / "src/web").mkdir(parents=True, exist_ok=True)
        (source / "src/web/sw.js").write_text("// fixture worker")
        (source / "build/cc/backplane-serve").mkdir(parents=True, exist_ok=True)
        for name in ("bp_google.h", "bp_telemetry.h"):
            (source / "build/cc/backplane-serve" / name).write_text("private fixture input")
        for name in ("bend", "clang"):
            (self.root / name).write_text("fixture tool")
        check_log = self.root / "source.log"
        check_log.write_text("fixture source pass, not product acceptance")
        self.write(self.root / "qualification.json", {"schema": 1, "kind": "backplane-web-source-qualification", "status": "PASS",
            "commit": self.release["commit"], "tree": self.release["tree"], "version": S.VERSION, "checks": [S.reference(check_log)]})
        self.write(self.root / "notices.json", {"schema": 1, "kind": "backplane-web-source-notices",
            "sourceCommit": self.release["commit"], "sourceTree": self.release["tree"], "licenses": {"LICENSE.Backplane": "LICENSE"},
            "packages": [{"name": "fixture", "version": "1", "sourceDisposition": "fixture", "licenses": [
                {"path": "LICENSE.Backplane", "sha256": S.digest(source / "LICENSE")}]}]})
        req = request or {"schema": 1, "kind": "backplane-service-build-inputs", "target": "linux-x64", "release": self.release,
               "toolchain": {name: {**S.reference(self.root / name), "version": "fixture"} for name in ("bend", "clang")},
               "sourceQualification": S.reference(self.root / "qualification.json"), "webNotices": S.reference(self.root / "notices.json")}
        out, calls = self.root / outname, []

        def fake_run(args, log, env, timeout=2400):
            args = [str(a) for a in args]
            calls.append(args)
            Path(log).write_text("offline mock operation\n")
            if args[0] == "scripts/build-js.sh":
                self.assertEqual(args[1], "src/server/main.bend")
                self.assertEqual(env["BACKPLANE_GOOGLE_BAKE"], "1")
                self.assertEqual(env["BACKPLANE_TELEMETRY_BAKE"], "1")
                self.assertEqual(env["BACKPLANE_JS_TARGET"], req["target"])
                self.assertEqual(env["BACKPLANE_JS_BUN_RUNTIME"], str(self.root / "bun-runtime"))
                self.assertNotIn("CC", env)
                self.elf(Path(args[2]), 183 if req["target"] == "linux-arm64" else 62)
                return "stages: linux-arm64 emit (fresh)\n"
            if args[0] == "scripts/build-app.sh":
                self.assertEqual(args[1], "src/server/main.bend")
                if env.get("BACKPLANE_EMIT_ONLY") == "1":
                    self.assertEqual(env["BACKPLANE_GOOGLE_BAKE"], "0")
                    self.assertEqual(env["BACKPLANE_TELEMETRY_BAKE"], "0")
                    for key in self.env:
                        self.assertNotIn(key, env)
                    (source / "build/cc/backplane-serve/all.c").write_text("// offline portable C fixture\n")
                    return "build/cc/backplane-serve/all.c\n"
                self.elf(Path(args[2]))
                if env.get("BACKPLANE_PREBUILT") == "1":
                    self.assertEqual(env["BACKPLANE_JOBS"], "1")
                    self.assertEqual(env["BACKPLANE_GOOGLE_BAKE"], "1")
                    self.assertEqual(env["BACKPLANE_TELEMETRY_BAKE"], "1")
                    text = "prebuilt build/cc/backplane-serve/all.c\n"
                    Path(log).write_text(text)
                    return text
            elif "src/web/index.html" in args:
                web = Path(args[-1])
                web.mkdir()
                (web / "index.html").write_text("fixture")
                (web / "chunk-0.js").write_text("// fixture")
            elif args[-1] == "--version":
                return "backplane 0.12.0\n"
            return ""

        with patch.object(S, "ROOT", source), patch.object(S, "NATIVE_LOCK", self.root / "native.lock"), \
             patch.dict(os.environ, self.env, clear=True), patch.object(S, "frozen"), \
             patch.object(S, "check_tool", side_effect=lambda spec, base, name: self.root / name), \
             patch.object(S.subprocess, "check_output", return_value="1792559000000000000"), \
             patch.object(S, "git", return_value="LICENSE"), patch.object(S, "run", side_effect=fake_run), \
             patch.object(S.platform, "system", return_value="Linux"), patch.object(S.platform, "machine", return_value="x86_64"), \
             patch.object(S.fcntl, "flock"):
            result = S.build(req, self.root, out, phase=phase)
        return out, calls, req

    def test_build_calls_only_server_and_production_web_then_writes_witness(self):
        out, calls, req = self.mocked_build()
        self.assertEqual(len([c for c in calls if c[0] == "scripts/build-app.sh"]), 1)
        self.assertFalse(any("src/app/main.bend" in c for c in calls))
        self.assertEqual(S.read(out / "BUILD.json")["compileInputs"], S.compile_inputs())
        self.assertEqual(S.read(out / "WEB.json"), S.web_manifest(out / "web", self.release))
        self.assertEqual(S.read(out / "WEB-EMISSION.json")["sourceCommit"], self.release["commit"])
        self.assertFalse(any((self.root / "source/build/cc/backplane-serve" / n).exists() for n in ("bp_google.h", "bp_telemetry.h")))

    def js_request(self, target="linux-arm64", runtime_machine=183, runtime_target=None):
        self.elf(self.root / "bun-runtime", runtime_machine)
        for name in ("bend", "bun"):
            (self.root / name).write_text("fixture tool")
        req = {"schema": 1, "kind": "backplane-service-js-inputs", "target": target, "release": self.release,
               "toolchain": {"bend": {**S.reference(self.root / "bend"), "version": "fixture"},
                             "bun": {**S.reference(self.root / "bun"), "version": "1.4.3"},
                             "bunRuntime": {**S.reference(self.root / "bun-runtime"), "version": "1.4.3",
                                            "target": runtime_target or target}},
               "sourceQualification": S.reference(self.root / "qualification.json"),
               "webNotices": S.reference(self.root / "notices.json")}
        return req

    def test_js_cross_builds_on_the_pinned_target_runtime(self):
        # an x64 host makes the arm64 service: bun links the JS onto the
        # arm64 runtime pinned in the request; nothing runs it here
        self.mocked_build()  # writes the fixture qualification and notices
        out, calls, req = self.mocked_build(phase="js", request=self.js_request(), outname="js-output")
        self.assertEqual([c[0] for c in calls if c[0].startswith("scripts/")], ["scripts/build-js.sh"])
        self.assertFalse(any(c[-1] == "--version" for c in calls))
        build = S.read(out / "BUILD.json")
        self.assertEqual(build["serviceRuntime"], "bun")
        self.assertEqual(build["target"], "linux-arm64")
        self.assertEqual(set(build["toolchain"]), {"bend", "bun", "bunRuntime"})
        self.assertEqual(build["compileInputs"], S.compile_inputs())
        S.architecture(out / "backplane-serve", "linux-arm64")

    def test_js_on_its_own_host_checks_the_version(self):
        self.mocked_build()
        out, calls, req = self.mocked_build(phase="js", request=self.js_request("linux-x64", 62), outname="js-x64")
        self.assertTrue(any(c[-1] == "--version" for c in calls))

    def test_js_refuses_a_runtime_of_another_target(self):
        self.mocked_build()
        with self.assertRaises(ValueError):
            self.mocked_build(phase="js", request=self.js_request("linux-arm64", 62, "linux-x64"), outname="js-bad")
        with self.assertRaises(ValueError):
            self.mocked_build(phase="js", request=self.js_request("linux-arm64", 62), outname="js-bad2")

    def test_js_has_no_windows_target_yet(self):
        self.mocked_build()
        req = self.js_request()
        req["target"] = "windows-x64"
        with self.assertRaises(ValueError):
            S.build(req, self.root, self.root / "js-win", phase="js")

    def test_c_build_keeps_its_targets_and_runtime(self):
        out, calls, req = self.mocked_build()
        self.assertEqual(S.read(out / "BUILD.json")["serviceRuntime"], "native-c")
        req = dict(req, target="darwin-x64")
        with self.assertRaises(ValueError):
            S.build(req, self.root, self.root / "c-darwin-x64")

    def test_darwin_x64_macho(self):
        b = bytearray(32)
        struct.pack_into("<II", b, 0, 0xfeedfacf, 0x1000007)
        (self.root / "m").write_bytes(b)
        S.architecture(self.root / "m", "darwin-x64")
        with self.assertRaises(ValueError):
            S.architecture(self.root / "m", "darwin-arm64")

    def test_secretless_emit_has_no_native_binary_or_bake_witness(self):
        self.env = {}
        out, calls, req = self.mocked_build(phase="emit")
        emitted = S.read(out / "EMISSION.json")
        self.assertFalse((out / "BUILD.json").exists())
        self.assertFalse((out / "backplane-serve").exists())
        self.assertFalse(emitted["nativeCompilation"])
        self.assertFalse(emitted["configuredBuild"])
        self.assertEqual(emitted["nativeTargets"], sorted(S.TARGETS))
        self.assertEqual(S.bound(emitted["cArtifact"], out), out / "all.c")
        self.assertFalse(any("--version" in c for c in calls))

    def native_request(self, emitted):
        return {"schema": 1, "kind": "backplane-service-native-inputs", "target": "linux-x64", "release": self.release,
                "toolchain": {"clang": {**S.reference(self.root / "clang"), "version": "fixture"}},
                "emissionReceipt": S.reference(emitted / "EMISSION.json"), "emissionDirectory": str(emitted)}

    def test_bound_native_split_never_emits_bend_or_web_and_uses_one_cc_lane(self):
        emitted, _, _ = self.mocked_build(phase="emit", outname="packet")
        request = self.native_request(emitted)
        out, calls, _ = self.mocked_build(phase="native", request=request, outname="native")
        self.assertEqual(len(calls), 2)  # existing native builder + actual binary --version only
        self.assertEqual(calls[0][0], "scripts/build-app.sh")
        self.assertFalse(any("src/web/index.html" in c or "bend_check" in " ".join(c) for c in calls))
        built = S.read(out / "BUILD.json")
        self.assertEqual(built["compileInputs"], S.compile_inputs())
        self.assertEqual(built["portableEmission"]["receiptSha256"], request["emissionReceipt"]["sha256"])
        self.assertEqual(built["portableEmission"]["cSha256"], S.digest(emitted / "all.c"))
        self.assertEqual(S.read(out / "WEB.json"), S.read(emitted / "WEB.json"))

    def test_emit_strips_inherited_protected_configuration(self):
        out, _, _ = self.mocked_build(phase="emit")
        text = (out / "EMISSION.json").read_text()
        for value in self.env.values():
            self.assertNotIn(value, text)

    def test_native_rejects_unbound_changed_c_before_compiler(self):
        packet, _, _ = self.mocked_build(phase="emit", outname="packet")
        request = self.native_request(packet)
        (packet / "all.c").write_text("// changed C")
        with patch.object(S, "ROOT", self.root / "source"), patch.object(S, "git", return_value="LICENSE"):
            with self.assertRaisesRegex(ValueError, "membership changed"):
                S.emission_packet(request, self.root, self.release)

    def test_native_rejects_foreign_commit_and_source_bytes(self):
        packet, _, _ = self.mocked_build(phase="emit", outname="packet")
        request = self.native_request(packet)
        with patch.object(S, "ROOT", self.root / "source"), patch.object(S, "git", return_value="LICENSE"):
            with self.assertRaisesRegex(ValueError, "identity"):
                S.emission_packet(request, self.root, {**self.release, "commit": "c" * 40})
            (self.root / "source/LICENSE").write_text("changed source")
            with self.assertRaisesRegex(ValueError, "source bytes"):
                S.emission_packet(request, self.root, self.release)

    def test_packet_external_notice_needs_no_emitter_checkout_file(self):
        web = self.root / "packet/web"
        web.mkdir(parents=True)
        (web / "LICENSE.Bend").write_text("public external license")
        metadata = {"licenses": {"LICENSE.Bend": "build/release-notice-inputs/Bend.txt"},
                    "packages": [{"licenses": [{"path": "LICENSE.Bend", "sha256": S.digest(web / "LICENSE.Bend")}]}]}
        with patch.object(S, "ROOT", self.root / "absent-native-checkout"), patch.object(S, "git", return_value="LICENSE"):
            S.license_sources(metadata, self.root / "packet")
            with self.assertRaisesRegex(ValueError, "License content"):
                S.license_sources(metadata)
            (web / "LICENSE.Bend").write_text("changed")
            with self.assertRaisesRegex(ValueError, "License content"):
                S.license_sources(metadata, self.root / "packet")

    def test_packet_tracked_notice_still_requires_exact_checkout_bytes(self):
        web = self.root / "packet/web"
        web.mkdir(parents=True)
        (web / "LICENSE.Backplane").write_text("original")
        (self.root / "LICENSE").write_text("changed")
        metadata = {"licenses": {"LICENSE.Backplane": "LICENSE"}, "packages": [{"licenses": [
                    {"path": "LICENSE.Backplane", "sha256": S.digest(web / "LICENSE.Backplane")}]}]}
        with patch.object(S, "ROOT", self.root), patch.object(S, "git", return_value="LICENSE"):
            with self.assertRaisesRegex(ValueError, "Tracked license"):
                S.license_sources(metadata, self.root / "packet")
            (self.root / "LICENSE").write_text("original")
            S.license_sources(metadata, self.root / "packet")
            metadata["packages"][0]["licenses"][0]["sha256"] = "0" * 64
            with self.assertRaisesRegex(ValueError, "License content"):
                S.license_sources(metadata, self.root / "packet")

    def test_native_requires_protected_configuration_before_consumption(self):
        packet, _, _ = self.mocked_build(phase="emit", outname="packet")
        request = self.native_request(packet)
        with patch.object(S, "frozen"), patch.dict(os.environ, {}, clear=True), patch.object(S, "run") as run, \
             patch.object(S.platform, "system", return_value="Linux"), patch.object(S.platform, "machine", return_value="x86_64"):
            with self.assertRaisesRegex(ValueError, "Google"):
                S.native(request, self.root, self.root / "native")
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main(verbosity=2)
