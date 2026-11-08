#!/usr/bin/env python3
"""TestFlight crash feedback for the iOS app, read-only, through the App Store Connect API.

Runs on the Mac that cuts TestFlight builds: python3 asc_crashes.py [--days N] [--all] [--env PATH] [--out DIR]
The credentials are the ones the archive already uses (signing.env: BUNDLE_ID, ASC_KEY_ID, ASC_ISSUER_ID, ASC_KEY_PATH, default ~/backplane-ios/signing.env);
nothing secret is in this file, and the key and token are never printed. The token is signed with openssl, so no Python packages are needed.

Prints one JSON line per crash not reported before (--all: every crash in the window): build, time, device, iOS version, the tester's comment, the
exception and the crashed thread's top frames (Apple symbolicates TestFlight logs from the dSYMs the archive uploads). Each crash's record and log are
kept in OUT (default ~/bp-gpui-ios/crashes), whose seen.txt remembers what was reported. A last line {"summary": ...} counts them.

What the API does not give: Xcode Organizer's automatic crash reports (only crashes a tester sent from TestFlight's prompt are here), and
diagnosticSignatures answered 404 for this app's builds (2026-11-07).
"""
import argparse, base64, calendar, json, os, re, subprocess, sys, time, urllib.error, urllib.request

ap = argparse.ArgumentParser()
ap.add_argument("--days", type=float, default=3)
ap.add_argument("--all", action="store_true", help="report crashes already reported too")
ap.add_argument("--env", default="~/backplane-ios/signing.env")
ap.add_argument("--out", default="~/bp-gpui-ios/crashes")
a = ap.parse_args()

env = {}
for line in open(os.path.expanduser(a.env)):
    if "=" in line and not line.lstrip().startswith("#"):
        k, v = line.strip().split("=", 1)
        env[k] = v.split("#")[0].strip().strip('"')

def b64(b): return base64.urlsafe_b64encode(b).rstrip(b"=").decode()

def token():
    now = int(time.time())
    head = b64(json.dumps({"alg": "ES256", "kid": env["ASC_KEY_ID"], "typ": "JWT"}).encode())
    body = b64(json.dumps({"iss": env["ASC_ISSUER_ID"], "iat": now, "exp": now + 900, "aud": "appstoreconnect-v1"}).encode())
    msg = (head + "." + body).encode()
    der = subprocess.run(["openssl", "dgst", "-sha256", "-sign", os.path.expanduser(env["ASC_KEY_PATH"])], input=msg, capture_output=True, check=True).stdout
    i, raw = 2, b""
    for _ in range(2):  # DER (r, s) -> the 64 raw bytes ES256 wants
        n = der[i + 1]; raw += der[i + 2:i + 2 + n].lstrip(b"\0").rjust(32, b"\0"); i += 2 + n
    return msg.decode() + "." + b64(raw)

TOKEN = token()

def get(path):
    req = urllib.request.Request("https://api.appstoreconnect.apple.com" + path, headers={"Authorization": "Bearer " + TOKEN})
    try:
        with urllib.request.urlopen(req, timeout=60) as r: return r.status, json.load(r)
    except urllib.error.HTTPError as e:
        return e.code, {}

def top(log, n=6):
    """The exception and the first frames of the crashed thread."""
    exc = next((l.split(":", 1)[1].strip() for l in log.splitlines() if l.startswith("Exception Type:")), "")
    frames, on = [], False
    for l in log.splitlines():
        if re.match(r"Thread \d+ Crashed", l): on = True; continue
        if on:
            if not l.strip(): break
            frames.append(re.sub(r"\s+", " ", l.strip())[:200])
            if len(frames) >= n: break
    return exc, frames

out = os.path.expanduser(a.out); os.makedirs(out, exist_ok=True)
seen_file = os.path.join(out, "seen.txt")
seen = set(open(seen_file).read().split()) if os.path.exists(seen_file) else set()

s, apps = get("/v1/apps?filter[bundleId]=" + env["BUNDLE_ID"])
if s != 200 or not apps.get("data"): print(json.dumps({"error": "app %s not readable (HTTP %s)" % (env["BUNDLE_ID"], s)})); sys.exit(1)
app = apps["data"][0]["id"]
s, j = get("/v1/apps/%s/betaFeedbackCrashSubmissions?limit=200&include=build" % app)
if s != 200: print(json.dumps({"error": "crash feedback not readable (HTTP %s)" % s})); sys.exit(1)
builds = {x["id"]: x["attributes"].get("version") for x in j.get("included", []) if x["type"] == "builds"}
cut, new, total = time.time() - a.days * 86400, 0, 0
for x in j.get("data", []):
    at = x["attributes"]; t = at.get("createdDate", "")
    if t and calendar.timegm(time.strptime(t[:19], "%Y-%m-%dT%H:%M:%S")) < cut: continue
    total += 1
    if x["id"] in seen and not a.all: continue
    s2, c = get("/v1/betaFeedbackCrashSubmissions/%s/crashLog" % x["id"])
    log = (c.get("data") or {}).get("attributes", {}).get("logText", "") if s2 == 200 else ""
    json.dump(x, open(os.path.join(out, x["id"] + ".json"), "w"), indent=1)
    open(os.path.join(out, x["id"] + ".crash"), "w").write(log)
    exc, frames = top(log)
    bid = ((x.get("relationships") or {}).get("build") or {}).get("data") or {}
    print(json.dumps({"id": x["id"], "build": builds.get(bid.get("id"), "?"), "time": t, "device": at.get("deviceModel"), "ios": at.get("osVersion"),
                      "comment": (at.get("comment") or "")[:200], "exception": exc, "top": frames, "log": "saved" if log else "none (HTTP %s)" % s2}))
    seen.add(x["id"]); new += 1
open(seen_file, "w").write("\n".join(sorted(seen)) + "\n")
print(json.dumps({"summary": {"days": a.days, "in_window": total, "reported": new}}))
