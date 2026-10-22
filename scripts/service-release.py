#!/usr/bin/env python3
"""Configured service/web producer. Building and offline component assembly are separate."""
import argparse
import fcntl
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import sys

VERSION = "0.12.0"
ENDPOINT = "https://backplane.works/api/v1/events"
TARGETS = {"linux-x64", "linux-arm64", "darwin-arm64"}
# The JS service (phase "js": scripts/build-js.sh, `bun build --compile`
# against a pinned Bun runtime of the target), cross-built from any host.
# Windows joins when Bend's JS runtime has Win32 syscalls.
JS_TARGETS = {"linux-x64", "linux-arm64", "darwin-x64", "darwin-arm64"}
ROOT = Path(__file__).resolve().parents[1]
NATIVE_LOCK = Path("/tmp/bp-wt-build.lock")
HEX = re.compile(r"[0-9a-f]{64}\Z")
GIT = re.compile(r"[0-9a-f]{40}\Z")


def require(ok, message):
    if not ok:
        raise ValueError(message)


def digest(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for part in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(part)
    return h.hexdigest()


def read(path):
    return json.loads(Path(path).read_text())


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def reference(path):
    path = Path(path).resolve()
    require(path.is_file() and not path.is_symlink(), "Artifact must be a regular file")
    return {"path": str(path), "sha256": digest(path), "bytes": path.stat().st_size}


def local_reference(path, base):
    return {**reference(path), "path": Path(path).resolve().relative_to(Path(base).resolve()).as_posix()}


def bound(ref, base):
    require(isinstance(ref, dict) and HEX.fullmatch(str(ref.get("sha256", "")))
            and type(ref.get("bytes")) is int and ref["bytes"] >= 0, "Invalid artifact reference")
    path = Path(base) / ref["path"]
    require(path.is_file() and not path.is_symlink() and path.stat().st_size == ref["bytes"]
            and digest(path) == ref["sha256"], "Artifact reference mismatch")
    return path.resolve()


def relative(name):
    require(isinstance(name, str) and name and "\\" not in name and ":" not in name
            and "\0" not in name and not name.startswith("/")
            and all(p not in {"", ".", ".."} for p in name.split("/")), "Unsafe member path")
    return name


def inventory(root):
    root = Path(root).resolve()
    result, seen = {}, set()
    for path in sorted(root.rglob("*")):
        name = relative(path.relative_to(root).as_posix())
        require(name.lower() not in seen, "Case-colliding member")
        seen.add(name.lower())
        require(not re.search(r"(^|/)(\.env[^/]*|token|events\.jsonl|api-keys\.json|keyring|browser-profile|electron-profile|fontconfig)(/|$)", name, re.I), "Private payload member")
        if path.is_symlink():
            link = os.readlink(path)
            require(not Path(link).is_absolute() and "\\" not in link and ":" not in link,
                    "Unsafe payload link")
            resolved = path.resolve(strict=True)
            require(root in resolved.parents, "Escaping payload link")
            result[name] = {"type": "symlink", "target": link}
        elif path.is_dir():
            require(not path.stat().st_mode & 0o7000, "Privileged directory mode")
        else:
            require(path.is_file() and not path.stat().st_mode & 0o7000, "Special/privileged member")
            require(path.suffix.lower() not in {".ttf", ".otf", ".ttc", ".woff", ".woff2"}, "Bundled font")
            with path.open("rb") as stream:
                require(stream.read(4) not in {b"\x00\x01\x00\x00", b"OTTO", b"wOFF", b"wOF2", b"ttcf"}, "Font-format member")
            result[name] = {"type": "file", "sha256": digest(path), "bytes": path.stat().st_size,
                            "mode": path.stat().st_mode & 0o777}
    return result


def architecture(path, target):
    with Path(path).open("rb") as stream:
        b = stream.read(32)
    if target.startswith("linux-"):
        require(len(b) >= 20 and b[:6] == b"\x7fELF\x02\x01"
                and struct.unpack_from("<H", b, 18)[0] == (62 if target == "linux-x64" else 183),
                "Service is not the requested ELF architecture")
    else:
        cpu = 0x1000007 if target == "darwin-x64" else 0x100000c
        require(len(b) >= 12 and struct.unpack_from("<II", b) == (0xfeedfacf, cpu),
                "Service is not the requested Mach-O architecture")


def identity(request):
    release = request.get("release", {})
    require(request.get("schema") == 1 and request.get("target") in TARGETS | JS_TARGETS, "Unsupported target/schema")
    require(release.get("version") == VERSION and release.get("tag") == "v" + VERSION
            and release.get("channel") == "release" and GIT.fullmatch(release.get("commit", ""))
            and GIT.fullmatch(release.get("tree", "")), "Exact stable source identity required")
    return release


def compile_inputs():
    return {"version": VERSION, "versionStamp": VERSION, "telemetryChannel": "release",
            "google": {"requested": True, "requiredInputsPresent": True},
            "telemetry": {"requested": True, "requiredInputsPresent": True, "endpoint": ENDPOINT}}


def configured_environment(source):
    env = dict(source)
    for key in ("BACKPLANE_GOOGLE_CLIENT_ID", "BACKPLANE_GOOGLE_CLIENT_SECRET"):
        require(re.fullmatch(r"[A-Za-z0-9._-]+", env.get(key, "")), "Protected Google compile inputs missing/invalid")
    require(env.get("BACKPLANE_TELEMETRY_URL") == ENDPOINT, "Agreed telemetry endpoint required")
    require(re.fullmatch(r"rel-[a-z0-9-]{1,28}", env.get("BACKPLANE_TELEMETRY_KID", "")), "Release telemetry key ID required")
    require(re.fullmatch(r"[0-9a-f]{64}", env.get("BACKPLANE_TELEMETRY_KEY", "")), "Protected telemetry key required")
    for name in ("BACKPLANE_PREBUILT", "BACKPLANE_EMIT_ONLY", "BP_BUILD_LOCKED"):
        env.pop(name, None)
    env.update(BACKPLANE_VERSION=VERSION, BACKPLANE_VERSION_STAMP=VERSION,
               BACKPLANE_GOOGLE_BAKE="1", BACKPLANE_TELEMETRY_BAKE="1",
               BACKPLANE_TELEMETRY_CHANNEL="release", BACKPLANE_EMIT_CACHE="0",
               BACKPLANE_OBJ_CACHE="0", BACKPLANE_CFLAGS="-O3 -gline-tables-only")
    return env


def emission_environment(source):
    # Emission never consumes protected compile inputs, including inherited ones.
    env = {k: v for k, v in source.items()
           if not k.startswith(("BACKPLANE_GOOGLE_", "BACKPLANE_TELEMETRY_"))
           and k not in {"BACKPLANE_PREBUILT", "BACKPLANE_EMIT_ONLY", "BP_BUILD_LOCKED"}}
    env.update(BACKPLANE_VERSION=VERSION, BACKPLANE_VERSION_STAMP=VERSION,
               BACKPLANE_GOOGLE_BAKE="0", BACKPLANE_TELEMETRY_BAKE="0",
               BACKPLANE_EMIT_CACHE="0", BACKPLANE_OBJ_CACHE="0", BACKPLANE_EMIT_ONLY="1")
    return env


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def frozen(release):
    require(git("rev-parse", "HEAD") == release["commit"]
            and git("rev-parse", "HEAD^{tree}") == release["tree"], "Checkout differs from frozen commit/tree")
    require(not git("status", "--porcelain", "--untracked-files=all"), "Release checkout is dirty")


def check_tool(spec, base, command):
    path = bound(spec, base)
    version = subprocess.check_output([str(path), "version" if command == "bend" else "--version"], text=True, stderr=subprocess.STDOUT).strip()
    require(spec.get("version") and version == spec["version"], command + " version mismatch")
    if command == "bend":
        require(path.name == "bend", "Bend tool must have canonical executable name")
    elif command == "bun":
        require(re.fullmatch(r"1\.[0-9]+\.[0-9]+", version), "Bun 1.x required")
    else:
        require(re.search(r"clang version (?:[2-9][0-9]|19)\.", version), "Clang 19+ required")
    return path


def run(args, log, env, timeout=2400):
    result = subprocess.run([str(x) for x in args], cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout, close_fds=True)
    text = result.stdout.decode(errors="replace")
    # Build-app prints a Google client prefix. Neither it nor protected values enter public evidence.
    text = re.sub(r"(?m)^baked Google client .*?$", "configured Google inputs supplied privately", text)
    for name in ("BACKPLANE_GOOGLE_CLIENT_ID", "BACKPLANE_GOOGLE_CLIENT_SECRET",
                 "BACKPLANE_TELEMETRY_KID", "BACKPLANE_TELEMETRY_KEY"):
        if env.get(name):
            text = text.replace(env[name], "[protected]")
    Path(log).write_text(text)
    require(result.returncode == 0 and not re.search(r"^(SOME PROOFS FAIL|Error:|FAIL\b)", text, re.M),
            "Build operation failed; inspect sanitized operation log")
    return text


def web_manifest(root, release):
    members = inventory(root)
    require("index.html" in members and all(v["type"] == "file" for v in members.values()), "Invalid generated web root")
    for name in members:
        if Path(name).suffix in {".html", ".css", ".js"}:
            require(name + ".gz" in members, "Missing gzip sibling")
        if name.endswith(".gz"):
            plain = Path(root) / name[:-3]
            require(plain.is_file() and gzip.decompress((Path(root) / name).read_bytes()) == plain.read_bytes(),
                    "Gzip does not match generated plain asset")
    return {"schema": 1, "kind": "matching-generated-web", "sourceCommit": release["commit"],
            "sourceTree": release["tree"], "entrypoint": "src/web/index.html",
            "members": {k: {"sha256": v["sha256"], "bytes": v["bytes"]} for k, v in members.items()}}


def new_output(path):
    require(path.is_absolute() and not path.exists(), "Output must be a new absolute directory")


def emission_packet(request, base, release):
    receipt_path = bound(request["emissionReceipt"], base)
    packet = (base / request["emissionDirectory"]).resolve()
    r = read(receipt_path)
    require(receipt_path == packet / "EMISSION.json" and r.get("schema") == 1
            and r.get("kind") == "backplane-service-c-emission"
            and r.get("status") == "VERIFIED_PORTABLE_C_EMISSION_ONLY"
            and r.get("target") == "portable-c" and request["target"] in r.get("nativeTargets", [])
            and r.get("sourceCommit") == release["commit"] and r.get("sourceTree") == release["tree"]
            and r.get("version") == VERSION and r.get("versionStamp") == VERSION
            and r.get("channel") == "release" and r.get("entrypoint") == "src/server/main.bend",
            "Portable C packet identity does not match native request")
    require(r.get("emissionInputs") == {"googleBake": False, "telemetryBake": False, "protectedInputsForwarded": False},
            "Secret-free emission witness required")
    actual = inventory(packet)
    actual.pop("EMISSION.json", None)
    require(actual == r.get("members"), "Portable emission packet membership changed")
    require(bound(r["cArtifact"], packet) == packet / "all.c", "Wrong portable C artifact")
    source = read(packet / "SOURCE.json")
    require(digest(packet / "SOURCE.json") == r["sourceManifestSha256"]
            and source.get("commit") == release["commit"] and source.get("tree") == release["tree"]
            and source.get("entrypoint") == "src/server/main.bend", "Portable C source provenance mismatch")
    require(source.get("members") == {n: digest(ROOT / n) for n in git("ls-files").splitlines()},
            "Native checkout source bytes differ from portable C source")
    require(digest(packet / "WEB.json") == r["webManifestSha256"]
            and web_manifest(packet / "web", release) == read(packet / "WEB.json"), "Portable matching web changed")
    require(isinstance(r.get("compiler"), dict) and HEX.fullmatch(str(r["compiler"].get("sha256", "")))
            and r["compiler"].get("version"), "Missing emitter Bend identity")
    qualification = read(bound(r["sourceQualification"], packet))
    require(qualification.get("kind") == "backplane-web-source-qualification" and qualification.get("status") == "PASS"
            and qualification.get("commit") == release["commit"] and qualification.get("tree") == release["tree"]
            and qualification.get("version") == VERSION and qualification.get("checks"), "Portable source qualification mismatch")
    for ref in qualification["checks"]:
        bound(ref, packet)
    return packet, r


def license_sources(web_notices, packet=None):
    tracked = set(git("ls-files").splitlines())
    declared = {}
    for package in web_notices.get("packages", []):
        for ref in package.get("licenses", []):
            dest, h = relative(ref["path"]), ref.get("sha256", "")
            require(HEX.fullmatch(h) and declared.get(dest, h) == h, "Conflicting license hash")
            declared[dest] = h
    require(set(declared) == set(web_notices["licenses"]), "License inventory differs from packages")
    for dest, src in web_notices["licenses"].items():
        relative(dest)
        relative(src)
        # The packet has already passed its complete member/WEB hash checks.
        # External public notices are emission inputs, not native checkout files.
        source = packet / "web" / dest if packet else ROOT / src
        require(source.is_file() and not source.is_symlink() and digest(source) == declared[dest],
                "License content differs from declared package")
        if packet and src in tracked:
            frozen_source = ROOT / src
            require(frozen_source.is_file() and not frozen_source.is_symlink()
                    and digest(frozen_source) == declared[dest], "Tracked license source changed")


def build(request, base, out, phase="build"):
    release = identity(request)
    expected_kind = {"native": "backplane-service-native-inputs", "js": "backplane-service-js-inputs"}.get(
        phase, "backplane-service-build-inputs")
    require(request.get("kind") == expected_kind, "Wrong build request kind")
    require(request["target"] in (JS_TARGETS if phase == "js" else TARGETS), "Unsupported target for this producer")
    actual_host = {("Linux", "x86_64"): "linux-x64", ("Linux", "aarch64"): "linux-arm64",
                   ("Darwin", "arm64"): "darwin-arm64", ("Darwin", "x86_64"): "darwin-x64"}.get((platform.system(), platform.machine()))
    # the JS service is cross-built: bun links the emitted JS onto the
    # target's own pinned Bun runtime, nothing is compiled for the host
    require(phase in ("emit", "js") or actual_host == request["target"], "Real target host required; cross-target compilation is not supported")
    frozen(release)
    env = emission_environment(os.environ) if phase == "emit" else configured_environment(os.environ)
    bend = None if phase == "native" else check_tool(request["toolchain"]["bend"], base, "bend")
    if phase == "js":
        bun = check_tool(request["toolchain"]["bun"], base, "bun")
        bun_runtime = bound(request["toolchain"]["bunRuntime"], base)
        require(request["toolchain"]["bunRuntime"].get("target") == request["target"]
                and request["toolchain"]["bunRuntime"].get("version") == request["toolchain"]["bun"]["version"],
                "Pinned Bun runtime must be the target's, of the pinned Bun's version")
        architecture(bun_runtime, request["target"])
        env.update(BACKPLANE_JS_TARGET=request["target"], BACKPLANE_JS_BUN_RUNTIME=str(bun_runtime),
                   PATH=str(bun.parent) + os.pathsep + str(bend.parent) + os.pathsep + env.get("PATH", ""))
        env.pop("BACKPLANE_CFLAGS", None)
        env.pop("BACKPLANE_OBJ_CACHE", None)
    else:
        cc = check_tool(request["toolchain"]["clang"], base, "clang")
        env.update(CC=str(cc), PATH=(str(bend.parent) + os.pathsep if bend else "") + env.get("PATH", ""))
    clock = subprocess.check_output(["date", "+%s%N"], env=env, text=True).strip()
    require(re.fullmatch(r"[0-9]{19,}", clock), "Native builder requires GNU date in PATH (Darwin: coreutils gnubin)")
    packet, emitted = emission_packet(request, base, release) if phase == "native" else (None, None)
    if phase == "native":
        env.update(BACKPLANE_PREBUILT="1", BACKPLANE_JOBS="1")
    qualification_path = packet / "SOURCE-QUALIFICATION-BOUND.json" if packet else bound(request["sourceQualification"], base)
    qualification = read(qualification_path)
    require(qualification.get("schema") == 1 and qualification.get("kind") == "backplane-web-source-qualification"
            and qualification.get("status") == "PASS" and qualification.get("commit") == release["commit"]
            and qualification.get("tree") == release["tree"] and qualification.get("version") == VERSION,
            "Final source qualification is not bound to release")
    require(qualification.get("checks"), "Actual final source check logs required")
    for ref in qualification["checks"]:
        bound(ref, qualification_path.parent)
    notice_path = packet / "WEB-NOTICES.json" if packet else bound(request["webNotices"], base)
    web_notices = read(notice_path)
    require(web_notices.get("schema") == 1 and web_notices.get("kind") == "backplane-web-source-notices"
            and web_notices.get("sourceCommit") == release["commit"]
            and web_notices.get("sourceTree") == release["tree"]
            and isinstance(web_notices.get("licenses"), dict) and web_notices["licenses"],
            "Frozen web redistribution notices required")
    license_sources(web_notices, packet)
    new_output(out)
    out.mkdir(parents=True)
    logs = out / "logs"
    if packet:
        shutil.copytree(packet / "logs", logs)
    else:
        logs.mkdir()
    # Keep the standard native build lock through compile-header cleanup.
    with NATIVE_LOCK.open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        env["BP_BUILD_LOCKED"] = "1"
        try:
            if packet:
                work = ROOT / "build/cc/backplane-serve"
                work.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(packet / "all.c", work / "all.c")
                require(digest(work / "all.c") == emitted["cArtifact"]["sha256"], "Copied portable C changed")
            else:
                run(["bash", "-c", 'set -e; source scripts/bendok.sh; bend_check src/server/main.bend; bend_check src/web/app.bend'], logs / "entries.log", env, 600)
            if phase == "js":
                native_log = run(["scripts/build-js.sh", "src/server/main.bend", out / "backplane-serve"], logs / "js.log", env)
            else:
                native_log = run(["scripts/build-app.sh", "src/server/main.bend", out / "backplane-serve"],
                                 logs / ("c-emission.log" if phase == "emit" else "native.log"), env)
            if phase == "emit":
                shutil.copyfile(ROOT / "build/cc/backplane-serve/all.c", out / "all.c")
            if packet:
                require("prebuilt build/cc/backplane-serve/all.c" in native_log
                        and digest(work / "all.c") == emitted["cArtifact"]["sha256"], "Native builder did not consume exact approved C")
        finally:
            for name in ("bp_google.h", "bp_telemetry.h"):
                (ROOT / "build/cc/backplane-serve" / name).unlink(missing_ok=True)
            for name in ("baked.js", "bin.js", "baked.js.t", "baked.js.g"):
                (ROOT / "build/js/backplane-serve" / name).unlink(missing_ok=True)
    if phase != "emit":
        architecture(out / "backplane-serve", request["target"])
    # a cross-built JS service is run on its target by runtime acceptance
    if phase != "emit" and actual_host == request["target"]:
        version = run([out / "backplane-serve", "--version"], logs / "version.log", env, 30).strip()
        require(version == "backplane " + VERSION, "Compiled service did not report exact stable version")
    frozen(release)
    tool_names = ("clang",) if packet else ("bend", "bun", "bunRuntime") if phase == "js" else ("bend", "clang")
    for name in tool_names:
        bound(request["toolchain"][name], base)
    shutil.copyfile(Path(__file__), out / "EXECUTION-HELPER.py")
    web = out / "web"
    if packet:
        shutil.copytree(packet / "web", web)
    else:
        run([bend, "src/web/index.html", "-o", web], logs / "web.log", env, 900)
        chunks = sorted(web.glob("chunk-*.js"))
        require(chunks, "No emitted web chunks")
        run([sys.executable, "scripts/js-unmarshal.py", "../core/client.Ui", *chunks], logs / "unmarshal.log", env)
        run([sys.executable, "scripts/js-native.py", *chunks], logs / "native-js.log", env)
        shutil.copyfile(ROOT / "src/web/sw.js", web / "sw.js")
        for dest, src in web_notices["licenses"].items():
            destination = web / dest
            require(not destination.exists(), "License collides with generated web asset")
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / src, destination)
    notices(web, web_notices.get("packages"))
    for path in ([] if packet else sorted(web.rglob("*"))):
        if path.is_file() and path.suffix in {".js", ".css", ".html"}:
            path.with_name(path.name + ".gz").write_bytes(gzip.compress(path.read_bytes(), compresslevel=9, mtime=0))
    write(out / "WEB.json", web_manifest(web, release))
    write(out / "WEB-NOTICES.json", web_notices)
    shutil.copyfile(ROOT / "LICENSE", out / "LICENSE.Backplane")
    shutil.copyfile(packet / "SOURCE-QUALIFICATION.json" if packet else qualification_path,
                    out / "SOURCE-QUALIFICATION.json")
    # Preserve the qualified logs and rebind their locations without changing their bytes.
    copied = dict(qualification)
    copied["checks"] = []
    for i, ref in enumerate(qualification["checks"]):
        dest = logs / ("source-" + str(i) + ".log")
        shutil.copyfile(bound(ref, qualification_path.parent), dest)
        copied["checks"].append(local_reference(dest, out))
    write(out / "SOURCE-QUALIFICATION-BOUND.json", copied)
    source = {"schema": 1, "kind": "backplane-service-source", "commit": release["commit"], "tree": release["tree"],
              "entrypoint": "src/server/main.bend", "members": {name: digest(ROOT / name) for name in git("ls-files").splitlines()}}
    write(out / "SOURCE.json", source)
    compiler = emitted["compiler"] if packet else {"version": request["toolchain"]["bend"]["version"], "sha256": request["toolchain"]["bend"]["sha256"]}
    write(out / "WEB-EMISSION.json", {"schema": 1, "kind": "backplane-web-emission", "status": "VERIFIED", "target": "web",
          "version": VERSION, "channel": "release", "sourceCommit": release["commit"], "sourceTree": release["tree"],
          "entrypoint": "src/web/index.html", "webManifestSha256": digest(out / "WEB.json"), "compiler": compiler,
          "packages": web_notices["packages"], "sourceQualification": local_reference(out / "SOURCE-QUALIFICATION-BOUND.json", out)})
    frozen(release)
    for name in tool_names:
        bound(request["toolchain"][name], base)
    if phase == "emit":
        result = {"schema": 1, "kind": "backplane-service-c-emission", "status": "VERIFIED_PORTABLE_C_EMISSION_ONLY",
                  "target": "portable-c", "nativeTargets": sorted(TARGETS), "version": VERSION,
                  "versionStamp": VERSION, "channel": "release", "sourceCommit": release["commit"], "sourceTree": release["tree"],
                  "entrypoint": "src/server/main.bend", "compiler": compiler, "cArtifact": local_reference(out / "all.c", out),
                  "sourceManifestSha256": digest(out / "SOURCE.json"), "webManifestSha256": digest(out / "WEB.json"),
                  "sourceQualification": local_reference(out / "SOURCE-QUALIFICATION-BOUND.json", out),
                  "emissionInputs": {"googleBake": False, "telemetryBake": False, "protectedInputsForwarded": False},
                  "members": inventory(out), "nativeCompilation": False, "configuredBuild": False, "published": False}
        result["executionHelper"] = local_reference(out / "EXECUTION-HELPER.py", out)
        write(out / "EMISSION.json", result)
        return {"status": result["status"], "emissionReceipt": reference(out / "EMISSION.json"), "payload": str(out)}
    if packet:
        # Revalidate the complete immutable producer packet after native consumption.
        emission_packet(request, base, release)
    result = {"schema": 1, "status": "VERIFIED_GENUINE_SERVICE_ONLY_CONFIGURED_BUILD", "target": request["target"],
              "entry": "src/server/main.bend", "version": VERSION, "versionStamp": VERSION, "telemetryChannel": "release",
              "source_commit": release["commit"], "source_tree": release["tree"], "compileInputs": compile_inputs(),
              "binary_sha256": digest(out / "backplane-serve"), "webManifestSha256": digest(out / "WEB.json"),
              "sourceManifestSha256": digest(out / "SOURCE.json"),
              "toolchain": {name: {key: request["toolchain"][name][key] for key in ("path", "bytes", "sha256", "version")}
                            for name in tool_names},
              "serviceRuntime": "bun" if phase == "js" else "native-c",
              "logs": {p.name: local_reference(p, out) for p in sorted(logs.iterdir())},
              "runtimeAcceptance": "PENDING", "published": False}
    result["executionHelper"] = local_reference(out / "EXECUTION-HELPER.py", out)
    if packet:
        result["portableEmission"] = {"receiptSha256": request["emissionReceipt"]["sha256"],
                                      "cSha256": emitted["cArtifact"]["sha256"], "compiler": compiler,
                                      "nativeCompilerLanes": 1}
    write(out / "BUILD.json", result)
    return {"status": result["status"], "buildReceipt": reference(out / "BUILD.json"), "payload": str(out)}


def emit(request, base, out):
    return build(request, base, out, phase="emit")


def native(request, base, out):
    return build(request, base, out, phase="native")


def js(request, base, out):
    return build(request, base, out, phase="js")


def notices(root, packages):
    require(isinstance(packages, list) and packages, "Redistribution notices required")
    for item in packages:
        require(item.get("name") and item.get("version") and item.get("sourceDisposition") and item.get("licenses"), "Incomplete package notice")
        for license in item["licenses"]:
            require(digest(root / relative(license["path"])) == license["sha256"], "Notice mismatch")


def runtime_closure(root, target):
    runtime, closure = read(root / "RUNTIME.json"), read(root / "CLOSURE.json")
    require(runtime.get("target") == target and runtime.get("inventory") and closure.get("target") == target
            and closure.get("measured") is True and closure.get("status") == "PASS"
            and closure.get("unresolvedMandatory") == [] and isinstance(closure.get("edges"), list), "Measured target runtime closure required")
    for item in runtime["inventory"]:
        path = root / relative(item["path"])
        require(item.get("package") and item.get("version") and item.get("architecture")
                and path.stat().st_size == item["bytes"] and digest(path) == item["sha256"], "Runtime inventory mismatch")
    for edge in closure["edges"]:
        require((root / relative(edge["from"])).is_file() and edge.get("kind") in {"normal", "delay", "dynamic"}
                and edge.get("name"), "Invalid measured edge")
        if edge.get("system") is True:
            require(edge.get("classification") == "approved-system", "Unapproved system dependency")
        else:
            require((root / relative(edge["resolved"])).is_file(), "Unresolved dependency")
    notices(root, runtime.get("packages"))
    return runtime


def component(request, base, out):
    release = identity(request)
    require(request.get("kind") == "backplane-service-component-inputs", "Wrong component request kind")
    raw = bound(request["buildReceipt"], base)
    build_data = read(raw)
    source = (base / request["buildDirectory"]).resolve()
    require(raw == source / "BUILD.json" and build_data.get("status") == "VERIFIED_GENUINE_SERVICE_ONLY_CONFIGURED_BUILD"
            and build_data.get("target") == request["target"] and build_data.get("source_commit") == release["commit"]
            and build_data.get("source_tree") == release["tree"] and build_data.get("version") == VERSION
            and build_data.get("versionStamp") == VERSION and build_data.get("telemetryChannel") == "release"
            and build_data.get("compileInputs") == compile_inputs(), "Original configured BUILD identity required")
    require(digest(source / "backplane-serve") == build_data["binary_sha256"]
            and digest(source / "WEB.json") == build_data["webManifestSha256"]
            and digest(source / "SOURCE.json") == build_data["sourceManifestSha256"], "Built component changed")
    architecture(source / "backplane-serve", request["target"])
    require(web_manifest(source / "web", release) == read(source / "WEB.json"), "Web manifest mismatch")
    runtime_root = (base / request["runtimeDirectory"]).resolve()
    approval_path = bound(request["runtimeQualification"], base)
    approval = read(approval_path)
    require(approval.get("schema") == 1 and approval.get("kind") == "backplane-service-runtime-qualification"
            and approval.get("status") == "PASS" and approval.get("target") == request["target"]
            and approval.get("sourceCommit") == release["commit"] and approval.get("sourceTree") == release["tree"]
            and approval.get("binarySha256") == build_data["binary_sha256"]
            and approval.get("runtimeManifestSha256") == digest(runtime_root / "RUNTIME.json")
            and approval.get("closureSha256") == digest(runtime_root / "CLOSURE.json"), "Exact measured runtime approval required")
    expected_runtime = approval.get("members")
    require(expected_runtime == inventory(runtime_root), "Approved runtime payload changed")
    new_output(out)
    payload = out / "payload"
    payload.mkdir(parents=True)
    for name in ("backplane-serve", "BUILD.json", "SOURCE.json", "WEB.json", "LICENSE.Backplane",
                 "WEB-EMISSION.json", "SOURCE-QUALIFICATION.json", "SOURCE-QUALIFICATION-BOUND.json"):
        shutil.copy2(source / name, payload / name)
    if "executionHelper" in build_data:
        helper = bound(build_data["executionHelper"], source)
        require(helper == source / "EXECUTION-HELPER.py", "Execution helper provenance path mismatch")
        shutil.copy2(helper, payload / "EXECUTION-HELPER.py")
    shutil.copytree(source / "web", payload / "web")
    shutil.copytree(source / "logs", payload / "logs")
    for child in runtime_root.iterdir():
        require(not (payload / child.name).exists(), "Runtime collides with built service/web")
        if child.is_dir() and not child.is_symlink():
            shutil.copytree(child, payload / child.name, symlinks=True)
        else:
            shutil.copy2(child, payload / child.name, follow_symlinks=False)
    runtime = runtime_closure(payload, request["target"])
    shutil.copyfile(approval_path, payload / "RUNTIME-QUALIFICATION.json")
    result = {"schema": 1, "kind": "backplane-bend-service", "status": "VERIFIED", "target": request["target"],
              "sourceCommit": release["commit"], "sourceTree": release["tree"], "version": VERSION, "channel": "release",
              "genuineService": True, "entrypoint": "src/server/main.bend", "roles": {"service": "backplane-serve", "web": "web"},
              "buildReceipt": local_reference(payload / "BUILD.json", out), "runtimeManifest": "RUNTIME.json",
              "runtimeManifestSha256": digest(payload / "RUNTIME.json"), "closureManifest": "CLOSURE.json",
              "closureSha256": digest(payload / "CLOSURE.json"), "packages": runtime["packages"],
              "members": inventory(payload), "runtimeAcceptance": "PENDING", "published": False}
    notices(payload, result["packages"])
    write(out / "SERVICE.json", result)
    return {"status": "VERIFIED_COMPONENT_NOT_INSTALLED", "receipt": reference(out / "SERVICE.json"), "payload": str(payload)}


def main():
    global ROOT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("describe", "emit", "native", "build", "js", "component"))
    parser.add_argument("--request", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--source-root", type=Path,
                        help="Explicit frozen product checkout when executing a separately pinned helper")
    args = parser.parse_args()
    if args.source_root is not None:
        ROOT = args.source_root.resolve(strict=True)
        require(ROOT.is_dir(), "Product source root must be a directory")
    if args.operation == "describe":
        print(json.dumps({"interface": "backplane-service-release-producer-v1", "version": VERSION,
                          "targets": sorted(TARGETS), "jsTargets": sorted(JS_TARGETS),
                          "build": "build --request REQUEST.json --out NEW_ABS_BUILD",
                          "js": "js --request REQUEST.json --out NEW_ABS_BUILD",
                          "emit": "emit --request REQUEST.json --out NEW_ABS_PACKET",
                          "native": "native --request REQUEST.json --out NEW_ABS_BUILD",
                          "component": "component --request REQUEST.json --out NEW_ABS_COMPONENT",
                          "publication": False}, indent=2))
        return
    require(args.request is not None and args.out is not None, "--request and --out required")
    request_path = args.request.resolve()
    print(json.dumps(globals()[args.operation](read(request_path), request_path.parent, args.out), indent=2))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, subprocess.SubprocessError) as exc:
        # No command/env dumps: compiler configuration is protected input.
        print("Service producer: " + (str(exc) if isinstance(exc, ValueError) else type(exc).__name__), file=sys.stderr)
        sys.exit(1)
