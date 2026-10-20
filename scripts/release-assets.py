#!/usr/bin/env python3
"""Collect/verify immutable candidate bytes. Does not certify product gates or publish."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import zipfile

SOURCE_RE = re.compile(r"[0-9a-f]{40}")
TAG_RE = re.compile(r"v(0|[1-9][0-9]{0,3})\.(0|[1-9][0-9]{0,3})\.(0|[1-9][0-9]{0,3})(-nightly\.[0-9]{8}\.[1-9][0-9]*)?")


def identity(source, tag):
    if not SOURCE_RE.fullmatch(source) or not TAG_RE.fullmatch(tag):
        raise ValueError("full source commit and stable/nightly package tag required")
    if len(tag[1:]) > 40 or ("-nightly." in tag and len(tag.split("-nightly.", 1)[1]) > 24):
        raise ValueError("package version exceeds native stamp or telemetry suffix limit")
    return tag[1:], "nightly" if "-nightly." in tag else "release"


def check_bake(env, channel):
    # Validate presence/format only. Actual server/key agreement is an ingest gate.
    # Unbaked CI never calls this release-only preflight; development generators
    # retain their intentionally empty shared-client defaults.
    if env.get("BACKPLANE_GOOGLE_BAKE", "1") != "1":
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


def payload_names(directory, tag):
    choices = [f"backplane-{tag}-linux-x64{suffix}.tar.gz" for suffix in ("", "-cef")]
    archives = [name for name in choices if (directory / name).is_file()]
    if len(archives) != 1:
        raise ValueError("exactly one Linux x64 archive variant required")
    version = tag[1:].replace("-", "~")
    return [archives[0], f"backplane-{tag}-linux-arm64.tar.gz",
            f"backplane-{tag}-darwin-arm64.tar.gz",
            f"backplane-{tag}-linux-x64.AppImage", f"backplane-{tag}-linux-arm64.AppImage",
            f"backplane_{version}_amd64.deb", f"backplane_{version}_arm64.deb",
            f"backplane-{tag}-windows-x64.zip", f"backplane-{tag}-windows-x64.zip.sha256"]


def require_files(directory, names):
    if {p.name for p in directory.iterdir()} != set(names):
        raise ValueError("missing or unexpected candidate files")
    for name in names:
        p = directory / name
        if p.is_symlink() or not p.is_file() or p.stat().st_size == 0:
            raise ValueError("candidate files must be nonempty regular files")


def windows_identity(directory, source, tag, channel):
    archive = directory / f"backplane-{tag}-windows-x64.zip"
    if archive.with_suffix(".zip.sha256").read_text().split() != [digest(archive), archive.name]:
        raise ValueError("Windows checksum does not match ZIP")
    with zipfile.ZipFile(archive) as z:
        build = json.loads(z.read("backplane-windows-x64/BUILD.json"))
        emitted = json.loads(z.read("backplane-windows-x64/SOURCE.json"))
    for receipt in (build, emitted):
        if receipt.get("commit") != source or str(receipt.get("version", "")).removeprefix("v") != tag[1:]:
            raise ValueError("Windows source/package identity mismatch")
    if build.get("target") != "windows-x64" or build.get("telemetryChannel") != channel or emitted.get("telemetry_channel") != channel:
        raise ValueError("Windows immutable channel mismatch")


def fill(template, replacements):
    text = template.read_text()
    for marker, value in replacements.items():
        if marker not in text:
            raise ValueError("installer/package template marker missing")
        text = text.replace(marker, value)
    if re.search(r"@[A-Z][A-Z0-9_]*@", text):
        raise ValueError("unresolved installer/package template marker")
    return text


def check_download_targets(installer, recipe, cef):
    # Recognize the actual supported installer naming path, not a '-cef'
    # comment. Changes to this contract must be reviewed with delivery tests.
    lines = [line.strip() for line in installer.splitlines()
             if line.strip() and not line.lstrip().startswith("#")]
    selection = ['name="backplane-$VER-$os-$arch.tar.gz"']
    if cef:
        selection = ['archive_plat=$os-$arch',
                     '[ "$archive_plat" != linux-x64 ] || archive_plat=linux-x64-cef',
                     'name="backplane-$VER-$archive_plat.tar.gz"']
    selection += ['url="https://github.com/$REPO/releases/download/$VER/$name"']
    if not any(lines[i:i + len(selection)] == selection for i in range(len(lines))):
        raise ValueError("installer archive selection does not match candidate variant")
    if sum(line.startswith("name=") for line in lines) != 1 or sum(line.startswith("url=") for line in lines) != 1:
        raise ValueError("ambiguous installer archive selection")
    if recipe is not None:
        suffix = "-cef" if cef else ""
        expected = 'source_x86_64=("$url/releases/download/v$pkgver/backplane-v$pkgver-linux-x64' + suffix + '.tar.gz")'
        sources = [line.strip() for line in recipe.splitlines() if line.strip().startswith("source_x86_64=")]
        if sources != [expected]:
            raise ValueError("package recipe archive target does not match candidate variant")


def collect(directory, source, tag, templates, require_cef=False):
    version, channel = identity(source, tag)
    names = payload_names(directory, tag)
    require_files(directory, names)
    windows_identity(directory, source, tag, channel)
    sums = {name: digest(directory / name) for name in names}
    x64, arm, darwin = names[:3]
    installer = templates / "install.sh.in"
    text = fill(installer, {"@VERSION@": tag, "@SHA_LINUX_X64@": sums[x64],
                           "@SHA_LINUX_ARM64@": sums[arm], "@SHA_DARWIN_ARM64@": sums[darwin]})
    recipe = None
    if channel == "release":
        aur = templates / "aur/PKGBUILD.in"
        recipe = fill(aur, {"@PKGVER@": version, "@SHA_LINUX_X64@": sums[x64], "@SHA_LINUX_ARM64@": sums[arm]})
    check_download_targets(text, recipe, "-cef.tar.gz" in x64)
    (directory / "install.sh").write_text(text)
    (directory / "install.sh").chmod(0o755)
    if recipe is not None:
        (directory / "PKGBUILD").write_text(recipe)
    files = {p.name: {"bytes": p.stat().st_size, "sha256": digest(p)} for p in sorted(directory.iterdir())}
    manifest = {"sourceCommit": source, "tag": tag, "version": version, "channel": channel,
                "buildRunId": os.environ.get("GITHUB_RUN_ID"),
                "files": files,
                "identityEvidence": {"windows-x64": "SOURCE/BUILD metadata checked; native package gate pending",
                                     "linux-x64": "PENDING platform-owner payload identity receipt",
                                     "linux-arm64": "PENDING platform-owner payload identity receipt",
                                     "darwin-arm64": "PENDING platform-owner payload identity receipt"},
                "productAcceptance": "PENDING: native/content/auth/telemetry/package owner gates"}
    (directory / "release-assets.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (directory / "SHA256SUMS").write_text("".join(f"{digest(p)}  {p.name}\n" for p in sorted(directory.iterdir()) if p.name != "SHA256SUMS"))
    verify(directory, source, tag, require_cef)


def verify(directory, source, tag, require_cef=False, run_id=None):
    version, channel = identity(source, tag)
    manifest = json.loads((directory / "release-assets.json").read_text())
    if (manifest.get("sourceCommit"), manifest.get("tag"), manifest.get("version"), manifest.get("channel")) != (source, tag, version, channel):
        raise ValueError("candidate inventory source/version/channel mismatch")
    if run_id is not None and manifest.get("buildRunId") != run_id:
        raise ValueError("candidate inventory build run mismatch")
    expected = set(payload_names(directory, tag)) | {"install.sh"}
    if channel == "release":
        expected.add("PKGBUILD")
    if set(manifest["files"]) != expected:
        raise ValueError("candidate inventory payload mismatch")
    if require_cef and not (directory / f"backplane-{tag}-linux-x64-cef.tar.gz").is_file():
        raise ValueError("this final release requires its accepted CEF archive")
    require_files(directory, expected | {"release-assets.json", "SHA256SUMS"})
    for name, item in manifest["files"].items():
        p = directory / name
        if item != {"bytes": p.stat().st_size, "sha256": digest(p)}:
            raise ValueError("candidate asset changed after collection")
    checksum_entries = {}
    for line in (directory / "SHA256SUMS").read_text().splitlines():
        sha, name = line.split("  ", 1)
        if name in checksum_entries or name not in expected | {"release-assets.json"} or not re.fullmatch(r"[0-9a-f]{64}", sha):
            raise ValueError("invalid aggregate checksum inventory")
        checksum_entries[name] = sha
    if set(checksum_entries) != expected | {"release-assets.json"}:
        raise ValueError("incomplete aggregate checksum inventory")
    if any(digest(directory / name) != sha for name, sha in checksum_entries.items()):
        raise ValueError("aggregate checksum mismatch")
    check_download_targets((directory / "install.sh").read_text(),
                           (directory / "PKGBUILD").read_text() if channel == "release" else None,
                           (directory / f"backplane-{tag}-linux-x64-cef.tar.gz").is_file())
    windows_identity(directory, source, tag, channel)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["collect", "verify", "check-bake"])
    parser.add_argument("--directory", type=Path)
    parser.add_argument("--source", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--require-cef", action="store_true")
    parser.add_argument("--run-id")
    args = parser.parse_args()
    try:
        _, channel = identity(args.source, args.tag)
        if args.command == "check-bake":
            check_bake(os.environ, channel)
        else:
            if args.directory is None:
                raise ValueError("candidate directory is required")
            if args.command == "collect":
                collect(args.directory, args.source, args.tag, Path(__file__).resolve().parent.parent / "deploy", args.require_cef)
            else:
                verify(args.directory, args.source, args.tag, args.require_cef, args.run_id)
    except (ValueError, KeyError, OSError, zipfile.BadZipFile) as error:
        parser.exit(1, f"release-assets: {error}\n")
    print("PASS input/asset checks only; server ingest and product gate acceptance are separate")


if __name__ == "__main__":
    main()
