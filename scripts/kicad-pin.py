#!/usr/bin/env python3
"""Pin a Backplane KiCad release in native/host/src/kicad/install.rs: download, verify, fill in.

usage: scripts/kicad-pin.py <tag> [--platforms linux-x86_64,windows-x86_64,darwin-arm64] [--dir DIR] [--check] [--feed URL]
  <tag>  e.g. backplane-kicad-v10.99.0-bp.13 (the release in install.rs FROM)

For each platform named (default: all three):
  - its asset (backplane-kicad-<platform>-<version>.<tar.gz|zip>) must have a line in the release's SHA256SUMS;
  - the asset is downloaded (gh release download, into DIR, default $BP_TMP/kicad-pin/<tag>) and its sha256 must equal that line
    (the tarball's own SHA256SUMS line is the pin, never GitHub's artifact digest);
  - its manifest.json (the one root folder's) must say that platform and version; Windows and macOS must have the capability
    global-tables-seeded and a backplaneLibrary pin; macOS must say its minOs;
  - its install.rs line (LINUX, WINDOWS, MAC) gets that version, release, asset, digest and size (bytes: what the update offer says).
--feed URL reads SHA256SUMS and the assets from a staged http(s) folder instead of the release (a build before its tag; the hub installs
from the same folder with BACKPLANE_KICAD_FEED). --check verifies and prints without writing. Nothing is written unless every platform asked for passes.
"""
import hashlib, json, os, re, subprocess, sys, tarfile, zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
INSTALL = os.path.join(ROOT, "native/host/src/kicad/install.rs")
CONST = {"linux-x86_64": ("LINUX", "tar.gz"), "windows-x86_64": ("WINDOWS", "zip"), "darwin-arm64": ("MAC", "tar.gz")}


def die(msg):
    sys.exit(f"kicad-pin: {msg}")


def repo():
    m = re.search(r'pub const FROM: &str = "github\.com/([^"]+)"', open(INSTALL).read())
    return m.group(1) if m else die("no FROM in install.rs")


FEED = None


def gh_download(rep, tag, pattern, d):
    if FEED:
        import urllib.request
        try: urllib.request.urlretrieve(f"{FEED.rstrip('/')}/{pattern}", os.path.join(d, pattern))
        except Exception as e: die(f"could not download {pattern} from {FEED}: {e}")
        return
    r = subprocess.run(["gh", "release", "download", tag, "-R", rep, "-p", pattern, "-D", d, "--clobber"], capture_output=True, text=True)
    if r.returncode != 0: die(f"could not download {pattern} from {rep} {tag}: {r.stderr.strip()}")


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""): h.update(b)
    return h.hexdigest()


def manifest(path, fmt):
    if fmt == "zip":
        with zipfile.ZipFile(path) as z:
            names = [n for n in z.namelist() if n.count("/") == 1 and n.endswith("/manifest.json")]
            if len(names) != 1: die(f"{os.path.basename(path)}: no single <root>/manifest.json ({names[:3]})")
            return json.loads(z.read(names[0]))
    with tarfile.open(path, "r:gz") as t:
        for m in t:
            parts = m.name.lstrip("./").split("/")
            if len(parts) == 2 and parts[1] == "manifest.json":
                return json.load(t.extractfile(m))
    die(f"{os.path.basename(path)}: no <root>/manifest.json")


def main():
    args = sys.argv[1:]
    if not args or args[0].startswith("-"): die(__doc__)
    tag = args[0]; check = "--check" in args
    global FEED
    if "--feed" in args: FEED = args[args.index("--feed") + 1]
    plats = list(CONST)
    if "--platforms" in args: plats = args[args.index("--platforms") + 1].split(",")
    for p in plats:
        if p not in CONST: die(f"unknown platform {p}")
    version = tag.removeprefix("backplane-kicad-v")
    if not re.fullmatch(r"\d+\.\d+\.\d+-bp\.\d+(\.\d+)?", version): die(f"{tag} is not backplane-kicad-v<version>")
    d = args[args.index("--dir") + 1] if "--dir" in args else os.path.join(os.environ.get("BP_TMP", os.path.expanduser("~/.cache/bp-tmp")), "kicad-pin", tag)
    os.makedirs(d, exist_ok=True)
    rep = repo()
    gh_download(rep, tag, "SHA256SUMS", d)
    sums = {}
    for line in open(os.path.join(d, "SHA256SUMS")):
        f = line.split()
        if len(f) == 2: sums[f[1].lstrip("*").removeprefix("./")] = f[0].lower()
    pins = {}
    for p in plats:
        const, fmt = CONST[p]
        asset = f"backplane-kicad-{p}-{version}.{fmt}"
        want = sums.get(asset) or die(f"SHA256SUMS of {tag} has no line for {asset}")
        if not re.fullmatch(r"[0-9a-f]{64}", want): die(f"{asset}: SHA256SUMS says {want!r}, not a sha256")
        path = os.path.join(d, asset)
        if not (os.path.isfile(path) and sha256(path) == want): gh_download(rep, tag, asset, d)
        got = sha256(path)
        if got != want: die(f"{asset}: downloaded sha256 {got} is not SHA256SUMS's {want}")
        m = manifest(path, fmt)
        caps = m.get("capabilities", [])
        bad = []
        if m.get("platform") != p: bad.append(f"platform {m.get('platform')!r}")
        if m.get("version") != version: bad.append(f"version {m.get('version')!r}")
        if p != "linux-x86_64" and "global-tables-seeded" not in caps: bad.append("no global-tables-seeded")
        if p != "linux-x86_64" and not (m.get("backplaneLibrary") or {}).get("commit"): bad.append("no backplaneLibrary pin")
        if p == "darwin-arm64" and not m.get("minOs"): bad.append("no minOs")
        if bad: die(f"{asset}: manifest {', '.join(bad)}")
        lib = m.get("backplaneLibrary") or {}
        print(f"ok {p}: {asset} {os.path.getsize(path):,} bytes sha256 {got}; manifest {m.get('version')} {len(caps)} capabilities"
              f"{', minOs ' + m['minOs'] if m.get('minOs') else ''}{', library ' + str(lib.get('version')) + ' ' + str(lib.get('commit', ''))[:7] if lib else ''};"
              f" source {str(m.get('sourceCommit', ''))[:10]}")
        pins[const] = (p, asset, got, fmt, os.path.getsize(path))
    if check: print("checked; install.rs not written (--check)"); return
    s = open(INSTALL).read()
    for const, (p, asset, digest, fmt, size) in pins.items():
        rx = re.compile(rf'^pub const {const}: Package = Package \{{[^\n]*\}};$', re.M)
        if len(rx.findall(s)) != 1: die(f"install.rs: no single `pub const {const}` line")
        cli = "bin/kicad-cli.exe" if fmt == "zip" else "bin/kicad-cli"
        line = (f'pub const {const}: Package = Package {{ platform: "{p}", version: "{version}", release: "{tag}", asset: "{asset}", '
                f'sha256: "{digest}", format: "{fmt}", cli: "{cli}", size: {size} }};')
        s = rx.sub(lambda _: line, s)
    open(INSTALL, "w").write(s)
    left = re.findall(r'sha256: "PIN-ME[^"]*"', s)
    print(f"wrote {', '.join(pins)} in {os.path.relpath(INSTALL, ROOT)}" + (f"; placeholders left: {left}" if left else ""))


if __name__ == "__main__":
    main()
