#!/usr/bin/env python3
"""Package the existing native Linux build using the Freedesktop runtime."""
import argparse
import json
from pathlib import Path
import subprocess

APP_ID = "io.github.i2cjak.Backplane"
ROOT = Path(__file__).resolve().parents[1]


def manifest(package, assets):
    return {
        "app-id": APP_ID,
        "runtime": "org.freedesktop.Platform",
        "runtime-version": "25.08",
        "sdk": "org.freedesktop.Sdk",
        "command": "backplane",
        "finish-args": [
            "--share=network", "--share=ipc", "--socket=x11", "--device=dri",
            "--socket=pulseaudio", "--filesystem=host", "--filesystem=/tmp",
            "--filesystem=/var/tmp", "--talk-name=org.freedesktop.Flatpak",
            "--env=BACKPLANE_NO_UPDATE=1",
        ],
        "modules": [{
            "name": "backplane",
            "buildsystem": "simple",
            "build-commands": [
                "mkdir -p /app/lib/backplane /app/bin /app/share/applications "
                "/app/share/icons/hicolor/scalable/apps /app/share/metainfo",
                "cp -a . /app/lib/backplane/",
                "install -m755 backplane-launch /app/bin/backplane",
                "install -m755 kicad-cli /app/bin/kicad-cli",
                "chmod 755 /app/lib/backplane/host-tool",
                f"install -m644 backplane.desktop /app/share/applications/{APP_ID}.desktop",
                f"sed -i 's/^Icon=backplane$/Icon={APP_ID}/' /app/share/applications/{APP_ID}.desktop",
                f"install -m644 icon/backplane.svg /app/share/icons/hicolor/scalable/apps/{APP_ID}.svg",
                f"install -m644 {APP_ID}.metainfo.xml /app/share/metainfo/",
            ],
            "sources": [
                {"type": "dir", "path": str(package)},
                *[{"type": "file", "path": str(assets / source), "dest-filename": dest}
                  for source, dest in [
                      ("backplane", "backplane-launch"), ("host-tool", "host-tool"),
                      ("kicad-cli", "kicad-cli"),
                      (f"{APP_ID}.metainfo.xml", f"{APP_ID}.metainfo.xml"),
                  ]],
            ],
        }],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument("platform", choices=["linux-x64", "linux-arm64"])
    parser.add_argument("--manifest-only", action="store_true")
    args = parser.parse_args()
    package = ROOT / "dist/pkg" / f"backplane-{args.platform}"
    if not (package / "backplane").is_file():
        parser.error(f"run scripts/package.sh {args.version} {args.platform} first")
    work = ROOT / "build/flatpak" / args.platform
    work.mkdir(parents=True, exist_ok=True)
    source = work / "manifest.json"
    source.write_text(json.dumps(manifest(package, ROOT / "deploy/flatpak"), indent=2) + "\n")
    if args.manifest_only:
        print(source)
        return
    repo = work / "repo"
    subprocess.run(["flatpak-builder", "--force-clean", "--disable-rofiles-fuse",
                    "--default-branch=stable", f"--repo={repo}", str(work / "build"),
                    str(source)], check=True, cwd=ROOT)
    output = ROOT / "dist" / f"backplane-{args.version}-{args.platform}.flatpak"
    subprocess.run(["flatpak", "build-bundle", "--runtime-repo=https://flathub.org/repo/flathub.flatpakrepo",
                    str(repo), str(output), APP_ID, "stable"], check=True, cwd=ROOT)
    print(output)


if __name__ == "__main__":
    main()
