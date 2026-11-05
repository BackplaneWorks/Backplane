#!/bin/sh
# inside Boat: build native/bend/window.bend (native) and check its windows against the JS hub's (fixtures/windows.json)
set -u
export BEND_NO_TELEMETRY=1
O=out/window; mkdir -p $O
bend native/bend/window.bend --check-only 2>&1 | head -40
bend native/bend/window.bend -o $O/window.c 2>&1 | head -30 && clang -std=c11 -O2 $O/window.c -lpthread -lm -o $O/window 2>&1 | head -5
ls -l $O/window || exit 1
python3 - <<'PY'
import json, subprocess, os
F = "test/gpui/hub/fixtures"
pages = json.load(open(F + "/pages.json"))["pages"]; refs = json.load(open(F + "/windows.json"))
bad = 0
for r in refs:
    page = next(p for p in pages if p["thread"] == r["thread"] and p["before"] == 0)
    esc = lambda s: s.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n")
    lines = ["\t".join(["window", r["thread"], r["epoch"], "0", str(page["below"]), str(page["above"]), json.dumps({"target": r["request"]["target"]})])]
    for row in page["rows"]: lines.append("\t".join(esc(str(c)) for c in row))
    open("out/window/req", "w").write("\n".join(lines) + "\n")
    out = subprocess.run(["out/window/window", "out/window/req"], capture_output=True, text=True).stdout
    try: got = json.loads(out)
    except Exception: print(r["thread"], "NOT JSON:", out[:300]); bad += 1; continue
    diff = [k for k in sorted(set(got) | set(r["window"])) if got.get(k) != r["window"].get(k)]
    print(r["thread"], "window equal" if not diff else "DIFFERS in " + ",".join(diff))
    for k in diff[:4]: print("   ", k, "got", json.dumps(got.get(k))[:200], "| js", json.dumps(r["window"].get(k))[:200])
    bad += bool(diff)
print("windows differing:", bad, "of", len(refs))
PY
