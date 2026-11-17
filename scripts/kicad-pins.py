#!/usr/bin/env python3
"""The signed KiCad pin document (docs/kicad-pins.md; the hub's reader is native/host/src/kicad/pins.rs).

usage:
  scripts/kicad-pins.py open ENVELOPE [--pub PEM --keyid ID] > body.json
      verify ENVELOPE (kicad-pins.json) with openssl under the key (default: native/host/src/update/release.pub and release.keyid) and print
      its signed body; exits 1 when the signature, the key id or the shape is wrong.
  scripts/kicad-pins.py next [--current body.json] --platform P (--pin pin.json | --drop) --min-hub V [--days N] > body.json
      the next body: the current one's pins (none without --current), P's replaced by the pin kicad-pin.py --json checked (or dropped: that
      platform goes back to the compiled pin), sequence one past the current one, expiry N days from now (default 120).
      --platform none re-signs the current pins under a new sequence and expiry (a refresh before the expiry).
  scripts/kicad-pins.py seal BODY SIG KEYID > kicad-pins.json
      the envelope: BODY's bytes and SIG (64 raw bytes, openssl pkeyutl -sign -rawin) under KEYID.
Nothing here signs: the signature is made by .github/workflows/kicad-pins.yml with the release key, which exists only on GitHub.
"""
import base64, json, os, re, subprocess, sys, tempfile, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
KIND = "backplane-kicad-pins"
PLATFORMS = {"linux-x86_64": "tar.gz", "darwin-arm64": "tar.gz", "windows-x86_64": "zip"}
REL_PUB = os.path.join(ROOT, "native/host/src/update/release.pub")
REL_KEYID = os.path.join(ROOT, "native/host/src/update/release.keyid")


def die(msg):
    sys.exit(f"kicad-pins: {msg}")


def arg(args, name, default=None):
    return args[args.index(name) + 1] if name in args else default


def check_pin(plat, p):
    """The hub's checks (pins.rs parse, install.rs check_fields), so nothing is signed that every hub would refuse."""
    if plat not in PLATFORMS: die(f"unknown platform {plat}")
    v = p.get("version", "")
    if not re.fullmatch(r"[0-9A-Za-z.-]{1,64}", v): die(f"{plat}: bad version {v!r}")
    if p.get("release") != f"backplane-kicad-v{v}": die(f"{plat}: release {p.get('release')!r}")
    if p.get("asset") != f"backplane-kicad-{plat}-{v}.{PLATFORMS[plat]}": die(f"{plat}: asset {p.get('asset')!r}")
    if not re.fullmatch(r"[0-9a-f]{64}", p.get("sha256", "")): die(f"{plat}: sha256 {p.get('sha256')!r}")
    if not (isinstance(p.get("size"), int) and 0 < p["size"] <= 8 << 30): die(f"{plat}: size {p.get('size')!r}")
    if not re.fullmatch(r"\d{1,10}\.\d{1,10}\.\d{1,10}(-beta\.\d{1,10})?", p.get("min_hub", "")): die(f"{plat}: min_hub {p.get('min_hub')!r}")
    caps = p.get("caps", [])
    if not (isinstance(caps, list) and len(caps) <= 256 and all(isinstance(c, str) and re.fullmatch(r"[a-z0-9.-]{1,64}", c) for c in caps)): die(f"{plat}: caps")
    if "min_os" in p and not re.fullmatch(r"\d{1,4}(\.\d{1,4}){0,3}", p["min_os"]): die(f"{plat}: min_os {p['min_os']!r}")


def check_body(b):
    if b.get("schema") != 1 or b.get("kind") != KIND: die("not a pin document")
    if not (isinstance(b.get("sequence"), int) and b["sequence"] > 0): die("no sequence")
    if not (isinstance(b.get("expires"), int) and b["expires"] > 0): die("no expiry")
    for plat, p in (b.get("pins") or {}).items(): check_pin(plat, p)


def cmd_open(args):
    env = json.load(open(args[0]))
    pub, keyid = arg(args, "--pub", REL_PUB), arg(args, "--keyid", open(REL_KEYID).read().strip())
    if env.get("schema") != 1: die("not a pin envelope")
    if env.get("keyid", "").strip() != keyid: die(f"signed under key {env.get('keyid')!r}, not {keyid}")
    body, sig = base64.b64decode(env["doc"]), base64.b64decode(env["sig"])
    with tempfile.TemporaryDirectory() as d:
        open(f"{d}/b", "wb").write(body); open(f"{d}/s", "wb").write(sig)
        r = subprocess.run(["openssl", "pkeyutl", "-verify", "-pubin", "-inkey", pub, "-rawin", "-in", f"{d}/b", "-sigfile", f"{d}/s"], capture_output=True, text=True)
        if r.returncode != 0: die(f"the signature does not verify: {r.stdout.strip()} {r.stderr.strip()}")
    b = json.loads(body); check_body(b)
    sys.stdout.write(json.dumps(b, indent=1, sort_keys=True) + "\n")


def cmd_next(args):
    cur = json.load(open(arg(args, "--current"))) if arg(args, "--current") else None
    if cur: check_body(cur)
    plat = arg(args, "--platform") or die("--platform")
    pins = dict(cur["pins"]) if cur else {}
    if plat != "none":
        if plat not in PLATFORMS: die(f"unknown platform {plat}")
        if "--drop" in args: pins.pop(plat, None)
        else:
            got = json.load(open(arg(args, "--pin") or die("--pin or --drop")))
            p = dict(got.get(plat) or die(f"{arg(args, '--pin')} has no {plat}"))
            p["min_hub"] = arg(args, "--min-hub") or die("--min-hub")
            check_pin(plat, p); pins[plat] = p
    days = int(arg(args, "--days", "120"))
    if not 1 <= days <= 400: die("--days is 1 to 400")
    b = {"schema": 1, "kind": KIND, "sequence": (cur["sequence"] if cur else 0) + 1, "expires": int(time.time()) + days * 86400, "pins": pins}
    check_body(b)
    sys.stdout.write(json.dumps(b, indent=1, sort_keys=True) + "\n")


def cmd_seal(args):
    body, sig, keyid = open(args[0], "rb").read(), open(args[1], "rb").read(), args[2].strip()
    if len(sig) != 64: die("the signature is not 64 bytes")
    check_body(json.loads(body))
    sys.stdout.write(json.dumps({"schema": 1, "keyid": keyid, "sig": base64.b64encode(sig).decode(), "doc": base64.b64encode(body).decode()}) + "\n")


if __name__ == "__main__":
    a = sys.argv[1:]
    cmds = {"open": cmd_open, "next": cmd_next, "seal": cmd_seal}
    if not a or a[0] not in cmds: die(__doc__)
    cmds[a[0]](a[1:])
