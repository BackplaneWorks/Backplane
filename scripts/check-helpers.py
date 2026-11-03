#!/usr/bin/env python3
"""Refuse a packaged app whose service folder lacks a helper the hub runs, or KiStack.

  check-helpers.py PATH --target TARGET [--service-dir]

PATH is a desktop release archive (.tar.gz, .zip), an unpacked one or a
stage (a folder), or with --service-dir the service folder itself. Every
backplane-service folder in it must hold the target's helpers
(service-release.py's helper_names: backplane-kipc, -step2glb, -voice,
-browser; on Windows kipc and step2glb as .exe) beside backplane-serve, as
regular executable files of the target's architecture, code signed on macOS.
0.13.0's macOS and Linux apps shipped none, and KiCad IPC never started
(issue #510). Each must also hold KiStack's skills as kistack/, exactly the
commit deploy/kistack.lock pins (scripts/kistack.py): release packages
carried none from 0.12.0 on. TARGET "host" is this machine's.
"""
import argparse
import importlib.util
from pathlib import Path, PurePosixPath
import platform
import shutil
import sys
import tarfile
import tempfile
import zipfile

spec = importlib.util.spec_from_file_location("service_release", Path(__file__).resolve().with_name("service-release.py"))
S = importlib.util.module_from_spec(spec)
spec.loader.exec_module(S)

SERVICE_DIR = "backplane-service"


def host():
    return {("Linux", "x86_64"): "linux-x64", ("Linux", "aarch64"): "linux-arm64",
            ("Darwin", "arm64"): "darwin-arm64", ("Darwin", "x86_64"): "darwin-x64",
            ("Windows", "AMD64"): "windows-x64"}[(platform.system(), platform.machine())]


def folders(names):
    # the service folders an archive's member names show: those holding the service
    found = set()
    for name in names:
        p = PurePosixPath(name)
        if p.parent.name == SERVICE_DIR and p.name in ("backplane-serve", "backplane-serve.exe"):
            found.add(str(p.parent))
    return sorted(found)


def kistack_members(names, folder):
    # an archive's members under FOLDER/kistack/, as paths inside kistack/
    prefix = folder + "/" + S.KISTACK + "/"
    return {n: n[len(prefix):] for n in names if n.startswith(prefix) and n[len(prefix):]}


def safe(inside):
    S.require(all(p not in ("", ".", "..") for p in PurePosixPath(inside).parts) and not inside.startswith("/"),
              "Unsafe KiStack member " + inside)
    return inside


def extract(path, target, into):
    """The helpers and KiStack of each service folder in an archive, copied
    out with the modes the archive records. Answers the folders made under
    INTO."""
    wanted = S.helper_names(target)
    made = []
    if str(path).endswith(".zip"):
        with zipfile.ZipFile(path) as z:
            infos = {i.filename: i for i in z.infolist() if not i.is_dir()}
            for n, folder in enumerate(folders(infos)):
                dest = into / str(n)
                dest.mkdir()
                for name, inside in kistack_members(infos, folder).items():
                    S.require(((infos[name].external_attr >> 16) & 0o170000) in (0, 0o100000), "KiStack member " + name + " is not a regular file")
                    out = dest / S.KISTACK / safe(inside)
                    out.parent.mkdir(parents=True, exist_ok=True)
                    with z.open(infos[name]) as src, out.open("wb") as o:
                        shutil.copyfileobj(src, o)
                for name in wanted:
                    info = infos.get(folder + "/" + name)
                    if info is None:
                        continue
                    with z.open(info) as src, (dest / name).open("wb") as out:
                        shutil.copyfileobj(src, out)
                    mode = (info.external_attr >> 16) & 0o777
                    # a zip made on Windows records no Unix mode
                    (dest / name).chmod(mode or 0o755)
                made.append((folder, dest))
    else:
        with tarfile.open(path, "r:*") as tar:
            members = {m.name.removeprefix("./"): m for m in tar.getmembers()}
            for n, folder in enumerate(folders(members)):
                dest = into / str(n)
                dest.mkdir()
                for name, inside in kistack_members(members, folder).items():
                    m = members[name]
                    if m.isdir():
                        continue
                    S.require(m.isfile(), "KiStack member " + name + " is not a regular file")
                    out = dest / S.KISTACK / safe(inside)
                    out.parent.mkdir(parents=True, exist_ok=True)
                    with tar.extractfile(m) as src, out.open("wb") as o:
                        shutil.copyfileobj(src, o)
                for name in wanted:
                    m = members.get(folder + "/" + name)
                    if m is None:
                        continue
                    S.require(m.isfile(), "Helper " + name + " in " + folder + " is not a regular file")
                    with tar.extractfile(m) as src, (dest / name).open("wb") as out:
                        shutil.copyfileobj(src, out)
                    (dest / name).chmod(m.mode & 0o777)
                made.append((folder, dest))
    return made


def complete(root, target):
    # the helpers and KiStack of one service folder
    found = S.check_helpers(root, target)
    found[S.KISTACK] = S.check_kistack(root)["rev"]
    return found


def check(path, target, service_dir=False):
    path = Path(path)
    if service_dir:
        return {str(path): complete(path, target)}
    if path.is_dir():
        names = [p.relative_to(path).as_posix() for p in path.rglob("*") if p.is_file() or p.is_symlink()]
        places = [(f, path / f) for f in folders(names)]
        S.require(places, "No " + SERVICE_DIR + " folder with a service in " + str(path))
        result = {}
        for f, d in places:
            try:
                result[f] = complete(d, target)
            except ValueError as e:
                raise ValueError(str(e) + " (" + f + ")") from None
        return result
    with tempfile.TemporaryDirectory() as tmp:
        places = extract(path, target, Path(tmp))
        S.require(places, "No " + SERVICE_DIR + " folder with a service in " + str(path))
        result = {}
        for folder, dest in places:
            try:
                result[folder] = complete(dest, target)
            except ValueError as e:
                raise ValueError(str(e) + " (" + folder + ")") from None
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("path", type=Path)
    parser.add_argument("--target", required=True)
    parser.add_argument("--service-dir", action="store_true")
    args = parser.parse_args()
    target = host() if args.target == "host" else args.target
    S.require(target in S.TARGETS, "Unknown target " + target)
    for folder, helpers in check(args.path, target, args.service_dir).items():
        print("helpers ok: " + folder + ": " + ", ".join(sorted(n for n in helpers if n != S.KISTACK))
              + "; kistack " + helpers[S.KISTACK])


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, tarfile.TarError, zipfile.BadZipFile) as e:
        print("check-helpers: " + str(e), file=sys.stderr)
        sys.exit(1)
