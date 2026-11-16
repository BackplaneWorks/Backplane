#!/usr/bin/env python3
"""The libraries release-gate row (Backplane 0.16.0 waits on it), one definition for Linux, macOS and Windows (lanes J, M, Carbon).

A Backplane KiCad runtime ships KiCad's standard libraries and Backplane_KiCad_Library (footprints and 3D models only: 6 libraries,
223 footprints), registered in its default global tables. On a clean home, with only the managed runtime, through the hub:

  L1  the runtime is the hub's managed one (installed by the hub, or placed where its installer puts it); macOS: no quarantine and
      codesign --verify --deep --strict (hook sign_check)
  L2  the standard symbols, footprints, templates, 3D models and default tables, and the Backplane library's footprints and models,
      are inside the runtime: where each sits, with counts (hook runtime_paths)
  L3  after first use KiCad's global sym-lib-table and fp-lib-table exist on the clean config home, hold the standard entries and the
      Backplane entries, and every URI resolves, ${BACKPLANE_KICAD_LIBRARY} expanded from where the runtime defines it (said which):
      kicad_common.json's environment, else the runtime's launcher / KiCad's own default; never the test's environment
  L4  an agent-style kicad_edit makes the design: C1 Device:C on Capacitor_SMD:C_0603_1608Metric, C2 Device:C on
      Backplane_Passives_Nominal:C_0805_2012Metric, in parallel (two nets, passive pins only); the board updated from the schematic,
      both footprints inside a small Edge.Cuts rectangle, both nets routed
  L5  C2's model (${BACKPLANE_KICAD_LIBRARY}/3dmodels/Capacitor_SMD.3dshapes/C_0805_2012Metric.step) is in the runtime and a full
      board STEP export includes it (and C1's)
  L6  the embedded symbol and footprint choosers list both sets (hook chooser_lists; SKIP where a platform has none yet)
  L7  ERC 0 errors and 0 warnings; DRC with schematic parity 0 violations and 0 unconnected: by kicad-cli and in the editor (the
      edit's own check)
  L8  (phase b) a person's fp-lib-table with a custom entry and no Backplane entry: after the hub's step (libtables.rs, at hub start)
      exactly the six Backplane fp entries are added, every byte before the table's close is kept, and sym-lib-table is unchanged
  L9  the manifest's backplaneLibrary pin matches KICAD_LIBRARY_SOURCES.txt (SKIP until the runtime has the pin, batch 14)

Platform-neutral: the hub over its RPC (test/kicad_embed/lib/hub_rpc.mjs, bun), the agent's tools through the stand-in agent's
`mcpcall:` messages (the hub's own MCP path, as an agent), the runtime's kicad-cli, table parsing. Usage:
  libraries_row.py <phase a|b> <config.json>
config.json: {out, port, token_file, hub_rpc: [bun, mjs], standin_log, thread, runtime_root, kicad_cli, config_root,
  settings_version, project_dir, pcb, sch, platform (linux|macos|windows); phase b: table_before, sym_before}
Each step prints PASS/FAIL/SKIP/INFO lines with its L number; the exit code is the number of FAILs (0 = the row holds).
"""
import json, os, re, subprocess, sys, time
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import libgate_design as D  # the design and the bar, one copy for the three platforms (Carbon's)

PLATFORM_HOOKS = {}
FAILS = 0


def say(kind, step, what, detail=""):
    global FAILS
    if kind == "FAIL": FAILS += 1
    print(f"{kind} {step} {what}" + (f" ({detail})" if detail else ""), flush=True)


def check(step, what, ok, detail=""): say("PASS" if ok else "FAIL", step, what, "" if ok else detail)


# ---- the hub and the agent ------------------------------------------------------------------------------------------------
class Hub:
    def __init__(self, c):
        self.c = c; self.n = 0

    def rpc(self, steps, timeout=60):
        r = subprocess.run([*self.c["hub_rpc"], str(self.c["port"]), self.c["token_file"], json.dumps(steps)], capture_output=True, text=True, timeout=timeout)
        out = []
        for line in r.stdout.splitlines():
            try: out.append(json.loads(line))
            except Exception: pass
        return out

    def ask(self, tool, args, wait=240):
        """One agent tool call (the stand-in agent turns `mcpcall:` into the hub's MCP call); its answer, decoded."""
        log = self.c["standin_log"]
        before = open(log, errors="replace").read().count("\nMCPCALL ") if os.path.exists(log) else 0
        self.n += 1
        text = "mcpcall:" + json.dumps({"name": tool, "arguments": args})
        self.rpc([{"method": "turn.start", "params": {"thread": self.c["thread"], "text": text, "msg": f"mlib{time.time_ns()}{self.n}"}}])
        t0 = time.time()
        while time.time() - t0 < wait:
            s = open(log, errors="replace").read() if os.path.exists(log) else ""
            if s.count("\nMCPCALL ") > before: break
            time.sleep(0.3)
        time.sleep(2)
        lines = [l for l in open(log, errors="replace").read().splitlines() if l.startswith(f"MCPCALL {tool} ")]
        if not lines: return {"error": f"no answer to {tool}"}
        try:
            o = json.loads(lines[-1].split(" ", 2)[2]); t = o["result"]["content"][0]["text"]
            try: v = json.loads(t)
            except Exception: v = {"text": t}
            if o["result"].get("isError"): v = v if isinstance(v, dict) else {"error": t}; v.setdefault("error", t)
            return v
        except Exception as e:
            return {"error": f"{e}: {lines[-1][:300]}"}


def run(cmd, timeout=600, env=None):
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, env=env)
    return r.returncode, r.stdout + r.stderr


# ---- tables ---------------------------------------------------------------------------------------------------------------
LIB_RE = re.compile(r'\(lib\s+\(name\s+"?([^")]+)"?\)\s*\(type\s+"?([^")]+)"?\)\s*\(uri\s+"?([^")]*)"?\)')


def table_entries(text):
    return [{"name": m.group(1), "type": m.group(2), "uri": m.group(3)} for m in LIB_RE.finditer(text or "")]


def env_source(c):
    """Where ${BACKPLANE_KICAD_LIBRARY} (and KiCad's own path variables) come from for this runtime, and their values."""
    vars_, src = {}, {}
    common = os.path.join(c["config_root"], c["settings_version"], "kicad_common.json")
    try: ev = (json.load(open(common)).get("environment") or {}).get("vars") or {}
    except Exception: ev = {}
    for k, v in ev.items(): vars_[k] = v; src[k] = f"kicad_common.json ({common})"
    defaults = PLATFORM_HOOKS["runtime_defaults"](c)
    for k, (v, why) in defaults.items():
        if k not in vars_: vars_[k] = v; src[k] = why
    return vars_, src


def expand(uri, vars_, base=""):
    def sub(m): return vars_.get(m.group(1), m.group(0))
    u = re.sub(r"\$\{([A-Za-z0-9_]+)\}", sub, uri)
    if base and not os.path.isabs(u) and "${" not in u: u = os.path.join(base, u)
    return u


# ---- steps ----------------------------------------------------------------------------------------------------------------
STD_SYM = ["Device", "power"]
STD_FP = ["Capacitor_SMD", "Resistor_SMD"]
BP_FP = ["Backplane_Logo", "Backplane_MountingHole", "Backplane_Passives_Least", "Backplane_Passives_Most", "Backplane_Passives_Nominal", "Backplane_Shoggoth"]
C2_FP = "Backplane_Passives_Nominal:C_0805_2012Metric"
C1_FP = "Capacitor_SMD:C_0603_1608Metric"
C2_MODEL = "3dmodels/Capacitor_SMD.3dshapes/C_0805_2012Metric.step"


def l1(c):
    root = os.path.realpath(c["runtime_root"])
    managed = c.get("managed_parent", "")
    check("L1", "the runtime is the hub's managed one", bool(managed) and root.startswith(os.path.realpath(managed)), f"{root} not under {managed}")
    for kind, what, detail in PLATFORM_HOOKS["sign_check"](c): say(kind, "L1", what, detail)


def l2(c):
    paths = PLATFORM_HOOKS["runtime_paths"](c)
    for name, (p, pattern, least) in paths.items():
        n = len([e for e in os.listdir(p) if re.search(pattern, e)]) if os.path.isdir(p) else -1
        check("L2", f"{name} in the runtime ({p}: {n})", n >= least, f"{n} found, want {least} or more")
    lib = paths.get("Backplane footprint libraries", ("", "", 0))[0]
    total = sum(len([f for f in os.listdir(os.path.join(lib, d)) if f.endswith(".kicad_mod")]) for d in os.listdir(lib) if d.endswith(".pretty")) if os.path.isdir(lib) else 0
    check("L2", f"Backplane_KiCad_Library has its 223 footprints ({total})", total == 223, f"{total}")


def l3(c, hub):
    # first use: the hub's KiCad started for the project (its launcher seeds the tables a clean home lacks)
    st = hub.ask("kicad_ipc_status", {"path": c["pcb"]})
    say("INFO", "L3", f"first use: kicad_ipc_status {json.dumps(st)[:200]}")
    vars_, src = env_source(c)
    bpl = vars_.get("BACKPLANE_KICAD_LIBRARY")
    check("L3", "BACKPLANE_KICAD_LIBRARY is defined by the runtime or KiCad's config", bool(bpl), "not defined by kicad_common.json nor the runtime")
    if bpl: say("INFO", "L3", f"BACKPLANE_KICAD_LIBRARY = {bpl} (from {src['BACKPLANE_KICAD_LIBRARY']})")
    if os.environ.get("BACKPLANE_KICAD_LIBRARY"): say("INFO", "L3", "the test's own environment has BACKPLANE_KICAD_LIBRARY: not used")
    for table, std, bp in (("sym-lib-table", STD_SYM, []), ("fp-lib-table", STD_FP, BP_FP)):
        p = os.path.join(c["config_root"], c["settings_version"], table)
        ok = os.path.isfile(p); check("L3", f"the global {table} exists after first use", ok, p)
        if not ok: continue
        ents = table_entries(open(p).read())
        names = [e["name"] for e in ents]
        # the standard libraries: listed directly, or through KiCad's nested table ("KiCad", type Table)
        std_names, unresolved = set(names), []
        for e in ents:
            u = expand(e["uri"], vars_)
            if e["type"] == "Table":
                if os.path.isfile(u): std_names |= {x["name"] for x in table_entries(open(u).read())}
                else: unresolved.append(f"{e['name']} -> {u}")
                for x in (table_entries(open(u).read()) if os.path.isfile(u) else []):
                    if x["name"] in std and not os.path.exists(expand(x["uri"], vars_, os.path.dirname(u))): unresolved.append(f"{x['name']} -> {expand(x['uri'], vars_)}")
            elif not os.path.exists(u): unresolved.append(f"{e['name']} -> {u}")
        check("L3", f"{table} has the standard libraries ({', '.join(std)})", all(s in std_names for s in std), f"has {sorted(std_names)[:12]}")
        if bp: check("L3", f"{table} has the six Backplane entries", all(b in names for b in bp), f"missing {[b for b in bp if b not in names]}")
        else: check("L3", f"{table} has no Backplane entry (the library has no symbols)", not any(n.startswith("Backplane_") for n in names), f"{[n for n in names if n.startswith('Backplane_')]}")
        check("L3", f"every {table} URI resolves", not unresolved, "; ".join(unresolved[:6]))


def l4(c, hub):
    """The design, by libgate_design (Carbon's, one copy for the three platforms): the agent's kicad_edit on the schematic, the board
    updated from it (ImportNetlist), the parts moved inside the outline and the nets routed."""
    sch, pcb = c["sch"], c["pcb"]
    r = hub.ask("kicad_edit", {"path": sch, "ops": D.schematic_ops(), "message": "two capacitors in parallel"})
    say("INFO", "L4", f"schematic edit: {json.dumps(r)[:300]}")
    check("L4", "kicad_edit put C1 and C2 (Device:C) in the schematic in parallel on N1/N2", "error" not in r, json.dumps(r)[:300])
    net = os.path.join(c["out"], "libgate.net")
    rc, out = run([c["kicad_cli"], "sch", "export", "netlist", "--format", "kicadsexpr", "-o", net, sch])
    check("L4", "the schematic's netlist exports", rc == 0 and os.path.isfile(net), out[-300:])
    r = hub.ask("kicad_edit", {"path": pcb, "ops": D.netlist_ops(net), "message": "board from the schematic"}, wait=600)
    say("INFO", "L4", f"netlist import: {json.dumps(r)[:300]}")
    check("L4", "kicad_edit updated the board from the schematic (ImportNetlist)", "error" not in r, json.dumps(r)[:300])
    r = hub.ask("kicad_edit", {"path": pcb, "ops": D.board_ops(), "message": "both parts inside the outline, N1 and N2 routed"}, wait=600)
    say("INFO", "L4", f"place and route: {json.dumps(r)[:400]}")
    check("L4", "kicad_edit placed both parts and routed N1 and N2", "error" not in r, json.dumps(r)[:300])
    text = open(pcb).read()
    check("L4", f"C2's footprint is the Backplane library's ({C2_FP})", f'"{C2_FP}"' in text, "not in the saved board")
    check("L4", f"C1's footprint is the standard one ({C1_FP})", f'"{C1_FP}"' in text, "not in the saved board")
    check("L4", "both nets routed (tracks in the saved board)", text.count("(segment") >= 2, f"{text.count('(segment')} segments")
    return r


def l5(c):
    vars_, _ = env_source(c)
    model = os.path.join(vars_.get("BACKPLANE_KICAD_LIBRARY", ""), C2_MODEL)
    check("L5", "C2's 3D model is in the runtime", os.path.isfile(model), model)
    step, bare = os.path.join(c["out"], "libgate.step"), os.path.join(c["out"], "libgate-board.step")
    rc, out = run([c["kicad_cli"], "pcb", "export", "step", "--force", "-o", step, c["pcb"]], timeout=900)
    rc2, out2 = run([c["kicad_cli"], "pcb", "export", "step", "--force", "--board-only", "-o", bare, c["pcb"]], timeout=900)
    ok = rc == 0 and os.path.isfile(step) and rc2 == 0 and os.path.isfile(bare)
    check("L5", "the board exports to STEP (with models, and board only)", ok, (out + out2)[-300:])
    if not ok: return
    missing = [l for l in out.splitlines() if re.search(r"(could not|cannot|failed|no).*(model|3D)|not found.*\.(step|wrl)", l, re.I)]
    check("L5", "no missing-model message in the export", not missing, "; ".join(missing[:3]))
    r = D.step_has_model(open(step, errors="replace").read(), open(bare, errors="replace").read(), open(model, errors="replace").read() if os.path.isfile(model) else "")
    check("L5", f"the board's STEP holds C2's model (by {r['by']})", r["ok"], json.dumps(r))


def l6(c):
    for kind, what, detail in PLATFORM_HOOKS["chooser_lists"](c): say(kind, "L6", what, detail)


def l7(c, hub):
    erc = os.path.join(c["out"], "erc.json"); drc = os.path.join(c["out"], "drc.json")
    rc, out = run([c["kicad_cli"], "sch", "erc", "--format", "json", "--severity-all", "-o", erc, c["sch"]])
    e = D.count_cli(json.load(open(erc))) if os.path.isfile(erc) else None
    check("L7", "kicad-cli ERC: 0 errors and 0 warnings", e is not None and e["violations"] == 0, json.dumps(e) if e else out[-300:])
    rc, out = run([c["kicad_cli"], "pcb", "drc", "--format", "json", "--severity-all", "--schematic-parity", "-o", drc, c["pcb"]])
    d = D.count_cli(json.load(open(drc))) if os.path.isfile(drc) else None
    check("L7", "kicad-cli DRC with schematic parity: 0 violations, 0 unconnected, 0 parity", d is not None and d["violations"] == d["unconnected"] == d["parity"] == 0, json.dumps(d) if d else out[-300:])
    # in the editor: RunChecks (DRC with parity, then ERC), then its markers
    for kind, extra in (("FK_PCB", {"parity": True}), ("FK_SCH", {})):
        r = hub.ask("kicad_ipc", {"path": c["pcb"] if kind == "FK_PCB" else c["sch"], "type": "kiapi.backplane.embed.RunChecks", "message": {"kind": kind, **extra}})
        say("INFO", "L7", f"RunChecks {kind}: {json.dumps(r)[:200]}")
        if "error" in r: check("L7", f"in the editor: {kind} checks run", False, json.dumps(r)[:200]); continue
        m = None
        for _ in range(20):
            time.sleep(3)
            ans = hub.ask("kicad_ipc", {"path": c["pcb"] if kind == "FK_PCB" else c["sch"], "type": "kiapi.backplane.embed.GetMarkers", "message": {"kind": kind}})
            m = D.count_markers(ans)
            if m is None and "error" not in ans and "GetMarkersResponse" in json.dumps(ans): m = {"count": 0, "types": []}  # no markers: proto3 leaves the empty list out
            if m is not None: break
        what = "DRC with parity" if kind == "FK_PCB" else "ERC"
        check("L7", f"in the editor: {what} has no marker", m is not None and m["count"] == 0, json.dumps(m))


def l8(c):
    p = os.path.join(c["config_root"], c["settings_version"], "fp-lib-table"); ps = os.path.join(c["config_root"], c["settings_version"], "sym-lib-table")
    before = open(c["table_before"]).read(); after = open(p).read() if os.path.isfile(p) else ""
    head = before.rstrip()
    head = head[:head.rfind(")")]  # everything before the table's closing parenthesis
    check("L8", "every byte of the person's fp-lib-table before its close is kept", after.startswith(head), f"first difference at byte {next((i for i, (a, b) in enumerate(zip(head, after)) if a != b), min(len(head), len(after)))}")
    added = [e["name"] for e in table_entries(after[len(head):])]
    check("L8", "exactly the six Backplane fp entries are added", sorted(added) == sorted(BP_FP), f"added {added}")
    check("L8", "the person's custom entry is still there", "My_Parts" in [e["name"] for e in table_entries(after)], "")
    check("L8", "sym-lib-table is unchanged", (open(ps).read() if os.path.isfile(ps) else None) == open(c["sym_before"]).read(), "changed")


def l9(c):
    """The manifest's backplaneLibrary pin against KICAD_LIBRARY_SOURCES.txt's block for that repository: every key both name agrees
    (repository, version, commit); the pin's sha256 (which the sources file does not list) against the library's own SHA256SUMS."""
    m = json.load(open(os.path.join(c["runtime_root"], "manifest.json")))
    pin = m.get("backplaneLibrary")
    if not pin: say("SKIP", "L9", "the manifest has no backplaneLibrary pin yet (batch 14)"); return
    blocks, cur = [], {}
    for line in open(os.path.join(c["runtime_root"], "KICAD_LIBRARY_SOURCES.txt")).read().splitlines():
        if "=" not in line: continue
        k, v = line.split("=", 1)
        if k == "repository" and cur: blocks.append(cur); cur = {}
        cur[k.strip()] = v.strip()
    if cur: blocks.append(cur)
    src = next((b for b in blocks if b.get("repository") == pin.get("repository")), None)
    check("L9", "KICAD_LIBRARY_SOURCES.txt lists the pinned Backplane library repository", src is not None, f"pin {pin.get('repository')}")
    if src is None: return
    shared = [k for k in pin if k in src]
    diff = {k: (pin[k], src[k]) for k in shared if str(pin[k]) != src[k]}
    check("L9", f"the manifest's backplaneLibrary pin matches KICAD_LIBRARY_SOURCES.txt ({', '.join(shared)})", len(shared) >= 2 and not diff, json.dumps(diff))
    # the pin's sha256 is the library's own SHA256SUMS file's (inside the runtime's backplane-library), not in the sources file
    sha = str(pin.get("sha256") or "")
    check("L9", "the pin's sha256 is 64 hex digits", bool(re.fullmatch(r"[0-9a-f]{64}", sha)), sha or "none")
    lib = os.path.dirname(PLATFORM_HOOKS["runtime_paths"](c)["Backplane footprint libraries"][0])
    sums = os.path.join(lib, "SHA256SUMS")
    got = __import__("hashlib").sha256(open(sums, "rb").read()).hexdigest() if os.path.isfile(sums) else ""
    check("L9", "it is the sha256 of the library's own SHA256SUMS", bool(got) and got == sha, f"{sums}: {got or 'missing'}")


# ---- platform hooks (Linux here; macOS and Windows fill theirs) -------------------------------------------------------------
def linux_runtime_defaults(c):
    root = c["runtime_root"]; sk = os.path.join(root, "share", "kicad")
    launcher = os.path.join(root, "bin", "kicad-cli")
    lt = open(launcher, errors="replace").read() if os.path.isfile(launcher) else ""
    d = {}
    if "BACKPLANE_KICAD_LIBRARY" in lt: d["BACKPLANE_KICAD_LIBRARY"] = (os.path.join(sk, "backplane-library"), f"the runtime's launcher ({launcher})")
    for k, sub in (("KICAD10_SYMBOL_DIR", "symbols"), ("KICAD10_FOOTPRINT_DIR", "footprints"), ("KICAD10_3DMODEL_DIR", "3dmodels"), ("KICAD10_TEMPLATE_DIR", "template")):
        d[k] = (os.path.join(sk, sub), "KiCad's default (the runtime's share/kicad)")
    return d


def linux_runtime_paths(c):
    sk = os.path.join(c["runtime_root"], "share", "kicad"); bl = os.path.join(sk, "backplane-library")
    return {"standard symbols": (os.path.join(sk, "symbols"), r"\.kicad_sym(dir)?$", 50), "standard footprints": (os.path.join(sk, "footprints"), r"\.pretty$", 50),
            "standard 3D models": (os.path.join(sk, "3dmodels"), r"\.3dshapes$", 20), "templates": (os.path.join(sk, "template"), r".", 1),
            "default global tables": (sk, r"^(sym|fp)-lib-table$", 2),
            "Backplane footprint libraries": (os.path.join(bl, "libraries"), r"^Backplane_.*\.pretty$", 6), "Backplane 3D models": (os.path.join(bl, "3dmodels"), r"\.3dshapes$", 3)}


def macos_runtime_defaults(c):
    ss = os.path.join(c["runtime_root"], "KiCad.app", "Contents", "SharedSupport")
    d = {"BACKPLANE_KICAD_LIBRARY": (os.path.join(ss, "backplane-library"), "KiCad's built-in default for the bundle (global-tables-seeded): <SharedSupport>/backplane-library; "
                                     "KiCad.app started by the hub has no shell environment, so nothing else sets it")}
    for k, sub in (("KICAD10_SYMBOL_DIR", "symbols"), ("KICAD10_FOOTPRINT_DIR", "footprints"), ("KICAD10_3DMODEL_DIR", "3dmodels"), ("KICAD10_TEMPLATE_DIR", "template")):
        d[k] = (os.path.join(ss, sub), "KiCad's default (the bundle's SharedSupport)")
    return d


def macos_runtime_paths(c):
    ss = os.path.join(c["runtime_root"], "KiCad.app", "Contents", "SharedSupport"); bl = os.path.join(ss, "backplane-library")
    return {"standard symbols": (os.path.join(ss, "symbols"), r"\.kicad_sym(dir)?$", 50), "standard footprints": (os.path.join(ss, "footprints"), r"\.pretty$", 50),
            "standard 3D models": (os.path.join(ss, "3dmodels"), r"\.3dshapes$", 20), "templates": (os.path.join(ss, "template"), r".", 1),
            "default global tables": (ss, r"^(sym|fp)-lib-table$", 2),
            "Backplane footprint libraries": (os.path.join(bl, "libraries"), r"^Backplane_.*\.pretty$", 6), "Backplane 3D models": (os.path.join(bl, "3dmodels"), r"\.3dshapes$", 3)}


def macos_sign_check(c):
    app = os.path.join(c["runtime_root"], "KiCad.app")
    q = subprocess.run(["xattr", "-r", c["runtime_root"]], capture_output=True, text=True).stdout.count("com.apple.quarantine")
    cs = subprocess.run(["codesign", "--verify", "--deep", "--strict", app], capture_output=True, text=True)
    return [("PASS" if q == 0 else "FAIL", "nothing in the runtime is quarantined", f"{q} quarantined"),
            ("PASS" if cs.returncode == 0 else "FAIL", "KiCad.app passes codesign --verify --deep --strict, the libraries inside", (cs.stderr or "").strip()[:300])]


PLATFORM = {
    "macos": {"runtime_defaults": macos_runtime_defaults, "runtime_paths": macos_runtime_paths, "sign_check": macos_sign_check,
              "chooser_lists": lambda c: [("SKIP", "the choosers are opened in the pane by mac_pane_e2e.py (MAC_MODE=libs, its L6 rows and screenshots)", "")]},
    "linux": {"runtime_defaults": linux_runtime_defaults, "runtime_paths": linux_runtime_paths,
              "sign_check": lambda c: [("SKIP", "signing (macOS and Windows only)", "")],
              "chooser_lists": lambda c: [("SKIP", "the choosers' lists are read on the GUI rows (macOS, Windows); Linux reads the tables (L3)", "")]},
}


def main():
    phase, cfg = sys.argv[1], json.load(open(sys.argv[2]))
    PLATFORM_HOOKS.update(PLATFORM[cfg.get("platform", "linux")])
    hub = Hub(cfg)
    if phase == "a":
        l1(cfg); l2(cfg); l3(cfg, hub); l4(cfg, hub); l5(cfg); l6(cfg); l7(cfg, hub); l9(cfg)
    else:
        l8(cfg)
    print(f"libraries_row {phase}: {FAILS} failed", flush=True)
    sys.exit(min(FAILS, 100))


if __name__ == "__main__":
    main()
