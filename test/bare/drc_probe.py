#!/usr/bin/env python3
"""drc_probe.py --fork-cli F [--stock-cli S] --demo DIR --out OUT: does DRC see a short inside a zone fill (lane P's probe, Nov 13)?

On a COPY of the complex_hierarchy demo (DIR), a foreign-net track (Net-(R313-Pad1), 0.05 mm long, 0.1 mm wide, B.Cu) is put inside the
GND fill at (100.5, 100.9), where the fill's triangulation leaves real copper uncovered; a control track at (120, 90) sits in the same fill
where it does not. Three boards:
  rules   the demo's rules (0.508 mm): the track must be reported with actual 0 (it overlaps GND copper)
  tight   every rule at 0.1 mm (net classes, the GND zone; board minimum 0.05): the track must be reported at all (a short)
  control the control track, demo's rules: reported with actual 0 (the probe itself works)
The check is named `drc-zone-fill-short`. A miss is a FAIL (the gate's runtime is bp.12.1 or later since the pin moved; it was KNOWN-RED
on bp.10 until then). The runtime's version and whether it has "drc-zone-guard" are said with the result. With --stock-cli the `tight` and `rules` boards are exported with the fork's downgrade and stock KiCad's DRC is
asked too; whether it catches the short is recorded (NOTE), never a failure. Exit 1 on a FAIL.
"""
import argparse, json, os, re, shutil, subprocess, sys, uuid

NAME = "drc-zone-fill-short"
WHY = "DRC zone-fill triangulation, fixed in bp.12.1"
NET = "Net-(R313-Pad1)"


def env_for(root, cli):
    h = os.path.join(root, "cfg-" + os.path.basename(os.path.dirname(os.path.dirname(os.path.realpath(cli)))))
    os.makedirs(h, exist_ok=True)
    e = {k: v for k, v in os.environ.items() if not k.startswith("KICAD")}
    e.update(HOME=h, XDG_CONFIG_HOME=os.path.join(h, ".config"), XDG_CACHE_HOME=os.path.join(h, ".cache"), XDG_DATA_HOME=os.path.join(h, ".local/share"))
    e.pop("DISPLAY", None); e.pop("WAYLAND_DISPLAY", None)
    return e


def runtime(cli):
    """(version, capabilities) from the runtime's manifest beside its bin/ (stock KiCad has none)"""
    root = os.path.dirname(os.path.dirname(os.path.realpath(cli)))
    try:
        m = json.load(open(os.path.join(root, "manifest.json")))
        return m.get("version", ""), m.get("capabilities", []) or []
    except Exception:
        v = subprocess.run([cli, "--version"], capture_output=True, text=True).stdout.strip()
        return v, []


def guarded(version, caps):
    """whether this runtime must catch the short: bp.12.1 or later, or the capability"""
    if "drc-zone-guard" in caps:
        return True
    m = re.search(r"-bp\.(\d+)(?:\.(\d+))?", version)
    return bool(m) and (int(m.group(1)), int(m.group(2) or 0)) >= (12, 1)


def make(demo, dst, x, y, tight):
    shutil.copytree(demo, dst)
    pcb = os.path.join(dst, "complex_hierarchy.kicad_pcb"); s = open(pcb).read()
    m = re.search(r'\(net (\d+) "' + re.escape(NET) + r'"\)', s)
    net = f"(net {m.group(1)})" if m else f'(net "{NET}")'
    u = str(uuid.uuid4())
    seg = f'\n\t(segment (start {x} {y}) (end {x + 0.05} {y}) (width 0.1) (layer "B.Cu") {net} (uuid "{u}"))'
    i = s.rstrip().rindex(")"); s = s[:i] + seg + "\n" + s[i:]
    if tight:
        k = s.index('(net_name "GND")'); z = s.rfind("(zone", 0, k); j = s.index("(clearance ", z)
        e = s.index(")", j); s = s[:j] + "(clearance 0.1" + s[e:]
        pro = os.path.join(dst, "complex_hierarchy.kicad_pro"); d = json.load(open(pro))
        for c in d["net_settings"]["classes"]:
            c["clearance"] = 0.1
        d["board"]["design_settings"]["rules"]["min_clearance"] = 0.05
        json.dump(d, open(pro, "w"), indent=2)
    open(pcb, "w").write(s)
    return pcb, u


def drc(cli, env, pcb, out):
    p = subprocess.run([cli, "pcb", "drc", "--format", "json", "--severity-all", "-o", out, pcb], env=env, capture_output=True, text=True, timeout=900)
    return json.load(open(out)) if os.path.exists(out) else {"error": (p.stdout + p.stderr)[-600:]}


def hits(report, u):
    """the copper violations (clearance, shorting) that name the probe track (by its uuid, or as a track of its net on B.Cu); its width,
    dangling end and missing connection always fire and say nothing about the fill"""
    out = []
    for v in report.get("violations", []):
        if v.get("type") not in ("clearance", "shorting_items"):
            continue
        for i in v.get("items", []):
            if i.get("uuid") == u or (NET in i.get("description", "") and "Track" in i.get("description", "") and "B.Cu" in i.get("description", "")):
                out.append({"type": v.get("type"), "description": v.get("description")}); break
    return out


def actual0(h):
    return any(x["type"] in ("shorting_items",) or re.search(r"actual 0\.0+ mm", x["description"] or "") for x in h)


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--fork-cli", required=True); ap.add_argument("--stock-cli"); ap.add_argument("--demo", required=True)
    ap.add_argument("--out", required=True); a = ap.parse_args()
    shutil.rmtree(a.out, ignore_errors=True); os.makedirs(a.out)
    ver, caps = runtime(a.fork_cli); must = guarded(ver, caps); fe = env_for(a.out, a.fork_cli)
    res = {"check": NAME, "runtime": ver, "guarded": must, "cases": {}}
    cases = {"rules": (100.5, 100.9, False, actual0), "tight": (100.5, 100.9, True, bool), "control": (120.0, 90.0, False, actual0)}
    for k, (x, y, tight, good) in cases.items():
        pcb, u = make(a.demo, os.path.join(a.out, k), x, y, tight)
        h = hits(drc(a.fork_cli, fe, pcb, os.path.join(a.out, k + "-fork.json")), u)
        res["cases"][k] = {"fork": h, "fork_ok": bool(good(h))}
        if a.stock_cli and k != "control":
            k10 = os.path.join(a.out, k + "-kicad10")
            subprocess.run([a.fork_cli, "project", "downgrade", "--target", "10.0", "--force", "-o", k10, os.path.join(a.out, k, "complex_hierarchy.kicad_pro")], env=fe, capture_output=True)
            sh = hits(drc(a.stock_cli, env_for(a.out, a.stock_cli), os.path.join(k10, "complex_hierarchy.kicad_pcb"), os.path.join(a.out, k + "-stock.json")), u)
            res["cases"][k].update(stock=sh, stock_ok=bool(good(sh)))
    json.dump(res, open(os.path.join(a.out, "result.json"), "w"), indent=1)
    c = res["cases"]; fail = 0
    if not c["control"]["fork_ok"]:
        print(f"FAIL {NAME}: the control track at (120, 90) was not reported at actual 0: the probe does not work ({c['control']['fork']})"); fail = 1
    caught = c["rules"]["fork_ok"] and c["tight"]["fork_ok"]
    seen = f"demo rules: {c['rules']['fork'] or 'nothing'}; 0.1 mm rules: {c['tight']['fork'] or 'nothing'}"
    if caught:
        print(f"PASS {NAME}: DRC ({ver}) reports the short inside the GND fill ({seen})")
    else:
        print(f"FAIL {NAME}: DRC ({ver}{', drc-zone-guard' if must else ', no drc-zone-guard'}) misses the short inside the GND fill ({seen})"); fail = 1
    if a.stock_cli:
        for k in ("rules", "tight"):
            print(f"NOTE {NAME}: stock KiCad on the downgraded '{k}' board {'catches' if c[k].get('stock_ok') else 'misses'} it ({c[k].get('stock') or 'nothing'})")
    sys.exit(fail)


if __name__ == "__main__":
    main()
