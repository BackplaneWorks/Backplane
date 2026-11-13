#!/usr/bin/env python3
"""stock_compat.py --fork-cli F --stock-cli S --out DIR PROJECT_DIR [PROJECT_DIR...]

Stock KiCad compatibility: a project the fork has saved natively, exported with the fork's
`kicad-cli project downgrade --target 10.0`, must load in a release (stock) KiCad with no repair,
version or unknown-token message, and give the same ERC and DRC results (violations by type and
severity, counted) as the fork's own kicad-cli on the native files; stock gerbers and netlist must
export. PROJECT_DIR is COPIED into DIR first (never touched); backups, build, mfg and lock files are
left behind. --as-is skips the fork's native resave (the project was already saved by the fork,
e.g. after the bare gate's co-edit). Both CLIs run with a private HOME/XDG config, so neither reads
this user's KiCad settings or library tables.

Writes DIR/<name>/{native,kicad10,stock,fork}/... and DIR/<name>/result.json; prints one PASS/FAIL
line per check and a summary; exit 1 if any project fails. Findings carry the message and a diff.
"""
import argparse, collections, difflib, json, os, re, shutil, subprocess, sys, time

SKIP_DIRS = {"backups", "build", "mfg", ".git", "__pycache__", ".backplane"}
SKIP_RE = re.compile(r"(-backups$|^~.*\.lck$|\.lck$|^_autosave-|\.kicad_prl\.bak$)")
# stdout/stderr lines of kicad-cli that are progress, not a complaint
BENIGN = re.compile(r"^(Loading (board|schematic)|Running DRC|Running ERC|Found \d+ |Saved (DRC|ERC) Report|Report written|"
                    r"Plotted to |Plotting |Gerber |Done\.?$|Fetching |Updating |Building |Generating |Writing |Exporting |"
                    r"\d+ (violations|unconnected|footprint)|Copy |Copied |Created )", re.I)
COMPLAINT = re.compile(r"repair|version|unknown|token|unexpected|newer|upgrade|could not|cannot|can't|failed|error|warning|invalid|missing|corrupt", re.I)


def copy_project(src, dst):
    def ignore(d, names):
        return [n for n in names if n in SKIP_DIRS or SKIP_RE.search(n)]
    shutil.copytree(src, dst, ignore=ignore, symlinks=False)


def one(path, ext):
    c = sorted(f for f in os.listdir(path) if f.endswith(ext))
    return os.path.join(path, c[0]) if c else None


def run(cli, args, env, log):
    t = time.time()
    p = subprocess.run([cli, *args], env=env, capture_output=True, text=True, timeout=1800)
    out = (p.stdout or "") + (p.stderr or "")
    with open(log, "a") as f:
        f.write(f"$ {os.path.basename(cli)} {' '.join(args)}\n(exit {p.returncode}, {time.time()-t:.1f}s)\n{out}\n")
    return p.returncode, out


def complaints(out):
    bad = []
    for l in out.splitlines():
        s = l.strip()
        if not s or BENIGN.search(s):
            continue
        if COMPLAINT.search(s):
            bad.append(s[:400])
    return bad


def exact_items(project):
    """known entries for one project that name exact violations: [(key, description start, sorted item descriptions)]; by the items'
    descriptions, as the fork's gate does (a teardrop's uuid is made again at each load)"""
    try:
        return [(e["key"], i["description"], sorted(i["items"])) for e in json.load(open(KNOWN)) if e.get("items") and e.get("project") == project
                for i in e["items"]]
    except FileNotFoundError:
        return []


def drops():
    """known entries that name a description suffix: those items alone are left out on both sides (the type stays counted)"""
    try:
        return [(e["key"], e["drop_description_suffix"]) for e in json.load(open(KNOWN)) if e.get("drop_description_suffix")]
    except FileNotFoundError:
        return []


def fork_version(cli):
    """The fork runtime's version from its manifest (beside bin/), "" when it has none."""
    try: return json.load(open(os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(cli))), "manifest.json"))).get("version", "")
    except Exception: return ""


def bp_parts(v):
    """The bp. number's parts (10.99.0-bp.12.1 -> [12, 1]; the last bp. part of a describe name), [] for none: as kserve.rs KH.bp."""
    for part in reversed(v.split("-")):
        if part.startswith("bp."):
            n = [int(x) if x.isdigit() else 0 for x in part[3:].split(".")]
            while len(n) > 1 and n[-1] == 0: n.pop()
            return n
    return []


def tally(report, excluded=False, project=""):
    """(kind, type, severity) -> count from a kicad-cli ERC or DRC JSON report. Exclusions the person made (kept in the .kicad_pro)
    are counted apart: 10.0 lists a stale one (whose items are gone) under --severity-all, the fork drops it."""
    j = json.load(open(report))
    c = collections.Counter()
    vs = [("erc", v) for s in j.get("sheets", []) for v in s.get("violations", [])]
    vs += [(k, v) for k in ("violations", "unconnected_items", "schematic_parity") for v in j.get(k, [])]
    dr = drops(); exact = exact_items(project)
    for k, v in vs:
        key = (k, v.get("type"), v.get("severity"))
        if any("|".join(map(str, key)) == dk and str(v.get("description", "")).endswith(sfx) for dk, sfx in dr):
            continue
        # one reviewed violation, by its project, type, rule words and its exact items (their descriptions)
        uu = sorted(i.get("description", "") for i in v.get("items", []))
        if any("|".join(map(str, key)) == ek and str(v.get("description", "")).startswith(ed) and uu == eu for ek, ed, eu in exact):
            continue
        if bool(v.get("excluded")) == excluded:
            c[key] += 1
    return c


# Differences the two engines make on the same file (the fork's own checks give the same on the native files and on the export),
# each reviewed, with why; an entry here never hides a difference the export causes.
KNOWN = os.path.join(os.path.dirname(os.path.abspath(__file__)), "stock_engine_known.json")
def pending():
    """differences being chased: they stay FAILs, with who is on them"""
    try:
        return {(e.get("project", ""), e["key"]): e["pending"] for e in json.load(open(KNOWN)) if e.get("pending")}
    except FileNotFoundError:
        return {}


def known():
    try:
        return {e["key"]: e["why"] for e in json.load(open(KNOWN)) if not e.get("drop_description_suffix") and not e.get("pending") and not e.get("items")}
    except FileNotFoundError:
        return {}


def seed(cli, cfg):
    """the global symbol and footprint tables a person's first KiCad start copies from the install's template (the GUI wizard does it;
    a CLI-only first run of stock KiCad does not, which reports every library as missing)"""
    v = subprocess.run([cli, "--version"], capture_output=True, text=True).stdout.strip()
    ver = ".".join(v.split("-")[0].split(".")[:2]) or "10.0"
    share = [os.path.realpath(os.path.join(os.path.dirname(cli), "..", "share", "kicad", "template")), "/usr/share/kicad/template"]
    d = os.path.join(cfg, "kicad", ver); os.makedirs(d, exist_ok=True)
    for t in ("sym-lib-table", "fp-lib-table"):
        src = next((os.path.join(s, t) for s in share if os.path.isfile(os.path.join(s, t))), None)
        if src and not os.path.exists(os.path.join(d, t)):
            shutil.copy(src, os.path.join(d, t))
    return v


def env_for(root, name, cli):
    h = os.path.join(root, "cfg-" + name)
    os.makedirs(h, exist_ok=True)
    seed(cli, os.path.join(h, ".config"))
    e = {k: v for k, v in os.environ.items() if not k.startswith("KICAD")}
    e.update(HOME=h, XDG_CONFIG_HOME=os.path.join(h, ".config"), XDG_CACHE_HOME=os.path.join(h, ".cache"),
             XDG_DATA_HOME=os.path.join(h, ".local/share"))
    e.pop("DISPLAY", None); e.pop("WAYLAND_DISPLAY", None)
    return e


def kept(x, y, p=""):
    """the paths of values in x that y lost or changed (y may add keys: a writer filling in defaults loses nothing)"""
    if isinstance(x, dict) and isinstance(y, dict):
        return [q for k in x for q in (kept(x[k], y[k], f"{p}/{k}") if k in y else [f"{p}/{k} removed"])]
    if isinstance(x, list) and isinstance(y, list):
        return ([f"{p} length {len(x)} -> {len(y)}"] if len(x) != len(y) else []) + [q for i, (u, v) in enumerate(zip(x, y)) for q in kept(u, v, f"{p}[{i}]")]
    return [] if x == y else [f"{p}: {x!r:.60} -> {y!r:.60}"]


def file_diff(a, b, limit=400):
    try:
        x, y = open(a).read().splitlines(), open(b).read().splitlines()
    except Exception as ex:
        return [f"(no diff: {ex})"]
    d = list(difflib.unified_diff(x, y, a, b, n=1, lineterm=""))
    return d[:limit] + ([f"... {len(d)-limit} more diff lines"] if len(d) > limit else [])


def check(args, src):
    name = os.path.basename(os.path.normpath(src))
    root = os.path.join(args.out, name)
    if os.path.exists(root):
        shutil.rmtree(root)
    os.makedirs(root)
    nat, k10 = os.path.join(root, "native"), os.path.join(root, "kicad10")
    copy_project(src, nat)
    fenv, senv = env_for(root, "fork", args.fork_cli), env_for(root, "stock", args.stock_cli)
    flog, slog = os.path.join(root, "fork.log"), os.path.join(root, "stock.log")
    res = {"project": name, "source": src, "checks": [], "findings": [], "timings": {}}

    def chk(ok, what, detail=None):
        res["checks"].append({"ok": bool(ok), "what": what})
        print(f"{'PASS' if ok else 'FAIL'} {name}: {what}", flush=True)
        if not ok:
            res["findings"].append({"what": what, **(detail or {})})

    pro, pcb, sch = one(nat, ".kicad_pro"), one(nat, ".kicad_pcb"), one(nat, ".kicad_sch")
    if not pro:
        chk(False, "a .kicad_pro in the project"); return res
    stem = os.path.splitext(os.path.basename(pro))[0]
    pcb = os.path.join(nat, stem + ".kicad_pcb") if os.path.exists(os.path.join(nat, stem + ".kicad_pcb")) else pcb
    sch = os.path.join(nat, stem + ".kicad_sch") if os.path.exists(os.path.join(nat, stem + ".kicad_sch")) else sch

    # 1. saved natively by the fork
    if not args.as_is:
        t = time.time()
        for kind, f in (("pcb", pcb), ("sch", sch)):
            if f:
                rc, out = run(args.fork_cli, [kind, "upgrade", "--force", f], fenv, flog)
                chk(rc == 0, f"fork saves the {kind} natively", {"message": out[-2000:]})
        res["timings"]["fork_native_save_s"] = round(time.time() - t, 1)

    # 2. the fork's own checks on the native files
    os.makedirs(os.path.join(root, "fork")); os.makedirs(os.path.join(root, "stock"))
    fork_r, stock_r = {}, {}
    t = time.time()
    if sch:
        o = os.path.join(root, "fork", "erc.json"); rc, out = run(args.fork_cli, ["sch", "erc", "--format", "json", "--severity-all", "-o", o, sch], fenv, flog)
        if os.path.exists(o): fork_r["erc"] = o
        chk(os.path.exists(o), "fork ERC on the native schematic", {"message": out[-2000:]})
    if pcb:
        o = os.path.join(root, "fork", "drc.json"); a = ["pcb", "drc", "--format", "json", "--severity-all"] + (["--schematic-parity"] if sch else []) + ["-o", o, pcb]
        rc, out = run(args.fork_cli, a, fenv, flog)
        if os.path.exists(o): fork_r["drc"] = o
        chk(os.path.exists(o), "fork DRC on the native board", {"message": out[-2000:]})
    res["timings"]["fork_checks_s"] = round(time.time() - t, 1)

    # 3. the fork's export to a release KiCad
    t = time.time()
    rep = os.path.join(root, "downgrade-report.json")
    rc, out = run(args.fork_cli, ["project", "downgrade", "--target", args.target, "-o", k10, "--report-json", rep, pro], fenv, flog)
    forced = False
    if rc != 0:
        # a refusal that names what it would lose is the fork doing its job; before bp.12.1 it also refused what 10.0 evaluates the same
        # (intersectsArea rules, fixed in the fork at 13502c4cf1): a NOTE naming the features there, a FAIL from bp.12.1 on
        lost = []
        try: lost = [f"{e.get('feature')} ({e.get('category')})" for e in json.load(open(rep)).get("entries", []) if e.get("category") != "keep"]
        except Exception: pass
        fv = fork_version(args.fork_cli)
        if lost and not json.load(open(rep)).get("blocked") and bp_parts(fv) < [12, 1]:
            print(f"NOTE {name}: the fork ({fv}) exports to KiCad {args.target} only with --force: {'; '.join(lost)} (from bp.12.1 a rule 10.0 evaluates the same is kept)")
            res.setdefault("notes", []).append({"downgrade_refused": lost, "fork": fv})
        else:
            chk(False, f"fork exports to KiCad {args.target} without --force", {"message": out[-3000:], "report": rep})
        shutil.rmtree(k10, ignore_errors=True)
        rc, out = run(args.fork_cli, ["project", "downgrade", "--target", args.target, "--force", "-o", k10, "--report-json", rep, pro], fenv, flog)
        forced = True
    else:
        chk(True, f"fork exports to KiCad {args.target} without --force")
    res["timings"]["downgrade_s"] = round(time.time() - t, 1)
    res["downgrade_forced"] = forced
    if rc != 0 or not os.path.isdir(k10):
        chk(False, "an export was written at all", {"message": out[-3000:]}); return res
    # fork-only sidecars stay behind: the floorplanner's plan (<board>.kicad_plan, written on every board save) must not appear in the
    # export, and the project's own plan must come through the fork's native save unchanged
    plans = sorted(os.path.relpath(os.path.join(d, f), k10) for d, _, fs in os.walk(k10) for f in fs if f.endswith(".kicad_plan"))
    chk(not plans, "no .kicad_plan in the export" + (f" (found {plans})" if plans else ""), {"message": "\n".join(plans)})
    for d, _, fs in os.walk(src):
        for f in fs:
            if f.endswith(".kicad_plan"):
                a, b = os.path.join(d, f), os.path.join(nat, os.path.relpath(os.path.join(d, f), src))
                lost = kept(json.load(open(a)), json.load(open(b))) if os.path.exists(b) else ["(gone)"]
                if not lost and open(a, "rb").read() != open(b, "rb").read():
                    print(f"NOTE {name}: the fork's save rewrote {f} (every value kept; defaults added)")
                chk(not lost, f"the project's {f} keeps every value through the fork's native save", {"lost": lost[:50], "diff": file_diff(a, b)})
    kpcb = os.path.join(k10, os.path.basename(pcb)) if pcb else None
    ksch = os.path.join(k10, os.path.basename(sch)) if sch else None

    # 4. the release KiCad on the export: loads clean, same ERC/DRC, exports
    t = time.time()
    if ksch:
        o = os.path.join(root, "stock", "erc.json"); rc, out = run(args.stock_cli, ["sch", "erc", "--format", "json", "--severity-all", "-o", o, ksch], senv, slog)
        c = complaints(out)
        chk(os.path.exists(o), "stock ERC runs on the export", {"message": out[-2000:]})
        chk(not c, "stock loads the schematic with no repair/version/unknown-token message", {"message": "\n".join(c), "diff": file_diff(sch, ksch)})
        if os.path.exists(o): stock_r["erc"] = o
        o = os.path.join(root, "stock", "netlist.net"); rc, out = run(args.stock_cli, ["sch", "export", "netlist", "-o", o, ksch], senv, slog)
        chk(rc == 0 and os.path.getsize(o) > 0 if os.path.exists(o) else False, "stock exports the netlist", {"message": out[-2000:]})
        c = complaints(out); chk(not c, "stock netlist export says nothing alarming", {"message": "\n".join(c)})
    if kpcb:
        o = os.path.join(root, "stock", "drc.json"); a = ["pcb", "drc", "--format", "json", "--severity-all"] + (["--schematic-parity"] if ksch else []) + ["-o", o, kpcb]
        rc, out = run(args.stock_cli, a, senv, slog)
        c = complaints(out)
        chk(os.path.exists(o), "stock DRC runs on the export", {"message": out[-2000:]})
        chk(not c, "stock loads the board with no repair/version/unknown-token message", {"message": "\n".join(c), "diff": file_diff(pcb, kpcb)})
        if os.path.exists(o): stock_r["drc"] = o
        g = os.path.join(root, "stock", "gerbers") + "/"; os.makedirs(g)
        rc, out = run(args.stock_cli, ["pcb", "export", "gerbers", "-o", g, kpcb], senv, slog)
        n = len([f for f in os.listdir(g) if os.path.getsize(os.path.join(g, f)) > 0])
        chk(rc == 0 and n > 0, f"stock exports gerbers ({n} files)", {"message": out[-2000:]})
        c = complaints(out); chk(not c, "stock gerber export says nothing alarming", {"message": "\n".join(c)})
    res["timings"]["stock_s"] = round(time.time() - t, 1)

    # 5. the fork's own checks on the export too: a difference the fork also sees on the copy is the export's (data lost or changed);
    # one it does not see is the two engines' (fork 10.99 vs release 10.0) on the same file
    os.makedirs(os.path.join(root, "fork-on-export"))
    fx = {}
    if ksch:
        o = os.path.join(root, "fork-on-export", "erc.json"); run(args.fork_cli, ["sch", "erc", "--format", "json", "--severity-all", "-o", o, ksch], fenv, flog)
        if os.path.exists(o): fx["erc"] = o
    if kpcb:
        o = os.path.join(root, "fork-on-export", "drc.json")
        run(args.fork_cli, ["pcb", "drc", "--format", "json", "--severity-all"] + (["--schematic-parity"] if ksch else []) + ["-o", o, kpcb], fenv, flog)
        if os.path.exists(o): fx["drc"] = o

    # 6. identical results
    key = lambda x: "|".join(map(str, x))
    for k in ("erc", "drc"):
        if k in fork_r and k in stock_r:
            a, b = tally(fork_r[k], project=name), tally(stock_r[k], project=name); c = tally(fx[k], project=name) if k in fx else None
            res[k] = {"fork": {key(x): n for x, n in sorted(a.items(), key=str)}, "stock": {key(x): n for x, n in sorted(b.items(), key=str)}}
            diff = {}
            for x in set(a) | set(b):
                if a.get(x, 0) != b.get(x, 0):
                    cause = "?" if c is None else ("export" if c.get(x, 0) != a.get(x, 0) else "engine")
                    diff[key(x)] = {"fork_native": a.get(x, 0), "stock_export": b.get(x, 0), "fork_export": None if c is None else c.get(x, 0), "cause": cause}
            kn = known()
            engine = {x: d for x, d in diff.items() if d["cause"] == "engine" and x in kn}
            for x in engine:
                diff[x]["known"] = kn[x]
            hard = {x: d for x, d in diff.items() if x not in engine}
            res.setdefault("engine_known", {}).update(engine)
            pend = pending(); named = [f"{x}: {pend[(name, x)]}" for x in hard if (name, x) in pend]
            chk(not hard, f"{k.upper()} identical, fork vs stock ({sum(a.values())} vs {sum(b.values())}{'; known engine differences: ' + ', '.join(engine) if engine else ''}{'; being chased: ' + '; '.join(named) if named else ''})", {"difference": hard})
            ea, eb = tally(fork_r[k], True, name), tally(stock_r[k], True, name)
            if ea != eb:
                res.setdefault("excluded_differ", {})[k] = {"fork": {key(x): n for x, n in ea.items()}, "stock": {key(x): n for x, n in eb.items()}}
                why = next((e["why"] for e in json.load(open(KNOWN)) if e.get("key") == "exclusions|unresolved_sheet"), "")
                print(f"NOTE {name}: {k.upper()} exclusions listed differently (fork {sum(ea.values())}, stock {sum(eb.values())}; stale ones in the .kicad_pro){'; known: ' + why if why else ''}")
    with open(os.path.join(root, "result.json"), "w") as f:
        json.dump(res, f, indent=1)
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fork-cli", required=True); ap.add_argument("--stock-cli", required=True)
    ap.add_argument("--out", required=True); ap.add_argument("--target", default="10.0")
    ap.add_argument("--as-is", action="store_true"); ap.add_argument("projects", nargs="+")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    allr = []
    for p in a.projects:
        try:
            allr.append(check(a, p))
        except Exception as ex:
            print(f"FAIL {p}: harness error {ex!r}"); allr.append({"project": p, "findings": [{"what": f"harness error {ex!r}"}], "checks": []})
    json.dump(allr, open(os.path.join(a.out, "summary.json"), "w"), indent=1)
    for r in allr:
        n = len(r["checks"]); f = len(r["findings"])
        print(f"{'PASS' if not f else 'FAIL'} {r['project']}: {n-f}/{n} checks, timings {r.get('timings')}")
    sys.exit(1 if any(r["findings"] for r in allr) else 0)


if __name__ == "__main__":
    main()
