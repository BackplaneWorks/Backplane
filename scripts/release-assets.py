#!/usr/bin/env python3
"""Collect/verify immutable candidate bytes. Does not certify product gates or publish."""
import argparse
import gzip
import io
import posixpath
import tarfile
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import zipfile

SOURCE_RE = re.compile(r"[0-9a-f]{40}")
TAG_RE = re.compile(r"v(0|[1-9][0-9]{0,3})\.(0|[1-9][0-9]{0,3})\.(0|[1-9][0-9]{0,3})(-nightly\.[0-9]{8}\.[1-9][0-9]*)?")

# Coverage is independent of archive format and producer implementation. A local
# Linux desktop receipt cannot stand in for another desktop, web, or mobile row.
RELEASE_TARGETS = {
    "linux-x64": "desktop", "linux-arm64": "desktop",
    "darwin-arm64": "desktop", "windows-x64": "desktop",
    "web": "web", "android": "mobile", "ios": "mobile",
}
COMMON_GATES = {"runtime", "auth-security", "product-parity",
                "response-content", "telemetry-privacy"}

# Root accepted these specific component source scopes after the server-only
# telemetry repair. This is not an arbitrary source override or runtime waiver.
COMPONENT_DISPOSITION_SHA256 = "10961e9bfa190b09cec6b79ee5028e5ba9657ed5ef54513ecc20007c4d104b29"
CORRECTED_SERVICE_SOURCE = ("8147b68dbc9bcddad7e3b51ae7768ed9eb211ecc",
                            "37c64023d084bca7f209db9d14a84f59d365f696")
BROWSER_DISPOSITION_SHA256 = "02939bb0ba7e6e7975e1c199b9e4394d8871f5b7fb8e6a95d5c25fbf9d09720e"
# Exact root-owned browser-overlay policy; a claimed opaque hash is insufficient.
SELECTED_BROWSER_STAGING_DISPOSITION_SHA256 = "fee94f08b3ce7d909a1265d9f33820ea816887703e940ca0d7a57374f9509314"
SELECTED_BROWSER_SOURCE = ("a8844dba0626c491101508b632050c338083daa9",
                           "297f732988e715d9913eff2de4435c5b290bcf22")
SELECTED_BROWSER_MANIFEST = {"sha256": "ca23fead7805ef8e4c386b17aa2d06b1a9caaff4ad121fd9ddf2b045274ec3cc", "bytes": 2027}
SELECTED_WEB_ARCHIVE = {"sha256": "12a76c49dda3995dee6b2690b9cc62de761824b8cacc44d20bca97bd85d5f5e8", "bytes": 1436883}
RETAINED_COMPONENT_SOURCE = ("920c5a399dc7880bb3c6518239844570f116b3c7",
                             "02b851c54fff33a28e9d1cc60298f6527b78dd32")


def deferred_gates(tag):
    if tag != "v0.12.0":
        return {}
    return {"telemetry-privacy": {
        "state": "deferred-by-explicit-user-order",
        "requiredForThisPublication": False,
        "accepted": False,
        "reason": "2026-10-21: user halted telemetry work and validation to ship 0.12.0",
    }}


def target_gates(surface, tag):
    gates = COMMON_GATES | ({"package-identity"} if surface != "web" else set())
    return (gates | ({"os-sandbox"} if surface == "desktop" else set())) - set(deferred_gates(tag))


def version_identity(tag):
    if not isinstance(tag, str) or not TAG_RE.fullmatch(tag):
        raise ValueError("full source commit and stable/nightly package tag required")
    if len(tag[1:]) > 40 or ("-nightly." in tag and len(tag.split("-nightly.", 1)[1]) > 24):
        raise ValueError("package version exceeds native stamp or telemetry suffix limit")
    return tag[1:], "nightly" if "-nightly." in tag else "release"


def identity(source, tag):
    if not isinstance(source, str) or not SOURCE_RE.fullmatch(source):
        raise ValueError("full source commit and stable/nightly package tag required")
    return version_identity(tag)


def coverage_plan(source, tag):
    version, channel = identity(source, tag) if source is not None else version_identity(tag)
    return {
        "schema": 1, "kind": "backplane-release-coverage",
        "sourceCommit": source, "tag": tag, "version": version, "channel": channel,
        "deferredGates": deferred_gates(tag),
        "files": {},
        "targets": {name: {"surface": surface, "state": "pending", "artifacts": [],
                           "gates": {gate: None for gate in sorted(target_gates(surface, tag))}}
                    for name, surface in RELEASE_TARGETS.items()},
        "scope": "Artifact and owner-receipt bindings only; root reviews actual gate scope and alone publishes. No builds or product acceptance from this template.",
    }


def bound_coverage_file(directory, name, item):
    # Do not follow manifest-controlled paths outside the candidate directory.
    if (not isinstance(name, str) or not name or "\\" in name or ":" in name
            or name.startswith("/") or any(p in ("", ".", "..") for p in name.split("/"))):
        raise ValueError("unsafe coverage file path")
    path = directory
    for part in name.split("/"):
        path = path / part
        if path.is_symlink():
            raise ValueError("coverage evidence must not use symlinks")
    if (not isinstance(item, dict) or item.get("kind") not in ("asset", "owner-receipt")
            or type(item.get("bytes")) is not int or item["bytes"] <= 0
            or not isinstance(item.get("sha256"), str)
            or not re.fullmatch(r"[0-9a-f]{64}", item["sha256"])):
        raise ValueError("invalid coverage file binding")
    targets = item.get("targets")
    if (not isinstance(targets, list) or not targets
            or not all(isinstance(t, str) and t in RELEASE_TARGETS for t in targets)
            or len(set(targets)) != len(targets)):
        raise ValueError("coverage file requires explicit target scopes")
    if not path.is_file() or item["bytes"] != path.stat().st_size or digest(path) != item["sha256"]:
        raise ValueError("coverage asset or receipt changed")


def verify_coverage(directory, manifest, source, tag):
    """Check declared coverage and immutable references, not receipt truth."""
    version, channel = identity(source, tag)
    if not isinstance(manifest, dict):
        raise ValueError("coverage manifest must be an object")
    if (manifest.get("schema"), manifest.get("kind"), manifest.get("sourceCommit"),
            manifest.get("tag"), manifest.get("version"), manifest.get("channel")) != (
            1, "backplane-release-coverage", source, tag, version, channel):
        raise ValueError("coverage source/version/channel mismatch")
    if manifest.get("deferredGates", {}) != deferred_gates(tag):
        raise ValueError("coverage deferral does not match explicit user release scope")
    targets, files = manifest.get("targets"), manifest.get("files")
    if not isinstance(targets, dict) or set(targets) != set(RELEASE_TARGETS):
        raise ValueError("coverage requires every desktop, web and mobile target exactly once")
    if not isinstance(files, dict) or not files:
        raise ValueError("coverage requires actual asset and owner-receipt bindings")
    if not all(isinstance(name, str) for name in files) or len({name.casefold() for name in files}) != len(files):
        raise ValueError("duplicate or case-colliding coverage file paths")
    for name, item in files.items():
        bound_coverage_file(directory, name, item)
    used = set()
    for name, surface in RELEASE_TARGETS.items():
        row = targets[name]
        if not isinstance(row, dict) or row.get("surface") != surface or row.get("state") != "accepted":
            raise ValueError("target owner acceptance remains pending: " + name)
        artifacts, gates = row.get("artifacts"), row.get("gates")
        if (not isinstance(artifacts, list) or not artifacts
                or not all(isinstance(a, str) for a in artifacts)
                or len(set(artifacts)) != len(artifacts)):
            raise ValueError("target requires explicit, unique artifact references: " + name)
        if not isinstance(gates, dict) or set(gates) != target_gates(surface, tag):
            raise ValueError("target gate coverage incomplete: " + name)
        for reference in artifacts:
            if (reference not in files or files[reference]["kind"] != "asset"
                    or name not in files[reference]["targets"]):
                raise ValueError("target artifact reference missing or wrong kind")
        for reference in gates.values():
            if (not isinstance(reference, str) or reference not in files
                    or files[reference]["kind"] != "owner-receipt"
                    or name not in files[reference]["targets"]):
                raise ValueError("target gate requires an actual owner-receipt reference")
        used.update(artifacts)
        used.update(gates.values())
    if used != set(files):
        raise ValueError("unreferenced coverage file binding")


def check_bake(env, channel):
    # Validate presence/format only. Actual server/key agreement is an ingest gate.
    # Unbaked CI never calls this release-only preflight; development generators
    # retain their intentionally empty shared-client defaults.
    if env.get("BACKPLANE_GOOGLE_BAKE") != "1":
        raise ValueError("final release compile requires shared Google bake enabled")
    if not all(env.get(name, "").strip() for name in
               ("BACKPLANE_GOOGLE_CLIENT_ID", "BACKPLANE_GOOGLE_CLIENT_SECRET")):
        raise ValueError("final release compile requires shared Google client input pair")
    if env.get("BACKPLANE_TELEMETRY_BAKE") != "1":
        raise ValueError("final release compile requires telemetry BAKE1")
    if env.get("BACKPLANE_TELEMETRY_URL") != "https://backplane.works/api/v1/events":
        raise ValueError("approved telemetry endpoint input missing or different")
    if not re.fullmatch(r"rel-[a-z0-9-]{1,28}", env.get("BACKPLANE_TELEMETRY_KID", "")):
        raise ValueError("release telemetry KID missing or invalid")
    if not re.fullmatch(r"[0-9a-f]{64}", env.get("BACKPLANE_TELEMETRY_KEY", "")):
        raise ValueError("release telemetry KEY missing or invalid")
    if env.get("BACKPLANE_TELEMETRY_CHANNEL", channel) != channel:
        raise ValueError("telemetry channel does not match immutable version")


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


# Electron candidate input is a root-selected index of producer archives and
# receipts, not an instruction to build, install, sign or publish anything.
FONT_EXT = {".ttf", ".otf", ".ttc", ".otc", ".woff", ".woff2", ".dfont", ".pfb", ".pfa"}
FONT_MAGIC = (b"wOFF", b"wOF2", b"OTTO", b"ttcf", b"\x00\x01\x00\x00", b"%!PS-AdobeFont")
PRIVATE_NAMES = {"bp_google.h", "bp_telemetry.h", ".env", "signing.env"}


def safe_name(name):
    if (not isinstance(name, str) or not name or "\\" in name or ":" in name
            or name.startswith("/") or any(x in ("", ".", "..") for x in name.split("/"))
            or any(ord(c) < 32 for c in name)):
        raise ValueError("unsafe candidate member path")
    return name


def load_json(data):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate JSON field")
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=unique)


def file_binding(directory, item):
    if not isinstance(item, dict):
        raise ValueError("producer file binding required")
    name = safe_name(item.get("path"))
    bound_coverage_file(directory, name, dict(item, kind="asset", targets=["web"]))
    return name


class Package:
    """Inspect without extracting. Preserve audited internal framework symlinks."""
    def __init__(self, path):
        self.handle = None
        self.entries = {}
        self.links = {}
        self._inventory = None
        if str(path).endswith((".zip", ".apk", ".aab", ".ipa")):
            self.handle = zipfile.ZipFile(path)
            members = [(x.filename.rstrip("/"), x.is_dir(),
                        ((x.external_attr >> 16) & 0o170000) == 0o120000, x)
                       for x in self.handle.infolist()]
            self.open_member = self.handle.open
        elif str(path).endswith(".tar.gz"):
            self.handle = tarfile.open(path, "r:gz")
            members = []
            for x in self.handle.getmembers():
                if not (x.isdir() or x.isfile() or x.issym()):
                    raise ValueError("archive contains unsupported special member")
                members.append((x.name.rstrip("/"), x.isdir(), x.issym(), x))
            self.open_member = self.handle.extractfile
        else:
            raise ValueError("unsupported producer archive format")
        folded = set()
        for name, directory, symlink, member in members:
            safe_name(name)
            if name.casefold() in folded:
                raise ValueError("duplicate or case-colliding archive member")
            folded.add(name.casefold())
            if directory:
                continue
            self.entries[name] = member
            if symlink:
                target = (member.linkname if isinstance(member, tarfile.TarInfo)
                          else self.read(name, limit=4096).decode("utf-8"))
                if not target or target.startswith("/") or "\\" in target or ":" in target:
                    raise ValueError("unsafe archive symlink")
                resolved = posixpath.normpath(posixpath.join(posixpath.dirname(name), target))
                safe_name(resolved)
                # Links may not leave their producer's top-level archive root.
                if resolved.split("/")[0] != name.split("/")[0]:
                    raise ValueError("archive symlink escapes package root")
                self.links[name] = target
        if any(name.startswith(link + "/") for link in self.links for name in self.entries):
            raise ValueError("archive member traverses a symlink parent")
        for name in self.links:
            current = self.link_destination(name)
            if current not in self.entries and not any(x.startswith(current + "/") for x in self.entries):
                raise ValueError("archive symlink target absent")

    def link_destination(self, name):
        # Framework links often name a file below a linked directory, such as
        # Versions/Current/Foo with Current -> A. Walk components in filesystem
        # order so links are followed before a subsequent '..' is interpreted.
        root = name.split("/")[0]
        pending = name.split("/")[::-1]
        resolved = []
        traversals = 0
        while pending:
            part = pending.pop()
            if part in ("", "."):
                continue
            if part == "..":
                if len(resolved) <= 1:
                    raise ValueError("archive symlink escapes package root")
                resolved.pop()
                continue
            resolved.append(part)
            current = "/".join(resolved)
            if resolved[0] != root:
                raise ValueError("archive symlink escapes package root")
            if current in self.links:
                traversals += 1
                if traversals > 40:
                    raise ValueError("archive symlink cycle or depth limit")
                resolved.pop()
                pending.extend(self.links[current].split("/")[::-1])
        current = "/".join(resolved)
        safe_name(current)
        return current

    def close(self):
        if self.handle:
            self.handle.close()

    def read(self, name, limit=8 * 1024 * 1024):
        if name not in self.entries or name in self.links:
            raise ValueError("required package member absent or symlinked")
        with self.open_member(self.entries[name]) as stream:
            data = stream.read(limit + 1)
        if len(data) > limit:
            raise ValueError("package metadata exceeds bounded read")
        return data

    def mode(self, name):
        member = self.entries[name]
        return (member.mode if isinstance(member, tarfile.TarInfo) else member.external_attr >> 16) & 0o777

    def inventory(self):
        if self._inventory is not None:
            return self._inventory
        result = {}
        for name in self.entries:
            parts = name.lower().split("/")
            if (Path(name).suffix.lower() in FONT_EXT or parts[-1] in PRIVATE_NAMES
                    or any(x in {"fonts", "fontconfig", "font-cache", "glyph-cache", "glyph-cache-v2", ".cache", ".git", "__pycache__"} for x in parts)
                    or parts[-1].startswith(".env.") or Path(name).suffix.lower() in {".p8", ".p12", ".pfx", ".pem", ".key", ".keystore", ".h", ".c"}):
                raise ValueError("font, cache or private compile/signing input in package")
            if name in self.links:
                result[name] = {"symlink": self.links[name]}
                continue
            member = self.entries[name]
            mode = member.mode if isinstance(member, tarfile.TarInfo) else member.external_attr >> 16
            if mode & 0o7000:
                raise ValueError("archive cannot qualify privileged sandbox ownership through mode bits")
            sha, size, prefix = hashlib.sha256(), 0, b""
            with self.open_member(self.entries[name]) as stream:
                while block := stream.read(1024 * 1024):
                    if not prefix:
                        prefix = block[:64]
                    sha.update(block)
                    size += len(block)
            if name.endswith(".gz"):
                if Path(name[:-3]).suffix.lower() in FONT_EXT:
                    raise ValueError("compressed font payload")
                with self.open_member(self.entries[name]) as stream:
                    with gzip.GzipFile(fileobj=stream) as compressed:
                        if compressed.read(64).startswith(FONT_MAGIC):
                            raise ValueError("compressed font format signature")
            if prefix.startswith(FONT_MAGIC):
                raise ValueError("font format signature in package")
            if Path(name).suffix.lower() in {".css", ".html", ".js", ".cjs", ".mjs"}:
                # Text is bounded by archive member size; inspect in chunks so a
                # huge generated bridge need not be retained as a second copy.
                with self.open_member(self.entries[name]) as stream:
                    tail = b""
                    while block := stream.read(1024 * 1024):
                        if re.search(rb"berkeley[\s_-]*mono", tail + block, re.I):
                            raise ValueError("prohibited font selection in package")
                        tail = block[-64:]
            result[name] = {"sha256": sha.hexdigest(), "bytes": size}
        self._inventory = result
        return result


def configured_service(receipt, version, channel):
    if (receipt.get("entrypoint", receipt.get("entry")) != "src/server/main.bend"
            or receipt.get("serviceVersion", receipt.get("version")) != version
            or receipt.get("versionStamp") != version
            or receipt.get("telemetryChannel") != channel):
        raise ValueError("final genuine service version/stamp/channel required")
    config = receipt.get("compileInputs", {})
    for label in ("google", "telemetry"):
        item = config.get(label, {})
        if item.get("requested") is not True or item.get("requiredInputsPresent") is not True:
            raise ValueError("final configured service requires native Google and telemetry bake")
    if config.get("telemetry", {}).get("endpoint") != "https://backplane.works/api/v1/events":
        raise ValueError("actual native compile endpoint differs from approved input")
    if (config.get("version"), config.get("versionStamp"), config.get("telemetryChannel")) != (version, version, channel):
        raise ValueError("native compile identity differs from package")


def require_signing(receipt, target, expected_signer=None):
    signing = receipt.get("signing", {})
    if target == "android":
        if (not isinstance(expected_signer, str) or not re.fullmatch(r"[0-9a-f]{64}", expected_signer)
                or signing.get("certificateSha256") != expected_signer
                or signing.get("signatureVerified") is not True or signing.get("debuggable") is not False):
            raise ValueError("explicit approved Android signer and non-debuggable Release required")
        # The existing installed cohort used a historical Debug-DN certificate.
        # Exact root-approved fingerprint continuity governs, not the DN text.
    else:
        if (not isinstance(expected_signer, str) or not re.fullmatch(r"[0-9a-f]{64}", expected_signer)
                or signing.get("certificateSha256") != expected_signer
                or signing.get("signatureVerified") is not True or signing.get("debuggable") is not False
                or not signing.get("teamId")):
            raise ValueError("verified approved iOS distribution signature required")
        products = signing.get("products", {})
        if set(products) != {"app", "island"}:
            raise ValueError("iOS app and extension signatures required")
        for key, product in products.items():
            if (product.get("signatureVerified") is not True or product.get("debuggable") is not False
                    or product.get("certificateSha256") != expected_signer or not product.get("profileUuid")
                    or product.get("bundleId") != receipt.get("package", "") + (".island" if key == "island" else "")):
                raise ValueError("iOS embedded profile/extension signature mismatch")
    number = receipt.get("buildNumber")
    if isinstance(number, bool) or not isinstance(number, (str, int)) or not re.fullmatch(r"[1-9][0-9]*", str(number)):
        raise ValueError("mobile monotonic build number required")
    if receipt.get("configuration") != "Release" or receipt.get("actualNativeVersion", receipt.get("nativeVersion")) != receipt["version"]:
        raise ValueError("mobile debug/old native version cannot be relabeled")


def desktop_acceptance(package, root, receipt, target, source, tree, version, *, payload_inventory=None):
    freeze_ref = receipt.get("release", {}).get("freezeReceipt", {})
    freeze_path = safe_name(freeze_ref.get("path"))
    freeze = package.read(root + "/" + freeze_path)
    if hashlib.sha256(freeze).hexdigest() != freeze_ref.get("sha256") or len(freeze) != freeze_ref.get("bytes"):
        raise ValueError("desktop freeze receipt changed")
    frozen = load_json(freeze)
    policy = frozen.get("signingPolicy", {}).get(target)
    if policy not in {"required", "unsigned-portable-allowed"}:
        raise ValueError("explicit target signing policy required")
    kinds = {"signing": "desktop-signing", "sandbox": "desktop-sandbox",
             "lifecycle": "desktop-lifecycle", "runtimeAcceptance": "desktop-runtime-acceptance"}
    proofs = {}
    for key, kind in kinds.items():
        ref = receipt.get(key, {})
        name = safe_name(ref.get("path"))
        raw = package.read(root + "/" + name)
        proof = load_json(raw)
        if (hashlib.sha256(raw).hexdigest(), len(raw), ref.get("status")) != (ref.get("sha256"), ref.get("bytes"), "PASS"):
            raise ValueError("desktop acceptance reference mismatch")
        if (proof.get("schema"), proof.get("kind"), proof.get("status"), proof.get("target"), proof.get("commit"), proof.get("tree"), proof.get("version")) != (1, kind, "PASS", target, source, tree, version):
            raise ValueError("desktop target-scoped acceptance missing")
        if proof.get("payloadManifestSha256") != receipt.get("payloadManifestSha256") or proof.get("freezeReceiptSha256") != freeze_ref["sha256"]:
            raise ValueError("desktop acceptance does not bind final payload/freeze")
        proofs[key] = proof
    if not isinstance(proofs["signing"].get("signed"), bool) or policy == "required" and not proofs["signing"]["signed"]:
        raise ValueError("desktop signing disposition mismatch")
    if proofs["sandbox"].get("osEnforced") is not True or "--no-sandbox" in proofs["sandbox"].get("flags", []):
        raise ValueError("production OS sandbox remains unqualified")
    life = proofs["lifecycle"]
    if any(life.get(k) is not True for k in ("nativeChildIdentity", "authBound", "singleInstance", "reconnect")):
        raise ValueError("desktop lifecycle unqualified")
    resident = {"mode": "resident-service", "managedStop": False, "managedUpdate": False}
    lifecycle = frozen.get("lifecyclePolicy", {}).get(target)
    if lifecycle == resident:
        if receipt.get("lifecyclePolicy") != resident:
            raise ValueError("desktop resident policy differs from root freeze")
        if any(life.get(k) is not True for k in ("startup", "closePreservesService")) or any(
                life.get(k) is not False for k in ("managedStopAdvertised", "managedUpdateAdvertised")):
            raise ValueError("resident service startup/close policy unqualified")
        if life.get("windowClose") != "resident-service" or life.get("shutdown") != "unsupported":
            raise ValueError("resident service may not claim managed stop")
    elif life.get("shutdown") != ("approved-service-protocol" if target == "windows-x64" else "owned-native-child"):
        raise ValueError("desktop shutdown contract unqualified")
    if target == "windows-x64" and life.get("nativeToPosixPaths") is not True:
        raise ValueError("Windows path mapping unqualified")
    if proofs["runtimeAcceptance"].get("scope") != "desktop-release":
        raise ValueError("local synthetic runtime cannot qualify desktop release")
    payload = {}
    for name, item in (package.inventory() if payload_inventory is None else payload_inventory).items():
        n = name[len(root) + 1:]
        if n in {"DESKTOP.json", "MANIFEST.json", "STAGE.json"} or n.startswith("PROVENANCE/"):
            continue
        payload[n] = ({"type": "symlink", "target": item["symlink"]} if "symlink" in item else
                      dict(item, type="file", mode=package.mode(name) if payload_inventory is None else item["mode"]))
    actual = hashlib.sha256(json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
    if actual != receipt.get("payloadManifestSha256"):
        raise ValueError("actual desktop payload differs from accepted payload")


def packaged_reference(package, root, ref):
    if (not isinstance(ref, dict) or not isinstance(ref.get("sha256"), str)
            or not re.fullmatch(r"[a-f0-9]{64}", ref["sha256"])
            or type(ref.get("bytes")) is not int or ref["bytes"] <= 0):
        raise ValueError("invalid packaged reference")
    raw = package.read(root + "/" + safe_name(ref.get("path")))
    if hashlib.sha256(raw).hexdigest() != ref["sha256"] or len(raw) != ref["bytes"]:
        raise ValueError("packaged reference changed")
    return load_json(raw)


def same_reference(left, right):
    return (isinstance(left, dict) and isinstance(right, dict)
            and all(left.get(k) == right.get(k) for k in ("sha256", "bytes")))


def payload_members(members):
    return {n: v for n, v in members.items()
            if n not in {"STAGE.json", "DESKTOP.json", "MANIFEST.json"} and not n.startswith("PROVENANCE/")}


def content_digest(members):
    return hashlib.sha256(json.dumps(members, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def original_resource_web(package, root, service_root, receipt, source, tree, tag):
    """Check original resource inventory, substituting only its declared old web hashes.

    Old web bytes are deliberately absent in an overlay. Their original manifest
    must still match original BUILD/SERVICE; every non-web member stays exact.
    This never asserts that old browser bytes are the installed browser.
    """
    raw = package.read(service_root + "/WEB.json")
    original = load_json(raw)
    sha = hashlib.sha256(raw).hexdigest()
    if receipt.get("resourceWeb") != {"sourceCommit": source, "sourceTree": tree, "webManifestSha256": sha}:
        raise ValueError("original resourceWeb identity changed")
    if (original.get("schema"), original.get("kind"), original.get("sourceCommit"),
            original.get("sourceTree"), original.get("entrypoint")) != (
            1, "matching-generated-web", source, tree, "src/web/index.html"):
        raise ValueError("original resource web source changed")
    old = original.get("members", {})
    if not isinstance(old, dict) or "index.html" not in old:
        raise ValueError("missing original resource web manifest")
    for name, member in old.items():
        safe_name(name)
        if (set(member) != {"sha256", "bytes"} or type(member["bytes"]) is not int
                or member["bytes"] <= 0 or not re.fullmatch(r"[a-f0-9]{64}", member["sha256"])):
            raise ValueError("invalid original resource web member")
    service = packaged_reference(package, root, receipt["components"]["service"])
    if (service.get("kind"), service.get("status"), service.get("sourceCommit"),
            service.get("sourceTree"), service.get("target"), service.get("entrypoint"),
            service.get("genuineService")) != (
            "backplane-bend-service", "VERIFIED", source, tree, receipt["target"], "src/server/main.bend", True):
        raise ValueError("original native SERVICE identity changed")
    if service.get("roles", {}).get("service") != ("backplane-serve.exe" if receipt["target"] == "windows-x64" else "backplane-serve"):
        raise ValueError("original SERVICE service role mismatch")
    declared = service.get("members", {})
    actual = {n[len(service_root) + 1:]: v for n, v in package.inventory().items()
              if n.startswith(service_root + "/") and not n.startswith(service_root + "/web/")}
    old_declared = {n[4:]: {k: v.get(k) for k in ("sha256", "bytes")}
                    for n, v in declared.items() if n.startswith("web/")}
    if old_declared != old:
        raise ValueError("original SERVICE web differs from resource manifest")
    native_declared = {n: v for n, v in declared.items() if not n.startswith("web/")}
    expected = {n: ({"symlink": v.get("target")} if v.get("type") == "symlink"
                    else {k: v.get(k) for k in ("sha256", "bytes")}) for n, v in native_declared.items()}
    if actual != expected:
        raise ValueError("original native SERVICE closure changed")
    # A Windows ZIP carries no POSIX modes; the desktop producer checks them where they exist.
    # TODO (fast-hub J4): with the Win32 JS runtime, bring back the Windows
    # service member closure check the C producer had (verify_windows_members)
    # for packages without a selected browser.
    if receipt["target"] != "windows-x64":
        for n, v in native_declared.items():
            if v.get("type") == "file" and v.get("mode") != package.mode(service_root + "/" + n):
                raise ValueError("original native resource mode changed")
    build = packaged_reference(package, root, receipt["components"]["serviceBuild"])
    if (build != load_json(package.read(service_root + "/BUILD.json"))
            or not same_reference(service.get("buildReceipt"), receipt["components"]["serviceBuild"])):
        raise ValueError("original native BUILD differs from SERVICE provenance")


# Shared with the selected-browser producer/fixture; archive bounds, not new UI gates.
BROWSER_EVIDENCE_PREFIX = "PROVENANCE/selected-browser-evidence/"
BROWSER_EVIDENCE_FILE_LIMIT = 8 * 1024 * 1024
BROWSER_EVIDENCE_TOTAL_LIMIT = 32 * 1024 * 1024
BROWSER_EVIDENCE_COUNT_LIMIT = 16


def selected_browser_evidence(package, root, evidence):
    if not isinstance(evidence, list) or not 1 <= len(evidence) <= BROWSER_EVIDENCE_COUNT_LIMIT:
        raise ValueError("selected browser evidence count outside bounds")
    expected, folded, total = set(), set(), 0
    for ref in evidence:
        if not isinstance(ref, dict):
            raise ValueError("selected browser raw evidence reference invalid")
        name = safe_name(ref.get("path"))
        if not name.startswith(BROWSER_EVIDENCE_PREFIX):
            raise ValueError("selected browser evidence outside fixed namespace")
        basename = name[len(BROWSER_EVIDENCE_PREFIX):]
        if (not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", basename)
                or ".." in basename):
            raise ValueError("selected browser evidence filename invalid")
        if name.casefold() in folded:
            raise ValueError("duplicate or case-colliding selected browser evidence")
        folded.add(name.casefold())
        if (not isinstance(ref.get("sha256"), str) or not re.fullmatch(r"[a-f0-9]{64}", ref["sha256"])
                or type(ref.get("bytes")) is not int or not 1 <= ref["bytes"] <= BROWSER_EVIDENCE_FILE_LIMIT):
            raise ValueError("selected browser evidence hash/size outside bounds")
        total += ref["bytes"]
        if total > BROWSER_EVIDENCE_TOTAL_LIMIT:
            raise ValueError("selected browser evidence total bytes outside bounds")
        full = root + "/" + name
        entry = package.entries.get(full)
        regular = (entry.isfile() if isinstance(entry, tarfile.TarInfo) else
                   not entry.is_dir() and (entry.external_attr >> 16) & 0o170000 == 0o100000
                   if isinstance(entry, zipfile.ZipInfo) else False)
        parents = ["/".join(full.split("/")[:i]) for i in range(1, len(full.split("/")))]
        if (not regular or full in package.links or any(n in package.links or n in package.entries for n in parents)):
            raise ValueError("selected browser evidence must be a regular file without indirect parents")
        # Never resolve a host path or rewrite the immutable runtime envelope.
        raw = package.read(full, BROWSER_EVIDENCE_FILE_LIMIT)
        if len(raw) != ref["bytes"] or hashlib.sha256(raw).hexdigest() != ref["sha256"]:
            raise ValueError("selected browser raw evidence bytes changed")
        expected.add(full)
    actual = {n for n in package.entries if n.startswith(root + "/" + BROWSER_EVIDENCE_PREFIX)}
    if actual != expected:
        raise ValueError("selected browser evidence namespace has unreferenced or missing files")
    return expected


def selected_browser(package, root, service_root, receipt, target, version):
    selected = receipt["selectedBrowser"]
    freeze_ref = receipt.get("release", {}).get("freezeReceipt", {})
    freeze = packaged_reference(package, root, freeze_ref)
    pin = freeze.get("selectedBrowser", {})
    disposition = selected.get("rootStagingDisposition", {})
    if (not SELECTED_BROWSER_STAGING_DISPOSITION_SHA256
            or disposition.get("sha256") != SELECTED_BROWSER_STAGING_DISPOSITION_SHA256
            or disposition != freeze.get("rootStagingDisposition") or type(disposition.get("bytes")) is not int
            or disposition["bytes"] <= 0):
        raise ValueError("selected browser requires exact root staging disposition")
    if (freeze.get("schema"), freeze.get("kind"), freeze.get("status"), freeze.get("commit"), freeze.get("tree")) != (
            1, "backplane-release-freeze", "FROZEN", *CORRECTED_SERVICE_SOURCE):
        raise ValueError("selected browser native freeze changed")
    descriptor = dict(pin, rootStagingDisposition=disposition)
    if selected != descriptor or selected.get("mode") != "generated-web-subtree-only":
        raise ValueError("selected browser stage/freeze selection differs")
    if (selected.get("sourceCommit"), selected.get("sourceTree")) != SELECTED_BROWSER_SOURCE:
        raise ValueError("unapproved selected browser source")
    resource = receipt["resourceWeb"]
    if (selected.get("resourceWebManifestSha256") != resource["webManifestSha256"]
            or freeze.get("resourceWeb", {}).get(target) != resource
            or freeze.get("components", {}).get(target, {}).get("webManifestSha256") != resource["webManifestSha256"]):
        raise ValueError("selected browser changed original resourceWeb binding")
    policy_ref = receipt.get("components", {}).get("rootStagingDisposition")
    policy = packaged_reference(package, root, policy_ref)
    allowed = policy.get("targets", {}).get(target, {})
    release = receipt.get("release", {})
    if (not same_reference(policy_ref, disposition)
            or policy.get("kind") != "backplane-root-staging-disposition"
            or policy.get("status") != "AUTHORIZED_STAGING_POLICY_PENDING_ACTUAL_SERVICE_BINDINGS"
            or policy.get("publicationOwner") != "root" or policy.get("electronVersion") != "44.9.2"
            or any(policy.get(k) != release.get(k) for k in ("version", "tag", "channel", "commit", "tree"))
            or allowed.get("selectedBrowser") != pin
            or allowed.get("signingPolicy") != freeze.get("signingPolicy", {}).get(target)
            or allowed.get("lifecyclePolicy") != freeze.get("lifecyclePolicy", {}).get(target)
            or allowed.get("windowsShellSource") != freeze.get("windowsShellSource")
            or policy.get("compileInputs") != freeze.get("compileInputs")
            or policy.get("compileInputs") != load_json(package.read(service_root + "/BUILD.json")).get("compileInputs")):
        raise ValueError("actual root policy does not authorize this native/browser freeze")
    pins = freeze.get("components", {}).get(target, {})
    service_key = "serviceReceiptSha256"
    service_ref = receipt.get("components", {}).get("service", {})
    if (not allowed.get(service_key) or allowed[service_key] != pins.get(service_key)
            or allowed[service_key] != service_ref.get("sha256")):
        raise ValueError("actual root policy original native service/archive mismatch")
    for name in ("shell", "electron"):
        if allowed.get(name + "ReceiptSha256") != receipt.get("components", {}).get(name, {}).get("sha256"):
            raise ValueError("actual root policy native component mismatch")
    stage = packaged_reference(package, root, receipt.get("stageReceipt"))
    if (receipt["stageReceipt"]["sha256"] != receipt.get("stageReceiptSha256")
            or stage.get("kind") != "backplane-electron-desktop-stage" or stage.get("target") != target
            or stage.get("payloadManifestSha256") != receipt.get("payloadManifestSha256")
            or stage.get("selectedBrowser") != selected or stage.get("resourceWeb") != resource
            or stage.get("release") != receipt.get("release") or stage.get("components") != receipt.get("components")):
        raise ValueError("original current browser stage receipt changed")
    refs = receipt.get("components", {})
    browser = packaged_reference(package, root, refs.get("selectedBrowser"))
    manifest = packaged_reference(package, root, refs.get("selectedBrowserWeb"))
    if (not same_reference(refs["selectedBrowser"], selected.get("receipt"))
            or refs["selectedBrowserWeb"]["sha256"] != SELECTED_BROWSER_MANIFEST["sha256"]
            or refs["selectedBrowserWeb"]["bytes"] != SELECTED_BROWSER_MANIFEST["bytes"]
            or selected.get("webManifestSha256") != SELECTED_BROWSER_MANIFEST["sha256"]
            or receipt.get("webManifestSha256") != SELECTED_BROWSER_MANIFEST["sha256"]
            or freeze["components"][target].get("selectedBrowserReceiptSha256") != refs["selectedBrowser"]["sha256"]):
        raise ValueError("selected browser receipt/manifest pin changed")
    if (browser.get("schema"), browser.get("kind"), browser.get("status"), browser.get("version"),
            browser.get("channel"), browser.get("sourceCommit"), browser.get("sourceTree"),
            browser.get("entrypoint"), browser.get("webManifest"), browser.get("webRoot"),
            browser.get("webManifestSha256")) != (
            1, "backplane-selected-browser", "VERIFIED", version, "release", *SELECTED_BROWSER_SOURCE,
            "src/web/index.html", "WEB.json", "web", SELECTED_BROWSER_MANIFEST["sha256"]):
        raise ValueError("selected browser component identity changed")
    # Exact installed files and gzip remain independently checked from the archive.
    verify_web(package, dict(manifest, root=service_root + "/web"), *SELECTED_BROWSER_SOURCE)
    declared = browser.get("members", {})
    expected = {"WEB.json": SELECTED_BROWSER_MANIFEST,
                **{"web/" + n: v for n, v in manifest.get("members", {}).items()}}
    if ({n: {k: v.get(k) for k in ("sha256", "bytes")} for n, v in declared.items()} != expected
            or any(v.get("type") != "file" for v in declared.values())):
        raise ValueError("selected browser component member inventory changed")
    for n in manifest["members"]:
        if re.search(r"\.(exe|dll|so|dylib|node)$", n, re.I):
            raise ValueError("native member in selected browser")
        raw = package.read(service_root + "/web/" + n, 32 * 1024 * 1024)
        if raw.startswith((b"MZ", b"\x7fELF", bytes.fromhex("cffaedfe"), bytes.fromhex("cefaedfe"), bytes.fromhex("cafebabe"))):
            raise ValueError("native format in selected browser")
    packages = browser.get("packages")
    if not isinstance(packages, list) or not packages:
        raise ValueError("selected browser notices missing")
    for part in packages:
        if not isinstance(part.get("licenses"), list) or not part["licenses"]:
            raise ValueError("selected browser license missing")
        for license in part["licenses"]:
            raw = package.read(service_root + "/web/" + safe_name(license.get("path")))
            if not raw or hashlib.sha256(raw).hexdigest() != license.get("sha256"):
                raise ValueError("selected browser license changed")
    ref = (receipt.get("acceptance", {}).get("selectedBrowserRuntime")
           if receipt.get("acceptance") else receipt.get("selectedBrowserRuntime"))
    runtime = packaged_reference(package, root, ref)
    if ref.get("status") != "PASS":
        raise ValueError("selected browser runtime reference remains pending")
    if (runtime.get("schema"), runtime.get("kind"), runtime.get("scope"), runtime.get("status"),
            runtime.get("target"), runtime.get("version"), runtime.get("sourceCommit"), runtime.get("sourceTree")) != (
            1, "desktop-selected-browser-runtime", "same-target-selected-browser", "PASS", target, version, *SELECTED_BROWSER_SOURCE):
        raise ValueError("fresh same-target selected browser runtime required")
    if (runtime.get("payloadManifestSha256") != receipt.get("payloadManifestSha256")
            or runtime.get("freezeReceiptSha256") != freeze_ref["sha256"]
            or runtime.get("stageReceiptSha256") != receipt.get("stageReceiptSha256")
            or not re.fullmatch(r"[a-f0-9]{64}", str(receipt.get("stageReceiptSha256", "")))
            or runtime.get("selectedBrowserReceiptSha256") != refs["selectedBrowser"]["sha256"]
            or runtime.get("webManifestSha256") != SELECTED_BROWSER_MANIFEST["sha256"]):
        raise ValueError("browser runtime must bind current stage/payload/freeze/browser")
    if any(runtime.get("checks", {}).get(k) is not True for k in (
            "authenticatedOrigin", "newAssetsLoaded", "routeRetention", "draftRetention", "inlineBoundedMore", "explicitRetry")):
        raise ValueError("selected browser runtime behavior incomplete")
    sandbox = runtime.get("rendererSandbox", {})
    flags = sandbox.get("flags")
    if (sandbox.get("processType") != "renderer" or sandbox.get("osEnforced") is not True
            or not isinstance(flags, list) or any(not isinstance(f, str) or f.split("=")[0] in {
                "--no-sandbox", "--disable-sandbox", "--disable-setuid-sandbox", "--disable-seccomp-filter-sandbox", "--disable-namespace-sandbox", "--disable-gpu-sandbox"} for f in flags)):
        raise ValueError("selected browser actual renderer OS sandbox required")
    selected_browser_evidence(package, root, runtime.get("evidence"))


def composite_acceptance(package, root, resources, receipt, target, source, tree, version):
    acceptance = receipt["acceptance"]
    if (target not in {"darwin-arm64", "windows-x64"}
            or acceptance.get("kind") != "desktop-browser-composite"
            or acceptance.get("scope") != "retained-native-baseline+selected-browser"
            or any(k in receipt for k in ("signing", "sandbox", "lifecycle", "runtimeAcceptance"))):
        raise ValueError("only Mac/Windows explicit composite may inherit native baseline proofs")
    baseline = acceptance.get("baseline", {})
    old = packaged_reference(package, root, baseline.get("desktop"))
    old_freeze = packaged_reference(package, root, baseline.get("freeze"))
    current_freeze = packaged_reference(package, root, receipt["release"]["freezeReceipt"])
    if (baseline["desktop"]["sha256"] != current_freeze["selectedBrowser"].get("baselineDesktopReceiptSha256")
            or acceptance.get("baselineDesktopSha256") != baseline["desktop"]["sha256"]
            or old.get("schema") != 1 or old.get("kind") != "backplane-electron-desktop-package"
            or old.get("target") != target or old.get("selectedBrowser") or old.get("acceptance")
            or old.get("release", {}).get("commit") != source or old["release"].get("tree") != tree
            or old["release"].get("version") != version
            or not same_reference(old.get("components", {}).get("freeze"), baseline["freeze"])):
        raise ValueError("original accepted desktop lineage differs from root pin")
    if (old_freeze.get("signingPolicy", {}).get(target) != current_freeze.get("signingPolicy", {}).get(target)
            or old.get("lifecyclePolicy") != receipt.get("lifecyclePolicy")
            or old.get("windowsShellSource") != receipt.get("windowsShellSource")):
        raise ValueError("composite changed native policies/source")
    immutable = {k: v for k, v in old.get("components", {}).items()
                 if k in {"shell", "electron", "service", "serviceBuild"}}
    if acceptance.get("immutableComponents") != immutable or acceptance.get("selectedBrowser") != receipt["selectedBrowser"]:
        raise ValueError("composite immutable component lineage changed")
    for key, ref in immutable.items():
        if not same_reference(ref, receipt.get("components", {}).get(key)):
            raise ValueError("composite original component changed")
    old_members = payload_members(old.get("members", {}))
    if (content_digest(old_members) != old.get("payloadManifestSha256")
            or acceptance.get("baselinePayloadManifestSha256") != old["payloadManifestSha256"]
            or acceptance.get("baselineFreezeReceiptSha256") != baseline["freeze"]["sha256"]):
        raise ValueError("baseline payload/freeze digest changed")
    current = {n[len(root) + 1:]: ({"type": "symlink", "target": v["symlink"]} if "symlink" in v
               else dict(v, type="file", mode=package.mode(n))) for n, v in package.inventory().items()}
    current = payload_members(current)
    web = resources[len(root) + 1:] + "/backplane-service/web/"
    nonweb = lambda members: {n: v for n, v in members.items() if not n.startswith(web)}
    if nonweb(old_members) != nonweb(current):
        raise ValueError("composite changed non-web payload bytes/modes")
    if content_digest(current) != receipt.get("payloadManifestSha256"):
        raise ValueError("composite actual current payload differs")
    # Relocate references only in memory. Original proof JSON and old DESKTOP
    # remain byte-exact, and keep their original payload and freeze identities.
    projected = dict(old, release=dict(old["release"], freezeReceipt=baseline["freeze"]))
    baseline_inventory = {}
    for n, v in old_members.items():
        baseline_inventory[root + "/" + n] = ({"symlink": v["target"]} if v.get("type") == "symlink"
                                             else {k: v[k] for k in ("sha256", "bytes", "mode")})
    for key in ("signing", "sandbox", "lifecycle", "runtimeAcceptance"):
        ref = baseline.get("proofs", {}).get(key)
        if not same_reference(ref, old.get(key)) or ref.get("status") != "PASS":
            raise ValueError("baseline proof reference differs from original desktop")
        projected[key] = ref
    desktop_acceptance(package, root, projected, target, source, tree, version, payload_inventory=baseline_inventory)


def verify_web(package, web, source, tree):
    if (web.get("schema"), web.get("kind"), web.get("sourceCommit"), web.get("sourceTree"),
            web.get("entrypoint")) != (1, "matching-generated-web", source, tree, "src/web/index.html"):
        raise ValueError("matching generated web source required")
    members = web.get("members", {})
    root = safe_name(web.get("root"))
    if "index.html" not in members or not members:
        raise ValueError("generated web index missing")
    actual = {p[len(root) + 1:]: x for p, x in package.inventory().items() if p.startswith(root + "/")}
    if actual != members:
        raise ValueError("web manifest does not match exact packaged members")
    for name in members:
        safe_name(name)
        if name.endswith((".html", ".js", ".css")) and name + ".gz" not in members:
            raise ValueError("packaged web text requires matching gzip")
        if name.endswith(".gz"):
            plain = name[:-3]
            if plain not in members:
                raise ValueError("orphan compressed web asset")
            raw = package.read(root + "/" + plain, 32 * 1024 * 1024)
            with gzip.GzipFile(fileobj=io.BytesIO(package.read(root + "/" + name, 32 * 1024 * 1024))) as stream:
                inflated = stream.read(len(raw) + 1)
            if inflated != raw:
                raise ValueError("packaged gzip differs from matching web")
    return root


def verify_target(directory, target, row, source, tree, tag, *, browser_required=False):
    version, channel = identity(source, tag)
    archive_name = file_binding(directory, row.get("archive"))
    receipt_name = file_binding(directory, row.get("receipt"))
    receipt = load_json((directory / receipt_name).read_bytes())
    package = Package(directory / archive_name)
    try:
        inventory = package.inventory()
        if target == "windows-x64" and package.links:
            raise ValueError("Windows package reparse/symlink indirection refused")
        if RELEASE_TARGETS[target] == "desktop":
            root = "backplane-" + target
            suffix = ".zip" if target == "windows-x64" else ".tar.gz"
            if Path(archive_name).name != f"backplane-{tag}-{target}{suffix}":
                raise ValueError("desktop archive name differs from producer v2")
            release = receipt.get("release", {})
            if (receipt.get("schema"), receipt.get("kind"), receipt.get("target"),
                    release.get("commit"), release.get("tree"), release.get("version"),
                    release.get("tag"), release.get("channel")) != (
                    1, "backplane-electron-desktop-package", target, source, tree, version, tag, channel):
                raise ValueError("desktop DESKTOP1 frozen identity mismatch")
            if package.read(root + "/DESKTOP.json") != (directory / receipt_name).read_bytes():
                raise ValueError("exposed DESKTOP receipt differs from archive")
            outer = load_json(package.read(root + "/MANIFEST.json"))
            members = outer.get("members", outer.get("files"))
            if not member_map_matches(members, inventory, root, {root + "/MANIFEST.json"}, package):
                raise ValueError("desktop MANIFEST differs from archive")
            if not member_map_matches(receipt.get("members"), inventory, root,
                                      {root + "/MANIFEST.json", root + "/DESKTOP.json"}, package):
                raise ValueError("desktop DESKTOP member closure mismatch")
            if (receipt.get("electronVersion"), receipt.get("serviceVersion"), receipt.get("shellVersion")) != ("44.9.2", version, version):
                raise ValueError("pinned Electron and genuinely versioned shell/service required")
            if target == "darwin-arm64":
                resources = root + "/Backplane.app/Contents/Resources"
                launcher = root + "/Backplane.app/Contents/MacOS/Backplane"
            else:
                resources = root + ("/resources" if target == "windows-x64" else "/runtime/resources")
                launcher = root + ("/Backplane.exe" if target == "windows-x64" else "/backplane-open")
            service_root = resources + "/backplane-service"
            service = service_root + ("/backplane-serve.exe" if target == "windows-x64" else "/backplane-serve")
            for member in (launcher, service, service_root + "/LICENSE.Backplane"):
                if member not in inventory or member in package.links:
                    raise ValueError("actual desktop launcher/service/license missing")
            app = load_json(package.read(resources + "/app/package.json"))
            if app.get("version") != version or app.get("main") != "main.cjs":
                raise ValueError("packaged shell package.json must have actual stable version")
            for basename in ("LICENSE", "LICENSES.chromium.html"):
                if not any(p.endswith("/" + basename) and p not in package.links and inventory[p].get("bytes", 0) > 0 for p in inventory):
                    raise ValueError("Electron redistribution notices missing")
            notices = package.read(root + "/NOTICES.json")
            if hashlib.sha256(notices).hexdigest() != receipt.get("noticesManifestSha256"):
                raise ValueError("desktop notices manifest mismatch")
            build = load_json(package.read(service_root + "/BUILD.json"))
            configured_service(build, version, channel)
            if (build.get("commit", build.get("source_commit")), build.get("tree", build.get("source_tree"))) != (source, tree):
                raise ValueError("service build source mismatch")
            # The service is the JS one (Bun); Windows has had no other since the C service went.
            if (build.get("serviceRuntime", "bun" if target != "windows-x64" else None) != "bun"
                    or build.get("service", posixpath.basename(service)) != posixpath.basename(service)):
                raise ValueError("desktop service is not the configured JS service of its target")
            web_data = package.read(service_root + "/WEB.json")
            selected = receipt.get("selectedBrowser")
            if browser_required and not selected:
                raise ValueError("root disposition requires selected browser on every desktop")
            if selected and not browser_required:
                raise ValueError("selected desktop browser requires new root component disposition")
            if selected:
                # Validate original native resource receipts before selecting new web.
                original_resource_web(package, root, service_root, receipt, source, tree, tag)
                selected_browser(package, root, service_root, receipt, target, version)
            else:
                if receipt.get("resourceWeb") or receipt.get("acceptance") or receipt.get("selectedBrowserRuntime"):
                    raise ValueError("unexpected browser lineage without selected browser")
                if hashlib.sha256(web_data).hexdigest() != receipt.get("webManifestSha256"):
                    raise ValueError("desktop web receipt changed")
                verify_web(package, dict(load_json(web_data), root=service_root + "/web"), source, tree)
            if target == "windows-x64":
                # The JS service has no Windows managed-control protocol: only the
                # resident policy (no managed stop or update) can ship, and the BUILD
                # may not claim one.
                resident = {"mode": "resident-service", "managedStop": False, "managedUpdate": False}
                if receipt.get("lifecyclePolicy") != resident:
                    raise ValueError("Windows managed service control remains unsupported")
                if "managedControl" in build:
                    raise ValueError("resident Windows policy requires truthful unsupported control")
            if selected and receipt.get("acceptance"):
                composite_acceptance(package, root, resources, receipt, target, source, tree, version)
            else:
                desktop_acceptance(package, root, receipt, target, source, tree, version)
        else:
            if (receipt.get("schema"), receipt.get("kind"), receipt.get("target"),
                    receipt.get("sourceCommit"), receipt.get("sourceTree"), receipt.get("version"),
                    receipt.get("channel")) != (1, "backplane-target-package", receipt.get("target") if target in {"android", "ios"} else target, source, tree, version, channel):
                raise ValueError("web/mobile producer source/version/channel mismatch")
            if target == "android" and receipt.get("target") not in {"android-apk", "android-aab"}:
                raise ValueError("Android target must identify APK or AAB")
            if target == "ios" and receipt.get("target") not in {"ios-ipa", "ios-xcarchive"}:
                raise ValueError("iOS target must identify device IPA or archive")
            declared_archive = receipt.get("archive", {})
            bound = dict(row["archive"])
            bound.pop("path", None)
            declared_hash = dict(declared_archive)
            declared_hash.pop("path", None)
            declared_hash.pop("name", None)
            declared_members = receipt.get("members")
            if isinstance(declared_members, list):
                records = declared_members
                declared_members = {x["path"]: ({"symlink": x["target"]} if x.get("type") == "symlink" else
                                               {k: x[k] for k in ("sha256", "bytes")}) for x in records}
                if len(declared_members) != len(records):
                    raise ValueError("duplicate mobile producer member")
            if declared_hash != bound or inventory != declared_members:
                raise ValueError("web/mobile receipt must bind the exact archive and members")
            if target == "web":
                verify_web(package, receipt["web"], source, tree)
            else:
                bridge = safe_name(receipt.get("bridgeMember"))
                if inventory.get(bridge, {}).get("sha256") != receipt.get("bridgeSha256"):
                    raise ValueError("mobile qualified bridge mismatch")
        if target in {"android", "ios"}:
            require_signing(receipt, target, row.get("expectedSignerSha256"))
            if target == "ios" and (not row.get("expectedPackageId") or not row.get("expectedTeamId")
                                    or receipt.get("package") != row["expectedPackageId"]
                                    or receipt.get("signing", {}).get("teamId") != row["expectedTeamId"]):
                raise ValueError("iOS package must preserve root-configured public bundle/team identity")
            floor = row.get("publishedBuildFloor")
            if isinstance(floor, bool) or not isinstance(floor, (str, int)) or not re.fullmatch(r"[0-9]+", str(floor)) or int(receipt["buildNumber"]) <= int(floor):
                raise ValueError("root-observed mobile build floor required")
    finally:
        package.close()
    used = {archive_name, receipt_name}
    checksum = row.get("checksum")
    if target in {"linux-x64", "linux-arm64", "darwin-arm64", "windows-x64"} and checksum is None:
        raise ValueError("desktop producer digest sidecar required")
    if checksum is not None:
        checksum_name = file_binding(directory, checksum)
        if (directory / checksum_name).read_text().split() != [row["archive"]["sha256"], Path(archive_name).name]:
            raise ValueError("producer archive digest sidecar mismatch")
        used.add(checksum_name)
    for item in row.get("support", []):
        name = file_binding(directory, item)
        if target not in {"linux-x64", "linux-arm64", "darwin-arm64", "windows-x64"}:
            raise ValueError("unsupported extra target output")
        member = "backplane-" + target + "/" + safe_name(item.get("member", Path(name).name))
        check = Package(directory / archive_name)
        try:
            if check.read(member) != (directory / name).read_bytes():
                raise ValueError("exposed producer support receipt differs from archive")
        finally:
            check.close()
        used.add(name)
    return used


def member_map_matches(declared, inventory, root, excluded, package=None):
    if not isinstance(declared, dict):
        return False
    actual = {p: v for p, v in inventory.items() if p not in excluded}
    normalized = {}
    for name, value in declared.items():
        safe_name(name)
        path = name if name.startswith(root + "/") else root + "/" + name
        if path in normalized or not isinstance(value, dict):
            return False
        # Modes are audited by the target producer. Aggregate binds bytes/links;
        # it does not infer production sandbox/signing from executable bits.
        if value.get("type") == "symlink":
            normalized[path] = {"symlink": value.get("target")}
        else:
            normalized[path] = {k: v for k, v in value.items() if k in {"sha256", "bytes", "symlink"}}
            if package is not None and "mode" in value and (path not in inventory or value["mode"] != package.mode(path)):
                return False
    return actual == normalized


def component_sources(directory, inputs, source, tree, tag):
    scopes = {target: {"commit": source, "tree": tree} for target in RELEASE_TARGETS}
    binding = inputs.get("componentProvenanceDisposition")
    declared = {t: row["componentSource"] for t, row in inputs["targets"].items()
                if isinstance(row, dict) and "componentSource" in row}
    if binding is None:
        if declared:
            raise ValueError("component source scopes require pinned root disposition")
        return scopes, set()
    name = file_binding(directory, binding)
    if (binding["sha256"] not in {COMPONENT_DISPOSITION_SHA256, BROWSER_DISPOSITION_SHA256}
            or (source, tree) != CORRECTED_SERVICE_SOURCE or tag != "v0.12.0"):
        raise ValueError("unapproved component provenance disposition")
    disposition = load_json((directory / name).read_bytes())
    if (disposition.get("schema"), disposition.get("kind"), disposition.get("status"),
            disposition.get("release"), disposition.get("correctedServiceSource")) != (
            1, "backplane-root-component-provenance-disposition",
            "SOURCE_SCOPES_ACCEPTED_RUNTIME_GATES_REMAIN", "v0.12.0",
            {"commit": source, "tree": tree}):
        raise ValueError("root component provenance source mismatch")
    if binding["sha256"] == BROWSER_DISPOSITION_SHA256:
        expected = {t: dict(zip(("commit", "tree"),
                    SELECTED_BROWSER_SOURCE if t == "web" else
                    RETAINED_COMPONENT_SOURCE if t in {"android", "ios"} else CORRECTED_SERVICE_SOURCE))
                    for t in RELEASE_TARGETS}
        if (disposition.get("targetComponentSources") != expected
                or declared != expected
                or disposition.get("selectedBrowserSource") != expected["web"]
                or disposition.get("selectedBrowserManifest") != SELECTED_BROWSER_MANIFEST
                or disposition.get("selectedBrowserWebArchive") != SELECTED_WEB_ARCHIVE
                or disposition.get("desktopSelectedBrowserRequired") != [t for t, surface in RELEASE_TARGETS.items() if surface == "desktop"]):
            raise ValueError("exact browser disposition roles and all seven source scopes required")
        return expected, {name}
    allowed = {"android", "ios", "web"}
    retained = dict(zip(("commit", "tree"), RETAINED_COMPONENT_SOURCE))
    for target, scope in declared.items():
        if target not in allowed or scope != retained:
            raise ValueError("component source scope not authorized: " + target)
        scopes[target] = scope
    return scopes, {name}


def verify_inputs(directory, inputs, source, tag):
    version, channel = identity(source, tag)
    if (inputs.get("schema"), inputs.get("kind"), inputs.get("sourceCommit"), inputs.get("tag"),
            inputs.get("version"), inputs.get("channel")) != (1, "backplane-electron-release-inputs", source, tag, version, channel):
        raise ValueError("Electron release input identity mismatch")
    if inputs.get("rootFrozen") is not True:
        raise ValueError("root source freeze is not declared")
    tree = inputs.get("sourceTree")
    if not isinstance(tree, str) or not SOURCE_RE.fullmatch(tree):
        raise ValueError("actual full frozen source tree required")
    rows = inputs.get("targets", {})
    if set(rows) != set(RELEASE_TARGETS):
        raise ValueError("all four desktops, web, Android and iOS packages required")
    scopes, used = component_sources(directory, inputs, source, tree, tag)
    for target, row in rows.items():
        if not isinstance(row, dict):
            raise ValueError("target producer archive and receipt required")
        component = scopes[target]
        browser_policy = inputs.get("componentProvenanceDisposition", {}).get("sha256") == BROWSER_DISPOSITION_SHA256
        if browser_policy and target == "web" and {k: row.get("archive", {}).get(k) for k in ("sha256", "bytes")} != SELECTED_WEB_ARCHIVE:
            raise ValueError("standalone browser archive differs from root selection")
        refs = verify_target(directory, target, row, component["commit"], component["tree"], tag,
                             browser_required=browser_policy and RELEASE_TARGETS[target] == "desktop")
        if target == "android":
            main = load_json((directory / row["receipt"]["path"]).read_bytes())
            variants = row.get("variants", [])
            if main.get("target") != "android-apk" or not isinstance(variants, list) or len(variants) != 1:
                raise ValueError("Android requires main APK and same-build AAB variant")
            variant = variants[0]
            if "componentSource" in variant:
                raise ValueError("Android variants must share their target component source")
            if variant.get("expectedSignerSha256") != row.get("expectedSignerSha256"):
                raise ValueError("Android APK/AAB approved signer differs")
            extra = verify_target(directory, target, variant, component["commit"], component["tree"], tag)
            other = load_json((directory / variant["receipt"]["path"]).read_bytes())
            if (other.get("target") != "android-aab" or other.get("buildNumber") != main.get("buildNumber")
                    or other.get("bridgeSha256") != main.get("bridgeSha256") or extra & refs):
                raise ValueError("Android APK/AAB tuple differs")
            refs |= extra
        if target == "ios":
            primary = load_json((directory / row["receipt"]["path"]).read_bytes())
            if primary.get("target") != "ios-ipa":
                raise ValueError("iOS release row requires actual device IPA")
        for binding in row.get("evidence", []):
            name = file_binding(directory, binding)
            if not name.endswith(".json") or binding["bytes"] > 8 * 1024 * 1024:
                raise ValueError("additional producer evidence must be bounded JSON")
            load_json((directory / name).read_bytes())
            if name in refs:
                raise ValueError("duplicate producer evidence reference")
            refs.add(name)
        if used & refs:
            raise ValueError("producer artifacts may not substitute for another target")
        used |= refs
    return used


def candidate_members(directory):
    found = set()
    for p in directory.rglob("*"):
        if p.is_symlink():
            raise ValueError("indirect candidate member")
        if p.is_dir():
            continue
        if not p.is_file():
            raise ValueError("nonregular candidate member")
        found.add(safe_name(p.relative_to(directory).as_posix()))
    if len({n.casefold() for n in found}) != len(found):
        raise ValueError("case-colliding candidate files")
    return found


def public_archives(inputs):
    return [row["archive"]["path"] for target in RELEASE_TARGETS
            for row in [inputs["targets"][target], *inputs["targets"][target].get("variants", [])]]


def collect(directory, source, tag, manifest, **unused):
    inputs = load_json(manifest.read_bytes())
    files = verify_inputs(directory, inputs, source, tag)
    if manifest.resolve().parent != directory.resolve() or manifest.name != "release-inputs.json":
        raise ValueError("canonical in-directory release-inputs.json required")
    files.add(manifest.name)
    if candidate_members(directory) != files:
        raise ValueError("unexpected or missing candidate files")
    version, channel = identity(source, tag)
    output = {"schema": 2, "kind": "backplane-electron-candidate", "sourceCommit": source,
              "sourceTree": inputs["sourceTree"], "tag": tag, "version": version, "channel": channel,
              "componentSources": component_sources(directory, inputs, source, inputs["sourceTree"], tag)[0],
              "buildRunId": os.environ.get("GITHUB_RUN_ID"),
              "publicAssets": public_archives(inputs),
              "files": {n: {"sha256": digest(directory / n), "bytes": (directory / n).stat().st_size} for n in sorted(files)},
              "productAcceptance": "PENDING all seven target-scoped owner gates; root alone publishes"}
    for name in ("release-assets.json", "SHA256SUMS", "release-coverage.json"):
        if (directory / name).exists():
            raise ValueError("collection must not overwrite inventory or acceptance")
    (directory / "release-assets.json").write_text(json.dumps(output, indent=2) + "\n")
    (directory / "release-coverage.json").write_text(json.dumps(coverage_plan(source, tag), indent=2) + "\n")
    names = files | {"release-assets.json", "release-coverage.json"}
    (directory / "SHA256SUMS").write_text("".join(f"{digest(directory / n)}  {n}\n" for n in sorted(names)))
    verify(directory, source, tag)


def verify(directory, source, tag, run_id=None):
    version, channel = identity(source, tag)
    manifest = load_json((directory / "release-assets.json").read_bytes())
    if (manifest.get("schema"), manifest.get("kind"), manifest.get("sourceCommit"), manifest.get("tag"),
            manifest.get("version"), manifest.get("channel")) != (2, "backplane-electron-candidate", source, tag, version, channel):
        raise ValueError("Electron candidate inventory identity mismatch")
    if run_id is not None and manifest.get("buildRunId") != run_id:
        raise ValueError("candidate collection run mismatch")
    inputs = load_json((directory / "release-inputs.json").read_bytes())
    names = verify_inputs(directory, inputs, source, tag) | {"release-inputs.json"}
    if manifest.get("componentSources") != component_sources(directory, inputs, source, inputs["sourceTree"], tag)[0]:
        raise ValueError("candidate component source scopes changed")
    if manifest.get("sourceTree") != inputs["sourceTree"] or set(manifest.get("files", {})) != names:
        raise ValueError("candidate inventory/source tree mismatch")
    if manifest.get("publicAssets") != public_archives(inputs):
        raise ValueError("public asset list must include every target primary and variant package")
    for n in names:
        if manifest["files"][n] != {"sha256": digest(directory / n), "bytes": (directory / n).stat().st_size}:
            raise ValueError("candidate bytes changed after collection")
    expected = names | {"release-assets.json", "release-coverage.json", "SHA256SUMS"}
    if candidate_members(directory) != expected:
        raise ValueError("unexpected or indirect candidate member")
    lines = (directory / "SHA256SUMS").read_text().splitlines()
    hashes = {}
    for line in lines:
        sha, name = line.split("  ", 1)
        if name in hashes or name not in expected - {"SHA256SUMS"} or not re.fullmatch(r"[0-9a-f]{64}", sha):
            raise ValueError("invalid aggregate checksum member")
        hashes[name] = sha
    if set(hashes) != expected - {"SHA256SUMS"} or any(digest(directory / n) != h for n, h in hashes.items()):
        raise ValueError("aggregate checksum inventory mismatch")


def verify_final(directory, qualification, manifest, source, tag):
    if tag != "v0.12.0":
        raise ValueError("current stable publication target is v0.12.0")
    verify(directory, source, tag)
    coverage = load_json(manifest.read_bytes())
    verify_coverage(qualification, coverage, source, tag)
    inputs = load_json((directory / "release-inputs.json").read_bytes())
    for target, row in inputs["targets"].items():
        archives = [row["archive"], *[v["archive"] for v in row.get("variants", [])]]
        bound = [coverage["files"][n] for n in coverage["targets"][target]["artifacts"]]
        if not all(any((x["sha256"], x["bytes"]) == (archive["sha256"], archive["bytes"]) for x in bound) for archive in archives):
            raise ValueError("accepted target does not bind sealed candidate archive: " + target)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["collect", "verify", "check-bake", "coverage-plan", "check-coverage", "check-inputs", "check-final"])
    parser.add_argument("--directory", type=Path)
    parser.add_argument("--source")
    parser.add_argument("--tag", required=True)
    parser.add_argument("--run-id")
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--tree")
    parser.add_argument("--qualification-directory", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "coverage-plan":
            result = json.dumps(coverage_plan(args.source, args.tag), indent=2) + "\n"
            if args.manifest:
                with args.manifest.open("x") as output:
                    output.write(result)
            else:
                print(result, end="")
            return
        _, channel = identity(args.source, args.tag)
        if args.command == "check-bake":
            check_bake(os.environ, channel)
        elif args.command == "check-final":
            if args.directory is None or args.qualification_directory is None or args.manifest is None:
                raise ValueError("candidate and qualification directories/manifest required")
            verify_final(args.directory, args.qualification_directory, args.manifest, args.source, args.tag)
        elif args.command in ("check-inputs", "check-coverage", "collect"):
            if args.directory is None or args.manifest is None:
                raise ValueError("candidate directory and explicit manifest required")
            if args.command == "check-coverage":
                verify_coverage(args.directory, load_json(args.manifest.read_bytes()), args.source, args.tag)
            elif args.command == "check-inputs":
                verify_inputs(args.directory, load_json(args.manifest.read_bytes()), args.source, args.tag)
            else:
                collect(args.directory, args.source, args.tag, args.manifest)
        else:
            if args.directory is None:
                raise ValueError("candidate directory required")
            verify(args.directory, args.source, args.tag, args.run_id)
    except (ValueError, KeyError, TypeError, AttributeError, OSError, EOFError, zipfile.BadZipFile, tarfile.TarError) as error:
        parser.exit(1, f"release-assets: {error}\n")
    print("PASS candidate inputs/bytes only; root must review actual gate evidence before publication")


if __name__ == "__main__":
    main()
