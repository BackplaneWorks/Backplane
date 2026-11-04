"""Native read-only ABI measurement; never assemble/promote a TEST BUILD."""
import ctypes
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import sys


cfg = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8-sig"))
out = Path(cfg["out"]) / "canonical-946.json"
receipt = {"status": "INCOMPLETE", "scope": "Canonical native system ABI only",
           "releaseBuildApproved": False, "originalTestOnlyMetadataChanged": False}


def require(ok, why):
    if not ok:
        raise ValueError(why)


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def git(root, *args):
    return subprocess.check_output([cfg["git"], "-c", "safe.directory=" + str(root.resolve()), "-C", str(root), *args])


try:
    require(sys.platform == "win32" and os.environ.get("GITHUB_ACTIONS") == "true", "GitHub native Windows only")
    require(platform.machine() == "AMD64", "Native AMD64 Python required")
    source = Path(cfg["frozenSourceRoot"])
    factory = Path(cfg["canonicalFactoryRoot"])
    factory_commit = "2823fe4641bbadcb41a6cf49f91fe2c7fbc77978"
    factory_tree = "6aee3595dfa1862952cfb28969f0a884a49ae8ff"
    factory_sha = "b1f159e4304d82577a55465891c423e0e9c3fbce063696529ac15ef2a374b823"
    require(git(factory, "rev-parse", "HEAD").decode().strip() == factory_commit, "Wrong canonical factory commit")
    require(git(factory, "rev-parse", "HEAD^{tree}").decode().strip() == factory_tree, "Wrong canonical factory tree")
    require(git(source, "rev-parse", "HEAD").decode().strip() == cfg["sourceCommit"], "Wrong frozen product commit")
    require(git(source, "rev-parse", "HEAD^{tree}").decode().strip() == cfg["sourceTree"], "Wrong frozen product tree")
    tool = factory / "scripts" / "service-runtime.py"
    receipt.update(factorySHA256Observed=digest(tool), factoryCRLFCount=tool.read_bytes().count(b"\r\n"))
    require(digest(tool) == factory_sha, "Canonical verifier hash mismatch")
    require(digest(cfg["packagePath"]) == cfg["packageSHA256"], "ZIP changed")
    service = Path(cfg["executable"]).parent / "resources" / "backplane-service"
    binary = service / "backplane-serve.exe"
    binary_sha = "11f53c9619d095e0ccd520c5616743155ee240b9637021d4d5a0bea4707aadd1"
    require(digest(binary) == binary_sha, "Measured executable differs from retained service")
    build = service / "BUILD.json"
    original_build_sha = digest(build)
    original = json.loads(build.read_text())
    require(original["testOnly"] and not original["releaseQualified"], "Original TEST BUILD provenance required")
    require(original["buildProvenance"] == cfg["buildProvenance"], "Original compiler provenance differs")
    names, members = [], {}
    for row in git(source, "ls-tree", "-rz", "--full-tree", "HEAD").split(b"\0"):
        if not row:
            continue
        meta, raw_name = row.split(b"\t", 1)
        name = raw_name.decode("utf-8")
        if name.startswith("src/") and name.endswith(".js"):
            data = (source / name).read_bytes()
            blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
            require(blob == meta.split()[2].decode(), "Frozen JS bytes changed: " + name)
            names.append(name)
            members[name] = hashlib.sha256(data).hexdigest()
    spec = importlib.util.spec_from_file_location("canonical_native_946", tool)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    # Invoke the actual merged functions unchanged. Do not call prepare(),
    # substitute a DLL catalog, or promote the test-only BUILD's status.
    dynamic = module.ffi_libraries(source, names, "windows-x64")
    measurement = module.windows_measure(binary, "windows-x64", dynamic)
    require(any(item["name"] == "msvcrt.dll" and "src/server/effects/zip.js" in item["declaredBy"] for item in dynamic), "Actual frozen zip.js CRT declaration missing")
    system32 = module.windows_system_directory()
    crt = system32 / "msvcrt.dll"
    pe = module.pe(crt.read_bytes())
    library = ctypes.WinDLL(str(crt), winmode=0x800)
    kernel = ctypes.WinDLL("kernel32.dll", winmode=0x800)
    get_name = kernel.GetModuleFileNameW
    get_name.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_uint]
    get_name.restype = ctypes.c_uint
    buffer = ctypes.create_unicode_buffer(32768)
    count = get_name(library._handle, buffer, len(buffer))
    require(0 < count < len(buffer), "Loaded image path unavailable")
    loaded = Path(buffer.value).resolve(strict=True)
    require(loaded == crt.resolve(strict=True), "Loaded CRT differs from measured native System32 file")
    export = getattr(library, "_get_osfhandle")
    address = ctypes.cast(export, ctypes.c_void_p).value
    require(address, "Real _get_osfhandle export unavailable")
    require(any(edge["name"] == "msvcrt.dll" and edge.get("kind") == "dynamic" and Path(edge["resolvedSystemPath"]).resolve() == loaded for edge in measurement["dependencyGraph"]), "Canonical dynamic edge missing")
    require(digest(build) == original_build_sha and digest(binary) == binary_sha, "Measurement changed package bytes")
    receipt.update(status="PASS", factoryCommit=factory_commit, factoryTree=factory_tree,
                   factorySHA256=factory_sha, productCommit=cfg["sourceCommit"], productTree=cfg["sourceTree"],
                   packageSHA256=cfg["packageSHA256"], serviceSHA256=binary_sha,
                   originalBuildSHA256=original_build_sha, buildProvenance=original["buildProvenance"],
                   hostedRun=cfg["hostedRun"],
                   sourceJS=members, canonicalMeasurement=measurement,
                   msvcrtObservation={"nativeSystemDirectory": str(system32), "measuredPath": str(crt),
                                      "loadedImagePath": str(loaded), "sha256": digest(crt),
                                      "peMachine": pe["machine"], "export": "_get_osfhandle",
                                      "exportAddress": hex(address), "exportInvoked": False},
                   python={"version": sys.version, "executable": sys.executable, "machine": platform.machine(), "utf8Mode": sys.flags.utf8_mode})
except Exception as error:
    receipt.update(status="FAIL", error=f"{type(error).__name__}: {error}")
finally:
    out.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
if receipt["status"] != "PASS":
    raise SystemExit(1)
