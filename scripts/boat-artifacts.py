#!/usr/bin/env python3
"""boat-artifacts.py MSGS TARGETDIR OUT_LIST OUT_SHA: ON a Boat build box, after `cargo build --message-format=json-render-diagnostics`: the programs and libraries it
produced (paths relative to TARGETDIR) and their sha256, for scripts/lib-boat.sh to copy back and check."""
import hashlib, json, os, sys
msgs, tdir, out_list, out_sha = sys.argv[1:5]
tdir = os.path.realpath(tdir); seen = []
for line in open(msgs, errors="replace"):
    try: j = json.loads(line)
    except ValueError: continue
    if j.get("reason") != "compiler-artifact" or j.get("profile", {}).get("test"): continue
    kinds = j.get("target", {}).get("kind", [])
    files = [j["executable"]] if j.get("executable") else []
    if any(k in kinds for k in ("cdylib", "dylib", "bin")): files += j.get("filenames", [])
    for f in files:
        f = os.path.realpath(f)
        if f.startswith(tdir + os.sep) and os.path.isfile(f):
            r = os.path.relpath(f, tdir)
            if r not in seen: seen.append(r)
with open(out_list, "w") as a, open(out_sha, "w") as b:
    for r in seen:
        a.write(r + "\n")
        b.write("%s  %s\n" % (hashlib.sha256(open(os.path.join(tdir, r), "rb").read()).hexdigest(), r))
