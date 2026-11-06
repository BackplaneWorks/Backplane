#!/usr/bin/env python3
"""Feature parity checklist generator (stream `audit`).

Reads git objects only (no builds, no hubs, no store). It

  1. extracts, from the Bend source at BASE (default `gpui`), every hub request method
     (the `*.methods` lists and the `"m"` literals in src/server), every HTTP route and every
     MCP tool (`Tool.def("name"`);
  2. extracts, from the Rust hub (native/host/src) at BASE and at every `gpui-*` lane branch,
     the methods the hub registers (`fn methods` / `fn routes`, the literal `m == "..."` tests in
     hub.rs, the `view.*` family), the MCP tools it implements (`IMPLEMENTED`), and which agent
     providers are more than a stub;
  3. derives the Rust hub column of every feature row from those sets (the clients' columns
     and the evidence notes are written by hand in FEATURES below, next to the row);
  4. writes docs/gpui/PARITY.md (counts, rules, protocol gate, lanes, the table).

  python3 scripts/gpui/parity-features.py --write        # rewrite docs/gpui/PARITY.md
  python3 scripts/gpui/parity-features.py --sets         # print the extracted sets (JSON)
  python3 scripts/gpui/parity-features.py --check        # rows cover every Bend method/tool; exit 1 if not

Rules for the Rust hub column of a row (rules text is also written into PARITY.md):
  missing  none of the row's requests/tools/routes is registered on BASE or on any lane branch
           (an "unknown method" reply), or the only code is a stub (<= 3 lines).
  partial  some are registered, or all are registered on BASE but the row has no proof, or all are
           registered only on a lane branch that has not landed on BASE.
  done     every request/tool/route of the row is registered on BASE AND the row names a proof
           (a Rust test, a parity scenario, a results file).
Rows with no request (pure rendering) carry an explicit hub status written by hand with the same rules.
"""
import json, re, subprocess, sys, os, datetime

ROOT = subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True,
                               cwd=os.path.dirname(os.path.abspath(__file__))).strip()
BASE = os.environ.get("PARITY_BASE", "gpui")


def git(*a):
    return subprocess.run(["git", "-C", ROOT, *a], capture_output=True, text=True)


def show(ref, path):
    r = git("show", f"{ref}:{path}")
    return r.stdout if r.returncode == 0 else ""


def files(ref, prefix, ext):
    r = git("ls-tree", "-r", "--name-only", ref, prefix)
    return [f for f in r.stdout.split("\n") if f.endswith(ext)] if r.returncode == 0 else []


def strings(text):
    return re.findall(r'"((?:[^"\\]|\\.)*)"', text)


# ---------------------------------------------------------------- Bend side
def bend_sets(ref):
    core = {}  # method -> set(source names)
    def add(m, src):
        if re.fullmatch(r"[a-z][a-z0-9_]*(\.[A-Za-z0-9_]+)+|accounts", m):
            core.setdefault(m, set()).add(src)
    srcs = files(ref, "src", ".bend")
    for f in srcs:
        t = show(ref, f)
        # lists declared by a def named *methods* (Hub.methods, Hub.methods.git, Rv.methods, Fr.methods, Bots.methods, Ledger.methods)
        for m in re.finditer(r"^def ((?:Hub|Rv|Fr|Bots|Ledger)\.methods[A-Za-z.]*)\(\)[^\n]*:\n((?:[ \t]+[^\n]*\n)+)", t, re.M):
            for s in strings(m.group(2)):
                add(s, m.group(1))
        if f.startswith(("src/server/", "src/core/")):
            for s in re.findall(r'get_str\(o, "m", ""\),\s*"([A-Za-z_.]+)"', t):
                add(s, "server literal")
            for m in re.finditer(r'C\.Codec\.pos\(\[([^\]]*)\],\s*J\.Json\.get_str\(o, "m", ""\)\)', t):
                for s in strings(m.group(1)):
                    add(s, "server list")
            for s in re.findall(r'String\.eq\(m, "([a-z_.]+)"', t):
                add(s, "server m literal")
    tools = {}
    for f in srcs:
        for n in re.findall(r'Tool\.def\("([a-z_]+)"', show(ref, f)):
            tools[n] = f
    routes = set()
    for f in files(ref, "src/server", ".bend"):
        for s in re.findall(r'"(/[a-z0-9/_.-]+)"', show(ref, f)):
            if s in ROUTES_BEND:
                routes.add(s)
    return core, tools, sorted(routes)


# HTTP routes a client or agent uses (checked to exist in the Bend source by bend_sets)
ROUTES_BEND = ["/hello", "/ws", "/img", "/plot/info", "/files/get", "/files/zip", "/view/get", "/view/ver", "/view/git",
               "/mcp/", "/message-image", "/viewer-asset", "/voice/in", "/screen.jpg", "/auth/pair", "/auth/devices",
               "/auth/enroll", "/auth/invitations", "/auth/revoke", "/auth/known", "/auth/cache", "/client/capabilities",
               "/bots/pair", "/bots/dir", "/bots/link", "/bots/deliver", "/far/push", "/far/log", "/far/rpc",
               "/oauth/google", "/hook/", "/lock/", "/tasks", "/debug/perf", "/debug/diagnostics.json", "/debug/log",
               "/runs"]


# ---------------------------------------------------------------- Rust side
def strip_tests(t):
    m = re.search(r"#\[cfg\(test\)\]\s*(?:pub )?mod \w+\s*\{", t)  # an inline test module, not `mod tests;`
    return t if not m else t[:m.start()]


def rust_sets(ref):
    methods, routes, tools, providers = {}, set(), set(), {}
    for f in files(ref, "native/host/src", ".rs"):
        t = strip_tests(show(ref, f))
        for m in re.finditer(r"fn methods\(&self\)[^{]*\{\s*&\[(.*?)\]", t, re.S):
            for s in strings(m.group(1)):
                methods.setdefault(s, set()).add(f)
        for m in re.finditer(r"fn routes\(&self\)[^{]*\{\s*&\[(.*?)\]", t, re.S):
            for s in strings(m.group(1)):
                routes.add(s)
        if f.endswith("/hub.rs") or f == "native/host/src/hub.rs":
            for s in re.findall(r'\bm == "([a-z_.]+)"', t):
                methods.setdefault(s, set()).add(f)
        if f.endswith("wire/mod.rs"):
            for s in ("/hello", "/ws", "/client/capabilities"):
                if f'"{s}"' in t:
                    routes.add(s)
        if f.endswith("session/mod.rs"):
            for s in re.findall(r'"(view\.[a-z]+)"', t):
                methods.setdefault(s, set()).add(f)
        if f.endswith("agents/mcp.rs"):
            if '"/mcp/"' in t:
                routes.add("/mcp/")
            for m in re.finditer(r"const IMPLEMENTED[^=]*=\s*&\[(.*?)\];", t, re.S):
                tools.update(strings(m.group(1)))
        if "/providers/" in f and not f.endswith("mod.rs"):
            providers[os.path.basename(f)[:-3]] = len(show(ref, f).splitlines())
    if "view.open" in methods:
        for v in ("view.open", "view.update", "view.action", "view.page", "view.search", "view.message", "view.cancel"):
            methods.setdefault(v, set()).add("native/host/src/session/mod.rs")
    return {k: sorted(v) for k, v in methods.items()}, sorted(routes), sorted(tools), providers


def lane_branches():
    out = git("branch", "--list", "gpui-*", "--format=%(refname:short)").stdout.split()
    return sorted(out)


def collect():
    bm, bt, br = bend_sets(BASE)
    rb = rust_sets(BASE)
    lanes = {b: rust_sets(b) for b in lane_branches()}
    return {"bend_methods": {k: sorted(v) for k, v in sorted(bm.items())}, "bend_tools": bt, "bend_routes": br,
            "rust": {"methods": rb[0], "routes": rb[1], "tools": rb[2], "providers": rb[3]},
            "lanes": {b: {"methods": sorted(v[0]), "routes": v[1], "tools": v[2], "providers": v[3]} for b, v in lanes.items()}}


def where(S, kind, name):
    """'base' | lane branch names | ''"""
    key = {"m": "methods", "t": "tools", "r": "routes"}[kind]
    if name in S["rust"][key]:
        return "base"
    return ",".join(b for b, v in S["lanes"].items() if name in v[key])


def hub_status(S, row):
    """Derive the Rust hub status from the row's needs; returns (status, note)."""
    needs = [("m", x) for x in row["m"]] + [("t", x) for x in row["t"]] + [("r", x) for x in row["r"]] + [("p", x) for x in row["prov"]]
    if not needs:
        return row["hub"], ""
    have = []
    for k, n in needs:
        if k == "p":
            if S["rust"]["providers"].get(n, 0) > 5:
                have.append("base")
            else:
                have.append(",".join(b for b, v in S["lanes"].items() if v["providers"].get(n, 0) > 5))
        else:
            have.append(where(S, k, n))
    base = sum(1 for h in have if h == "base")
    lane = sum(1 for h in have if h and h != "base")
    miss = [n for (k, n), h in zip(needs, have) if not h]
    laned = sorted({x for h in have if h and h != "base" for x in h.split(",")})
    note = []
    if miss:
        note.append("no handler: " + ", ".join(miss[:6]) + (f" (+{len(miss) - 6})" if len(miss) > 6 else ""))
    if lane:
        note.append("code only on lane " + "/".join(laned))
    if base == len(needs):
        st = "done" if row.get("proof") else "partial"
        if not row.get("proof"):
            note.append("registered, no proof")
    elif base + lane == 0:
        st = "missing"
    else:
        st = "partial"
    if row.get("hub"):
        st = min_status(st, row["hub"])
    return st, "; ".join(note)


ORDER = {"missing": 0, "partial": 1, "done": 2, "n/a": 3}


def min_status(a, b):
    return a if ORDER[a] <= ORDER[b] else b


# ---------------------------------------------------------------- features
# FEATURES_BEGIN
# Every row: R(area, name, m=[requests], t=[mcp tools], r=[routes], prov=[providers], hub=<status when no request>,
#              pf=<proof key>, c="GIA" (GPUI, iOS, Android: d/p/m/n), lane=<owning lane>, ev=<evidence note>)
# Client letters follow the same rules as the hub (see RULES below). A client that depends on a hub request that the
# Rust hub lacks keeps its own status here (what the client does against any conformant hub) and the row is flagged
# [hub-blocked] in the table.
PF = {
    "agents": "agents/tests.rs (704 lines), results/agents/rust, test/gpui/agents/fixtures",
    "files": "results/files/parity-2.json (175/178 identical), bench-1.json",
    "git": "results/files/git-1.json (8/9 identical, 1 deliberate)",
    "plot": "results/files/plot-2.json, plot-native-*.json (sha256-identical frames)",
    "solid": "results/files/solid-e2e-*.json, mesh-*.json",
    "review": "kicad/review.rs 24 tests, bom.rs 16, checks.rs 16, release.rs 15; results/files/review-1,2.json",
    "kipc": "results/files/kipc-hub-e2e.txt, kicad/ipc.rs + kserve.rs tests",
    "state": "state/ tests (fam, liveness, todo, uber, star), model.rs vs store.db (895 entries, 100 pages)",
    "term": "term/ 27 tests on Boat (vt corpus = test/vt_test.bend, pty, actor, tools); results/term",
    "side": "sidebar.rs 5 tests, bin/sidebar-check, results/slim",
}
F = []


def R(area, name, m=(), t=(), r=(), prov=(), hub=None, pf=None, c="ppp", lane="", ev=""):
    F.append(dict(area=area, name=name, m=list(m), t=list(t), r=list(r), prov=list(prov), hub=hub, proof=PF.get(pf, pf),
                  c=c, lane=lane, ev=ev))


L = dict(core="core (bend)", side="sidebar (slim)", files="files", agents="agents", pses="parity-session", pedge="parity-edge",
         acp="prov-acp", msp="prov-msp", pi="prov-pi", state="state", soak="soak",
         cmd="NEW commands", sett="NEW settings", bots="NEW bots", net="NEW net", term="NEW term", mcp="NEW mcp2",
         orders="NEW orders", gscreens="NEW gpui-screens", gview="NEW gpui-viewers", gthread="NEW gpui-thread",
         ios="NEW ios-rust", andr="NEW android-rust", voice="NEW voice", i18n="NEW i18n")

# ---- Sidebar
A = "Sidebar"
R(A, "Projects and threads list, selection", m=["view.open", "view.update", "view.action"], hub="partial", pf="side", c="pdd", lane=L["side"],
  ev="Rust: view.* answered via the Bend presenter; parity-rust7 (gpui 0688158f6, uncapped) shows 1621 differences (gate; the old 127 was capped). GPUI: shots gpui-*-1-sidebar, rust-hub-first.png (drawn from Rust hub, bounded fields still absent). iOS/Android: shots/ios/list.png, shots/android/02-projects.png on the JS hub")
R(A, "Bounded sidebar pages (Newer / Older, 48 rows)", m=["view.action"], hub="partial", pf=None, c="pdd", lane=L["side"],
  ev="nativeSidebar fields (sidebarPages, threadsPage, *Count) absent in Rust replies: 30 of the 42 differences per session scenario in parity-rust6; android 09a-c and n300 shots on JS hub")
R(A, "Search / filter projects and threads", m=["view.action"], hub="partial", c="mpp", lane=L["gscreens"],
  ev="Find.* is client state, no request; GPUI draws a search glyph only (no field); phones: ProjectSearchIntent.swift, no shot")
R(A, "Older / quiet projects fold, faded rows", hub="partial", c="mpp", lane=L["side"], ev="presenter output, not compared on recorded logs with old projects; no phone shot")
R(A, "Active view and Settled fold (sidebar.view)", m=["setting.set"], c="mpp", lane=L["cmd"], ev="setting.set has no handler in Rust; Active.rows is presenter code")
R(A, "Row menu (right-click / long press): pin, settle, snooze, archive, delete", m=["thread.pin", "thread.unpin", "thread.settle", "thread.unsettle", "thread.snooze", "thread.archive", "thread.unarchive", "thread.delete"], c="mpp", lane=L["cmd"],
  ev="commands/ planners (33 Boat tests, JS-hub replay 0 differences: test/gpui/commands); GPUI draws presenter buttons only; no menu proof on phones")
R(A, "Pins", m=["thread.pin", "thread.unpin"], c="mpp", lane=L["cmd"], ev="commands/ planners, Boat tests + JS-hub replay (test/gpui/commands); archive/delete stop the process through agents::handle()")
R(A, "Settle / unsettle, autosettle", m=["thread.settle", "thread.unsettle"], hub="partial", c="mpp", lane=L["cmd"],
  ev="commands/ planners (33 Boat tests, JS-hub replay 0 differences: test/gpui/commands); state/liveness.rs folds settle state (8 tests); autosettle.version only seeded at start")
R(A, "Snooze", m=["thread.snooze"], c="mpp", lane=L["cmd"], ev="commands/ planners, Boat tests + JS-hub replay (test/gpui/commands); archive/delete stop the process through agents::handle()")
R(A, "Archive / unarchive", m=["thread.archive", "thread.unarchive"], c="mpp", lane=L["cmd"], ev="commands/ planners, Boat tests + JS-hub replay (test/gpui/commands); archive/delete stop the process through agents::handle()")
R(A, "Delete thread (asks first)", m=["thread.delete"], c="mpp", lane=L["cmd"], ev="commands/ planners, Boat tests + JS-hub replay (test/gpui/commands); archive/delete stop the process through agents::handle()")
R(A, "Rename thread / project", m=["thread.rename", "project.rename"], c="mpp", lane=L["cmd"], ev="commands/ planners, Boat tests + JS-hub replay (test/gpui/commands); archive/delete stop the process through agents::handle()")
R(A, "Fork thread, bring back", m=["thread.fork", "thread.back"], c="mpp", lane=L["cmd"], ev="commands/ planners (33 Boat tests, JS-hub replay 0 differences: test/gpui/commands); the fork's worktree is made by the worktree job (commands/worktree.rs), thread.branch is the server job's report")
R(A, "Reorder threads and projects", m=["thread.reorder", "project.reorder", "project.move"], c="mpp", lane=L["cmd"], ev="commands/ planners, Boat tests + JS-hub replay (test/gpui/commands); archive/delete stop the process through agents::handle()")
R(A, "New thread", m=["thread.create"], c="mpp", lane=L["cmd"], ev="commands/ planners, Boat tests + JS-hub replay (test/gpui/commands); archive/delete stop the process through agents::handle()")
R(A, "Add project (folder picker, mkdir, find)", m=["project.add", "fs.list", "fs.mkdir", "fs.find", "project.new", "project.create"], c="mpp", lane=L["cmd"],
  ev="commands/ planners (33 Boat tests, JS-hub replay 0 differences: test/gpui/commands); the picker scripts are FsJob's; phones' picker code exists, no shot")
R(A, "Remove project, restore on re-add", m=["project.remove"], c="mpp", lane=L["cmd"], ev="commands/ planners, Boat tests + JS-hub replay (test/gpui/commands); archive/delete stop the process through agents::handle()")
R(A, "Scratch project and threads (ephemeral, sweep)", hub="missing", c="ppp", lane=L["cmd"], ev="hub makes <home>/scratch and sweeps (Scratch.sweep); not in Rust")
R(A, "Thread tree: parent, detach, reparent", m=["thread.detach", "thread.reparent"], c="mpp", lane=L["cmd"], ev="commands/ planners (33 Boat tests, JS-hub replay 0 differences: test/gpui/commands); thread.worktree/note/local are the server job's reports, written by commands/worktree.rs (thread.merged/unrooted/titled belong to git and title jobs, not ported)")
R(A, "Project badge colour/icon editor", m=["setting.set"], c="mpp", lane=L["cmd"], ev="no Rust handler")
R(A, "Status colours, urgency order, counts", hub="partial", pf="side", c="pdd", lane=L["side"], ev="sidebar.rs/signal.rs/shelf.rs ported with tests; compared only through the 1621-difference gate")
R(A, "Machine switch / one list over machines", m=["view.action"], hub="missing", c="mpp", lane=L["net"], ev="needs far/links (see Machines)")

# ---- Platforms
A = "Platforms"
R(A, "Hub on macOS (darwin-arm64, darwin-x64)", hub="partial", c="ppp", lane=L["core"],
  ev="today's Bend hub runs on macOS; Rust: Linux-only calls (prctl PDEATHSIG, ptsname_r, /proc) are behind cfg with macOS fallbacks (agents/proc.rs: reap_orphans does nothing off Linux, so an orphaned agent is left running after a hub SIGKILL until exact env reading (KERN_PROCARGS2) or a pid file lands; term/pty.rs ptsname); `cargo check --bin backplane-hub` passes on hs-mac-mini (darwin-arm64, 2026-11-05); tests and a run on macOS not done")
R(A, "Hub on Windows (windows-x64)", hub="missing", c="ppp", lane=L["core"],
  ev="today's Bend hub is cross-built for windows-x64; the Rust hub is unix-only (libc flock/setsid/pty/ioctl, unix sockets, /proc): the plan and effort are in docs/gpui/PORT.md 'Platforms'")

# ---- Threads and timeline
A = "Threads and timeline"
R(A, "Open a thread, windowed history (view.page, anchors, newer/older)", m=["view.page", "view.open"], hub="partial", pf="state", c="pdd", lane=L["pses"],
  ev="model.rs pages equal store.db on 100 pages; parity session:* runs view.page; gate not zero. GPUI shows <=32 entries (README); phones tl-4..tl-40 shots")
R(A, "Messages, Markdown (lists, tables, code, quotes)", hub="partial", c="pdd", lane=L["gthread"], ev="presenter nodes. GPUI: inline styles missing (README known gaps); shots gpui-*-2-thread; phones Markdown.swift/.kt, thread shots")
R(A, "Streaming reply (deltas, live row)", m=["turn.start"], hub="partial", pf="agents", c="pdd", lane=L["pses"], ev="Rust delta.rs/live_delta; parity scenario with streamed deltas inside the gate")
R(A, "Tool-call folds, running label", hub="partial", c="mdd", lane=L["gthread"], ev="presenter; android 03b-tool-fold-open.png; GPUI draws no fold nodes distinct from text (unproven)")
R(A, "Message stamps (time, Finished, duration)", hub="done", pf="stamp.rs", c="ppp", lane=L["gthread"], ev="native/host/src/stamp.rs; clients draw the entry's stamp, no shot of it")
R(A, "Agent to-do list (To-do 3/7, edit, retire)", m=["todos.edit"], c="mpp", lane=L["cmd"], ev="state/todo.rs folds todos (2 tests) but todos.edit has no handler")
R(A, "Subagents panel, Subagents badge", hub="partial", pf="agents", c="mpp", lane=L["agents"], ev="agents/bg.rs ledger (Bg.*) writes the entries; presenter draws; no panel proof anywhere")
R(A, "Asks and approvals (Allow / session / deny, questions, plans)", m=["ask.answer"], t=["permission_prompt"], pf="agents", c="ppp", lane=L["gthread"], ev="agents/mcp.rs permit + answer, 'decisions_stay_in_the_thread' tests; GPUI has no ask UI; phones unproven | gpui: native/gpui/src/screens/asks.rs draws thread.asks (permission, plan, input, settings proposal, Backplane needs with per-ask typed box); unit tests asks::tests; no render shot yet")
R(A, "Stop a turn (interrupt)", m=["turn.interrupt"], pf="agents", c="ppp", lane=L["gthread"], ev="agents lifecycle/interrupt events equal the JS hub (results/agents); stop button only as a generic presenter button in GPUI")
R(A, "Send a message (composer, draft kept)", m=["turn.start"], pf="agents", c="dpd", lane=L["gthread"], ev="hub test turn.start commits MessagePosted; GPUI send in shots (rust-hub-first, gpui-dark-2); android 05-send-roundtrip.png; iOS shots/ios/send1.png")
R(A, "Long draft over 64 KB: typed, kept across restart, sent (bridge draft bound 0d930e939)", m=["turn.start"], c="ppp", hub="partial", lane=L["ios"], ev="iOS on the JS hub, bridge be6731574b: 30/70/100 KB drafts sent intact (results/ios/longdraft-hub.txt). The stuck send ('Updating conversation...', composer keeps the text, hub gets nothing) is EXPLAINED and fixed in the iOS app: a relaunch that restored a draft of 20 KB or more made SwiftUI's multi-line TextField stall the main thread 8-22 s per screen update (BPPerf display stage; the hub answered in under 62 ms and the bridge in under 15 ms), so view requests timed out; drafts over 8,000 bytes are now shown as a preview (kept and sent whole): display stage 22.6 s -> 0.48 s at 100 KB, 0 view failures at 60/100 KB, 59,999/70,000-character sends after relaunch reach the hub (results/ios/stuck*.txt, stuck-before/after). Composer cost, measured (JS hub, simulator, results/ios/keys): SwiftUI field 92 ms/key at 1 KB, 280 at 2, 650 at 4; UITextView sized without measuring the whole text: 22 ms at 0, 27 at 8, 38 at 20, 48 at 40, 40 at 60 KB; persisted-draft display stage 0.5 s flat from 0 to 60 KB (was 8-22 s); the field is a UITextView above all, preview only over 64,000 bytes. OPEN: Rust hub repeat (waiting on bend's rebuild), draft across a thread switch unconfirmed in a failing state (no failure at 60/70 KB in 4 runs), Android's half not done")
R(A, "Multi-line composer, paste, selection, IME", c="pdd", hub="n/a", lane=L["gthread"], ev="GPUI single-line input (README gap); phones native text fields | gpui: screens/composer.rs + edit.rs multi-line, wrapped, undo/redo, word/line selection, IME, paste; edit::tests; no render check yet")
R(A, "Queue while busy, steer, edit, remove, resume", m=["queue.move", "queue.remove", "queue.resume", "queue.steer", "queue.edit"], c="mpp", lane=L["cmd"], ev="commands/ planners (33 Boat tests, JS-hub replay 0 differences: test/gpui/commands); run through Agents::steer / start_one / start_queued; the agents lane owns the queue run")
R(A, "Model, effort and provider picker", m=["thread.provider", "thread.effort", "thread.modes"], c="mpd", lane=L["cmd"], ev="commands/ planners (33 Boat tests, JS-hub replay 0 differences: test/gpui/commands) (models.refresh is not mine); GPUI picker not drawn; android/ios pop/settings shots show the picker on JS hub")
R(A, "Slash commands and $skill / @ > % # popups", m=["skills.list"], hub="partial", c="dpd", lane=L["agents"], ev="slash popup drawn from presenter in GPUI (gpui-*-3-popup, side-*-popup); skills.list has no Rust handler (skills come from the view only); iOS pop.png, android 04-composer-popup.png")
R(A, "/btw side question", m=["thread.btw"], c="mpp", lane=L["cmd"], ev="no Rust handler, deliberately: the answer comes from the one-shot text model (OneShot.run with text.provider / text.model and its account scope), which is not ported; nothing is stubbed")
R(A, "/give hand-off, references, summaries", m=["bots.give", "summary.set"], c="mpp", lane=L["bots"], ev="no Rust handler")
R(A, "Images in messages (/message-image, lightbox)", r=["/message-image"], c="mpp", lane=L["files"], ev="route not registered in Rust; GPUI draws no image node (img => empty div)")
R(A, "Attachments (upload, file, paste image)", m=["attach.put", "attach.file"], c="ppp", lane=L["files"], ev="no Rust handler | gpui: paste/drop/picker upload via POST /attach (screens/attach.rs, attach::tests) but the Rust hub has no /attach route: hub-blocked, asked the files lane")
R(A, "File and web links, open in folder, copy path", m=["file.open"], pf="files", c="mpp", lane=L["gthread"], ev="files/misc.rs file.open (5 tests); client link menus unproven")
R(A, "Reading position, 'N new' pill, history jump", m=["view.action"], hub="partial", c="mpp", lane=L["pses"], ev="presenter + window fields; no shot")
R(A, "In-thread search (view.search) and cancel", m=["view.search", "view.cancel", "view.message"], hub="partial", c="mpp", lane=L["pses"], ev="session/mod.rs routes the names; behaviour unrecorded by the parity scenarios")
R(A, "Thread header tools (stop, pin, settle, fork, terminal, viewer)", m=["view.action"], hub="partial", c="ppp", lane=L["gthread"], ev="tools.bend list via presenter; handlers for the actions are in 'Sidebar'")
R(A, "Token and context stats, rate limits, usage page", hub="partial", pf="agents", c="ppp", lane=L["agents"], ev="UsageSet/context.usage events written by agents/lines.rs; usage page needs setting/allowance info; GPUI: the Context chip opens a panel with Context, Last turn and This thread from the thread's settings (screens/tokstats.rs, unit-tested; a real click opened and closed it on a private Xvfb); Claude speed rows, This session by model and the speed note are drawn (a real window on a private Xvfb showed all four sections); the presenter keeps those settings in its lean state (policy.rs)")
R(A, "Reconnect keeps drafts, resume from log position", m=["view.open"], hub="partial", c="ddd", lane=L["pedge"], ev="reconnect scenario: 1 difference (origin is hex in Rust vs decimal at in Bend, N19); android 07a-c; GPUI keeps session")
R(A, "Terminal-in-chat rows (emb)", m=["term.open", "term.input", "term.resize", "term.snap", "term.restart"], pf="term", c="mpp", lane=L["term"], ev="term/: pty, VT, term.* requests; frames {t:term,b:[..],reset}; GPUI row view unchecked")

# ---- Agents and providers
A = "Agents and providers"
R(A, "Claude (stream-json, one process per thread)", hub="done", pf="agents", c="ppp", lane=L["agents"], ev="agents/{proc,lines,prompt,mcp}.rs, replay fixtures equal event for event (lifecycle, agents, modes)")
R(A, "Codex (codex exec --json)", prov=["codex"], pf="agents", c="ppp", lane=L["agents"], ev="providers/codex.rs + agents/codex.rs (187 lines), tests.rs")
R(A, "Grok, OpenCode (ACP)", prov=["acp", "grok", "opencode"], pf="agents", c="ppp", lane=L["acp"], ev="stubs on gpui; WIP drivers on lane gpui-prov-acp (acp 187, opencode 281 lines)")
R(A, "Muse Code (MSP), Antigravity", prov=["msp", "muse", "agy"], c="ppp", lane=L["msp"], ev="drivers landed (providers/{msp,muse,agy}.rs): stand-in turns equal the JS hub event for event (muse turn/approval, antigravity turn/permission; fixtures test/gpui/agents/prov); UNPROVEN: Antigravity install, probe, sign-in/out and Muse/Antigravity model lists (no job host), real CLIs never run")
R(A, "Pi, Oh My Pi, bp/ask dialogs", prov=["pi", "omp", "dialog"], c="ppp", lane=L["pi"], ev="stubs on gpui; drivers on lane gpui-prov-pi/-pi2 (pi 366, omp 292, dialog 180 lines)")
R(A, "Interrupt, resume, restart-in-place, stale process refusal", m=["turn.interrupt"], pf="agents", c="ppp", lane=L["agents"], ev="agents lifecycle replay; Gen.refused in mcp.rs")
R(A, "Account management reachable from a client (the `accounts` grant)", hub="partial", c="mmm", lane=L["sett"], ev="hub side wired (settings/accounts_hub.rs note_request): a request in a view session that names `accountProfiles: 1` grants the connection until detach; `accounts` and `accounts.default` then answer, others get the named-account message. OPEN until a client sends accountProfiles:1 in its view requests and draws the page; the live profiles are read with accounts op list, not broadcast. Final gate journey \"Account management\" stays open until a client reaches it.")
R(A, "`&` model mention (&provider/model: autocomplete, validation, agent references)", hub="partial", c="mmm", lane=L["sett"], ev="NEW feature, not in the JS hub: hub side = info models.all {provider:[{value,label,ok,why}]} (settings/models.rs), setting models.fav (JSON list of provider and provider/model ids, written with setting.set), settings/mentions.rs resolves &provider/model words to {word,provider,model,state ok|unknown|unavailable} and builds the agent's references lines; it never changes the thread's model. OPEN: agents adds refs to the user-message event and the agent text; slim policy keeps models.all, builds the & popup rows and passes entries[].refs; desktop draws the chips.")
R(A, "Provider accounts, fallback chain, allowance", m=["accounts", "accounts.default", "accounts.select"], hub="partial", c="ppp", lane=L["sett"], ev="accounts.default gated like a mobile client; accounts_io/accounts_hub (list/add/status/login/respond/cancel/rename/disconnect/remove, stand-in CLI tests; AccountsRpc::grant has NO production caller yet, so `accounts` answers the named-account message to every client and accounts.live reaches none; GPUI's renderer is mobile so it cannot get accounts either), fallback planner (Hold.clear/inherit, Preference.*, fallback_no_recovery_while_locked); settings::driver's on_line/spawn_act have NO caller yet (the limit fallback does not run until the agents lane wires the two hooks); only ResumeTicker is live, ported from ProviderResume.ready/.one on settings-reads; accounts.select and launch_plan are not yet called by agents; Reset.fill and the provider-switch transcript handover not ported")
R(A, "Launched threads report back (thread_launch)", t=["thread_launch"], c="ppp", lane=L["mcp"], ev="thread_launch is in Rust (agents/launch.rs, chain bound, report toggle) and Launch.tell is ported (turn end, ask, exit, stop incl. archive, mute, work-out-of-hand, said, own stop, Lr.loops); not: a launcher on another machine (Lr.far), the uberbot destination (Lr.dest), the worktree (Hub.new.env auto)")
R(A, "Delegated tasks (delegate_task, task_status/cancel)", m=["task.cancel", "work.cancel", "wake.cancel"], t=["delegate_task", "task_status", "task_cancel"], c="ppp", lane=L["mcp"], ev="no handler or tool in Rust")
R("Threads and timeline", "Thread and project summaries (summary.<id>, summary.at.<id>)", hub="partial", c="ppp", lane=L["state"], ev="state/host.rs asks agents::summarizer() after a batch whose end stamped summary.at.<thread> (input from state/summary.rs over the lean state), keeps the cleaned answer as summary.<id> and asks for the project's next when due (Policy::summary_done); unit-tested with the pure parts, not yet run against a real model")
R("Threads and timeline", "Wt.name: a worktree thread's first message names its branch and folder", hub="missing", c="ppp", lane=L["agents"], ev="not done in turn.start or Agents::start_queued")
R("Threads and timeline", "Keyring.say: Turn.close adds a note when the keyring is locked", hub="missing", c="ppp", lane=L["agents"], ev="not ported")
R("Threads and timeline", "Standing rules, references and bot tasks around the turn text (Rules.agent, Refs.agent, <tasks>)", hub="missing", c="ppp", lane=L["agents"], ev="the agent text carries the open review comments and the <terminals> note only")
R("Threads and timeline", "omp prompt vault-masked (Vx.mask) and the agents' stderr tail beyond Exit.stderr", hub="partial", c="ppp", lane=L["agents"], ev="settings mask covers args, stdin, stdout and tool answers; the omp prompt is not masked separately")
R(A, "Restart continues running turns (restart.continue)", hub="done", pf="agents", c="n/a".replace("n/a", "nnn"), lane=L["agents"], ev="agents carry_will, hub recover; tests.rs")
R(A, "Stand-in agents / BACKPLANE_AGENTS isolation", hub="done", pf="agents", c="nnn", lane=L["agents"], ev="Cli::standin, standin-blocks; slim-stand-in rule")

# ---- MCP tools
A = "MCP tools"
R(A, "thread_list, thread_read, thread_send, thread_update, thread_wait, thread_interrupt", t=["thread_list", "thread_read", "thread_send", "thread_update", "thread_wait", "thread_interrupt"], pf="agents", c="nnn", lane=L["agents"], ev="agents/tools.rs + mcp.rs; the_tool_list_is_the_js_hubs_for_every_tool_we_serve")
R(A, "orchestrator_capabilities, machine_list", t=["orchestrator_capabilities", "machine_list"], pf="agents", c="nnn", lane=L["mcp"], ev="orchestrator_capabilities ported; machine_list not")
R(A, "kicad_ipc, kicad_ipc_status", t=["kicad_ipc", "kicad_ipc_status"], pf="kipc", c="nnn", lane=L["files"], ev="files lane: kicad/ipc.rs + kserve.rs, results/files/kipc-hub-e2e.txt")
R(A, "viewer_show", t=["viewer_show"], c="ppp", lane=L["agents"], ev="mcp.rs pushes a {t:view} frame to all clients; no test; clients' reaction unproven")
R(A, "work_run / work_watch / work_list / work_cancel / work_unwatch", t=["work_run", "work_watch", "work_list", "work_cancel", "work_unwatch"], c="nnn", lane=L["mcp"], ev="no tool in Rust")
R(A, "wake_on / wake_list / wake_cancel", t=["wake_on", "wake_list", "wake_cancel"], c="nnn", lane=L["mcp"], ev="no tool in Rust")
R(A, "lock_acquire / lock_release / lock_list / lock_cancel", t=["lock_acquire", "lock_release", "lock_list", "lock_cancel"], c="nnn", lane=L["mcp"], ev="no tool in Rust (locks.bend projection not ported)")
R(A, "deploy_main / deploy_restart / deploy_status / deploy_cancel", t=["deploy_main", "deploy_restart", "deploy_status", "deploy_cancel"], c="nnn", lane=L["mcp"], ev="no tool in Rust")
R(A, "terminal_open / read / send / close", t=["terminal_open", "terminal_read", "terminal_send", "terminal_close"], pf="term", c="nnn", lane=L["term"], ev="term/tools.rs; the agents MCP server must route NAMES to term::tools::call (seam)")
R(A, "preview_* browser tools (open, navigate, snapshot, click, type, press, scroll, evaluate, wait_for, resize, status)", t=["preview_open", "preview_navigate", "preview_snapshot", "preview_click", "preview_type", "preview_press", "preview_scroll", "preview_evaluate", "preview_wait_for", "preview_resize", "preview_status"], pf="term", c="nnn", lane=L["term"], ev="term/browser.rs: helper driver (stand-in helper tests, no Chrome run); agents lane must route NAMES to term::tools::call")
R(A, "review_comment, review_comments", t=["review_comment", "review_comments"], c="nnn", lane=L["mcp"], pf="agents", ev="agents/review.rs; the open comments also follow every message to the agent (Review.agent)")
R(A, "secret_request / secret_list / secret_run", t=["secret_request", "secret_list", "secret_run"], c="nnn", lane=L["mcp"], ev="no vault in Rust")
R(A, "settings_inspect, settings_propose", t=["settings_inspect", "settings_propose"], c="nnn", lane=L["mcp"], ev="no tool in Rust")
R(A, "told_check, told_mark, task_board_*", t=["told_check", "told_mark", "task_board_add", "task_board_import", "task_board_list", "task_board_remove", "task_board_update"], c="nnn", lane=L["bots"], ev="bots/ledger.rs: all seven, listed in every thread's tools (a bot's and a plain thread's), records the settings told.*/board.* read through the model; bots::tests::js_ledger_tools replays a JS-hub recording (answers and the records written), plus unit tests of keys, words, overlap, dates and checkboxes. The person's rules.set/drop and board.set/drop requests are not here (Ledger.request)")
R(A, "Bot tools: bot_*, room_*, routine_*, space_*, memory_*, project_*, plane_read, notify_person, tell_person, remind_person, person_timezone, decide, escalate, charter_set, voice_dictionary_add, task_* (browser skills)", t=["bot_create", "bot_list", "bot_send", "bot_whoami", "room_create", "room_list", "room_post", "room_read", "routine_delete", "routine_list", "routine_set", "space_get", "space_page", "space_set", "memory_forget", "memory_list", "memory_recall", "memory_save", "project_create", "project_list", "plane_read", "notify_person", "tell_person", "remind_person", "person_timezone", "decide", "escalate", "charter_set", "voice_dictionary_add", "task_delete", "task_end", "task_get", "task_list", "task_pause", "task_run", "task_save", "task_step"], c="nnn", lane=L["bots"], ev="bots/tools.rs: bot_whoami, bot_list, bot_send, room_list/create/post/read, memory_*, space_set/get/page, routine_list/set/delete, project_list, project_create, bot_create, plane_read, notify_person, remind_person, person_timezone (the JS hub's own tool list as fixture; bots::tests::js_bot_tools and js_ledger_tools replay the recorded calls, bot_messages_past_the_depth_bound_are_held_and_wake_nobody; what_a_tool_does_for_the_uberbot_makes_ops proves World::also for memory, space, page, routines and reminders); project_create asks the person through Inbox::approve and waits outside the world lock; decide, escalate, charter_set, tell_person are the mail lane's (bots/mail/tools.rs on gpui-uber-mail) and notify_person calls its Bots::notify_relay once that lands; not yet: voice_dictionary_add, task_* (term lane); registered with the agents' MCP server by Bots::start (Agents::add_tools; a bot's own tools listed in a bot's thread only; bots::tests::the_bots_tools_reach_a_bots_agent_through_the_tool_set); remote bots (name@machine) answer no linked machine (net lane); a thread whose worktree is not made reads ready, not working; person_timezone with an unresolved zone looks it up (zone.set, settings lane) after the answer; space_set accepts what a recorded lexer accepts, malformed object/array text refused")
R(A, "Mail and Google tools (mail*, gmail_*, calendar_*, google_accounts)", t=["mail", "mail_context", "mail_dispose", "mail_done", "mail_list", "mail_read", "mail_search", "mail_snooze", "mail_tag", "gmail_draft", "gmail_read", "gmail_search", "gmail_send", "calendar_create", "calendar_delete", "calendar_events", "calendar_update", "google_accounts"], c="nnn", lane=L["bots"], ev="none exist in Rust")

# ---- Bots
A = "Bots"
R(A, "Bots list, create, edit, delete, cats and looks", m=["bots.create", "bots.edit", "bots.delete", "bots.pin"], c="mdd", lane=L["bots"], ev="a bot's thread starts on the default account (account.thread.<th>.<provider> after its ThreadCreated, Accounts.created; bots::tests::a_bots_thread_starts_on_the_default_account); bots.create/edit/delete in bots/rpc.rs; proof: bots::tests::js_parity replays a JS-hub recording (test/gpui/bots/fixtures) event by event, plus tests for names, races, uber on create; bots.pin missing (routine pins are the uber sync); BotFx gone (close the bot's browser page) is the term lane's. Cats: GPUI logo/cats vector paths (shots/logo-cats/gpui-*), phones native Cat.swift/Cat.kt with shots; interaction missing in GPUI")
R(A, "Bot tabs (Status, Chat, Space, Routines, Memory, Hooks)", m=["bots.say", "bots.tell", "bots.forget", "bots.remember", "bots.hook", "bots.hook_revoke", "bots.hook_token"], c="pdd", lane=L["bots"], ev="bots.say/remember/forget/space done (bots/rpc.rs, js_parity incl. a retried msg, reply quotes and a dead bot); bots.tell (peers, net lane) missing; bots.hook, bots.hook_token and bots.hook_revoke done (hub tests); Tasks (Browser sub-tab: cards, run form, Run reaches the bot, Delete asks first, seen in a real window) and Webhooks (Settings sub-tab: New webhook, secret shown once, Token / New token / No token, Revoke, seen in a real window). GPUI: Chat, Mailroom, Space, Browser, Memory (paged at 64 KB: 154 memories, 1-42 / 43-84 seen in a real window with Newer and Older), Routines and Settings (persona edited and saved through bots.edit, seen in a window; the name is read-only as in Bend); android-dark-3-bot / ios-*-bot shots (JS hub)")
R(A, "Bot space (blocks, pages, links)", m=["bots.space"], c="dpp", lane=L["bots"], ev="bots.space (a button or field is heard as a person's message) in bots/rpc.rs, js_parity; the space spec tool (space_set) and /space.html missing; SpaceView.swift/.kt; GPUI: blocks and inputs drawn natively (screens/bots.rs); a typed input in a real window on a private Xvfb reached the stand-in bot as \"[space] tell: <text>\" and the box emptied")
R(A, "Rooms (create, post, mention, delete)", m=["bots.room", "bots.room_delete", "bots.post"], c="mdd", lane=L["bots"], ev="bots.room/room_delete/post in bots/rpc.rs; proof: js_parity (a room with a deleted member, a retried msg, a deleted room). GAPS: members elsewhere (name@peer) get no send (net lane, Bots.remotes); with mail.on the post is logged but Mail.room files nothing and wakes nobody (mail lane, B.Mail.room); the uberbot (Triage.owner) is skipped as Bots.wake.room does; hops past Hop.max are held (bot_held_wakes_none) but bot-sent posts need the bot tools. ios/android room shots (JS hub). A #room named in a thread message posts there as the person, with where it was said (Bots.mention.room; bots::tests triage::a_room_a_thread_message_names_gets_the_message_with_where_it_was_said)")
R(A, "Status plane (need, run, bots, rooms, routines, machines)", t=["plane_read"], hub="partial", c="mpp", lane=L["bots"], ev="plane_read in bots/plane.rs: the text over the state lane's fold and the bots' world (need, run, bots, rooms, locks, routines, idle), replayed against a JS-hub recording in bots::tests::js_ledger_tools (running, question, approval, failed, owned work, a wake, a lock, a routine, a room, the uberbot left out). Not yet: other machines (plane_read with machine answers no such machine; their threads and the Machines line wait for the net lane), and Plane.* for the clients' Status tab is presenter code")
R(A, "Uberbot, promotion, sync across machines", m=["bots.uber", "bots.uber_join", "bots.uber_home", "uber.sync"], hub="partial", pf="state", c="mpp", lane=L["state"], ev="state/uber.rs folds UberOp (2 tests); bots/uber.rs (bots lane) makes the ops: Ub.also beside every uberbot change (memory, space, page, name and look, routines), Uber.init's first ops at the minute tick, bots.pin, bots.uber_home (the lease moves one term on), bots.uber_join, bots.uber promotion with the backup; proof: bots::tests::js_parity_uber replays a JS-hub recording (promotion, first ops, memory, routines with plans, pin and unpin, delete, edit, refusals; machine ids, stamps and instants normalised). Order of the first ops follows Bend: memories newest first, routines by id (checked against a second JS recording, js_parity_uber_first_ops_...; space, page, join and a lease moving from another machine are unit tests with the Bend text). KNOWN BEND BUG, kept: Ub.promoted has no callers, so the first ops wait for the minute tick; a memory saved inside that minute becomes op 1, vv is then not 0, and Uber.init never logs who, lease and bind (decided: keep parity with today's Bend, no src/**.bend change; fix it in Rust when the cross-machine exchange is ported, by running init whenever this machine has no bind op rather than when vv is 0). WHEN THE AGENT TOOLS LAND (memory_*, space_*, routine_*, as Bots.ub wraps MCP tool steps at bothub.bend:2781) every commit they make must go through World::also, or an uberbot's own changes make no ops. GAPS: the exchange between machines (uber.sync over /far/rpc, the holder's beat, takeover, the old holder's handoff queue, the replica derive/commit of taken ops) needs the net lane's transport and is not ported; so Ub.chat is always this machine's thread; the uberbot's routines still do not fire (see Routines)")
R(A, "Routines (set, run, delete, schedule, time zones)", m=["bots.routine", "bots.routine_delete", "bots.routine_run", "zone.report", "zone.set"], c="mpp", lane=L["bots"], ev="a hub on a copy of a log (BACKPLANE_COPY=1) runs none of the lane's unprompted work: no ticker, no routine or reminder firing, no uberbot first ops, no mail recovery, no owner outbox (bots::tests::unprompted_work_runs_live_and_not_on_a_copy_hub runs both modes in child processes); bots.routine/routine_delete/routine_run, the minute tick and the between-ticks wake in bots/routines.rs; clock.rs ports Cron/Tz/When/Sched with the Bend corpora; proof: bots::tests::js_parity_routines replays a JS-hub recording (plans and their errors, run now, a one-time routine, a toggle, a deleted one), routines_tick_fires_what_is_due_once_and_a_reminder_alerts. FINAL-GATE BLOCKER: the uberbot's routines (the person's main bot, Bub) are now filed as mail with an owner ticket (and gated by Ub.fires, with a fired op), as is every firing while mail.on is on, but nothing routes the filed mail to the uberbot's triage yet (mail slice 3), so Bub still gets no agent turn from a routine; and every other inbound to the uberbot (thread_send, hook wakes, launch reports, task notes) is still delivered to its thread instead of filed; the first test after the mail/uber code lands must show all overdue routines fire once, not a backlog per missed minute. Other GAPS: zone.report/zone.set (the hub resolving zone rules from zoneinfo, settings lane), routines of the uberbot and any firing while mail.on is on are not fired (no Mail.funnel / Ub.fires yet), the routine_set/list tools; hub seeds tz.machine at start")
R(A, "Reminders and notifications to the person", m=["bots.answered"], c="dpp", lane=L["bots"], ev="a due reminder (routine kind remind) logs Reminded + the alert record alert.<thread> (Policy.reminder) with no agent turn: routines_tick_fires_what_is_due_once_and_a_reminder_alerts; the alert record now raises the webhook and the desk notification (settings/alerts.rs; a phone push is the APNs lane's); remind_person and notify_person are in bots/tools.rs (js_ledger_tools: the alert record, once a turn, done once per message, reminder and digest only in a routine's turn, the setting off) but write only that record, so they share the gap; bots.answered (a linked machine's reply) is not ported")
R(A, "Webhooks (bot hooks, /hook/)", r=["/hook/"], pf="bots::tests a_webhook_is_made_signed_calls_reach_the_bot_once", c="dpp", lane=L["net"], ev="bots/hooks.rs: POST /hook/<id> with our signature (fresh, replay once), GitHub's X-Hub-Signature-256 (delivery id as the key) or a bearer token; the bot hears the payload as data and the hook counts; a real signed curl at a hub got 200, the replay 401, an unsigned call 401. NOT ported: the Pebble Index ring's form and its recording; the token URL is the hub's listen address unless BACKPLANE_HOOK_URL names another")
R(A, "Browser teach and task skills", m=["bots.browser", "bots.teach", "bots.task"], r=["/tasks", "/runs"], pf="term", c="ppp", lane=L["term"], ev="term/{task,teach,driver}.rs: rules, requests, task_* tools, runs, pause ask; the Tasks tab lists, runs (the driver's wake now reaches the bot) and deletes tasks in GPUI; seams: page owner, agents lane ask answer (Driver::answered), /tasks and /runs HTTP routes not served")
R(A, "Peers: invite, join, unpeer, own", m=["bots.invite", "bots.join", "bots.unpeer", "bots.peer_own"], r=["/bots/pair", "/bots/dir", "/bots/link", "/bots/deliver"], c="mpp", lane=L["net"], ev="net/peering.rs, sign.rs, botnet.rs, machines.rs, peers.rs, tailnet.rs: /bots/pair (credential, owner trust, proof+callback), /bots/pair/check, /bots/link, signed intake, ask/join, two-hub tests; not wired (needs Req body/trusted seam); /bots/deliver and /bots/dir, bots.invite/join/unpeer/peer_own are net/peerrpc.rs (two-hub test). Gaps to carry with the wiring: nothing calls Peering::set_own_hosts until the tailnet listener exists, so Auth.owner.pair is dead (pairing on the tailnet needs a proof and links as unverified; refuses more, never less); signed_in lacks Hub.bot.vouched (a request Auth.owner.peer vouches for records peer.own.<id>=tailnet) and Bot.dir.awake; the Req body must be capped at 256 KB while reading (Content-Length, before the body is read). Gap: the machine finder probes http only, so an https BACKPLANE_PEERS hub is dropped (Bend's curl reaches it)")
R(A, "Google sign-in for bots (mail, calendar)", m=["bots.google"], r=["/oauth/google"], c="mpp", lane=L["bots"], ev="no Rust handler or route")
R(A, "Mail (mail.list/read/find/mark/tag/send, owner.mail)", m=["mail.list", "mail.read", "mail.find", "mail.mark", "mail.tag", "mail.send", "owner.mail"], c="mpp", lane=L["bots"], ev="SLICE 2 (requests) in bots/mail/requests.rs: mail.list/find/mark/tag/read/send (bots::tests::mail_requests_list_read_find_mark_tag_send; mail.read of a room conversation pages its headers with texts from the room's RoomPosted lines or the journal (bots::tests::mail_read_of_a_room_conversation_pages_its_posts); owner.mail in bots/mail/owner.rs (tickets, outbox worker with plan/send/sent, the holder's import with receipts and the peer/accept/seen checks; bots::tests::owner_mail_moves_waiting_mail_to_the_holder_and_is_acknowledged_once runs two hubs): the walk does nothing on a hub started on a copy of a log, runs one walker at a time (a flag with a drop guard; a request is cut off after 30 s), runs every 20 s with or without a link (so recovery and the holder's own routing do not wait for a transport), and decides and commits each ticket change under the world lock; the transport is the net lane's, installed with Bots::set_owner_link, and it must enforce Far.own.id (send only to the owner's own machines), and the /far/rpc route must call Bots::owner_received(link, request) (neither exists in net yet, so nothing sends or receives between real machines); Owner.legacy/waiting/recovered (tickets for queues older than tickets and for recovered mail), Owner.claim (nothing clears a holder's ticket yet: no run claims it; the claim must take the world lock, and with a Router installed every walk routes each pending ticket again, so routed must be idempotent and claim must land with or before it) and the mail lane's OwnerMail arm calling owner_kick are not wired; mail.send to a non-bot thread posts the person's message through the inbox seam). SLICE 1 (filing) in bots/mail/: the journal <home>/mail/<conv>.jsonl (append with torn-tail repair, mode 0600), ConvSet/MailPosted, the box fold (conversations, headers, marks), Mail.route/classify/want/head, the funnel decision applied to routine firings and room posts only (the uberbot and mail.on file instead of delivering), the owner ticket (owner.mail.id/local/pending), recovery of journaled headers at start; proof: bots::tests::js_parity_mail_filing replays a JS-hub recording and compares events and journal lines (37 events), plus unit tests for route/classify/want/body/recover. NOT YET: every other inbound is still delivered as before, including to the uberbot, which Bend files and never delivers (Mail.inbound / Bots.inbox: a thread's message to a bot, hook wakes, launch reports, task notes, bots.tell, mentions: they live in the agents, net and tools lanes and call the funnel when they are ported; the decision is `Mail.funnel` in hub.bend: uberbot or mail.on => file only, mail.on to a thread => both); a journal failure is logged by kind and counted, and the changes before it stay committed as in Bend, but for mail to the uberbot Bend also tells triage (Mail.failure: slice 3, `Router::failed` is its seam); the requests mail.list/read/find/mark/tag/send and owner.mail (slice 2), the .idx hash index Bend keeps beside each journal (the JS hub rebuilds a stale one; the Rust reads will need it), routing after filing: triage runs, private coordinator threads, escalations, claims (slice 3, so the uberbot gets no agent turn from a filed mail yet), Google/Gmail/calendar and their tools (slice 4); mail_send/mail_* agent tools; the sender of a room post is only the person here (a bot's post comes with the bot tools)")

# ---- Project home, ordering
A = "Project home and ordering"
R(A, "Project home (goal, intent, parts, boards)", m=["project.home", "project.start", "fab.look", "fab.intent"], hub="partial", c="pdd", lane=L["orders"], ev="project.home (snapshot, document, intent write; the price field of the price op is not ported; fab/config metadata over 8 MiB and saved check reports over 16 MiB count as unreadable, where the JS effect streams them) and project.start in commands/, fab.look / fab.intent in settings/fab.rs; GPUI shot gpui-light-4-project-home.png (cards, generic nodes)")
R(A, "Ready to order bar, Buy it, price estimate", m=["fab.look", "order.buy"], c="mpp", lane=L["orders"], ev="order.buy no handler; order server client (orderio) not ported")
R(A, "Order progress, quote, approvals, notes", m=["order.buy"], c="mpp", lane=L["orders"], ev="no Od.* watcher in Rust")
R(A, "Manufacturing review (order_review) and .fab.json", t=["order_review"] if False else [], hub="missing", c="mpp", lane=L["orders"], ev="order_review tool absent from the Bend Tool.def list and from Rust; Order.prompt.review is Bend prompt text")
R(A, "Board picker (several boards)", m=["boards.list"], pf="files", c="mpp", lane=L["files"], ev="files/misc.rs boards.list (kicad/boards.rs 2 tests); picker UI unproven in clients")

# ---- Viewers
A = "Viewers"
R(A, "MCP endpoint for agents (/mcp/<thread>.<gen>?k=)", r=["/mcp/"], pf="agents", c="nnn", lane=L["agents"], ev="agents/mcp.rs listener with HMAC key per thread and generation; unit tests hmac, url key")
R(A, "Board viewer (layers, high contrast, pan/zoom, pick)", m=["kicad.watch", "kicad.info"], pf="plot", c="mdd", lane=L["gview"], ev="Rust plots equal the JS hub byte for byte (plot-2.json); GPUI has no canvas (README gap); phones PlotView (ios vboard.png)")
R(A, "Schematic viewer, sheets, hierarchy navigation", m=["kicad.watch", "kicad.info"], hub="partial", pf="plot", c="mdd", lane=L["files"], ev="plot-2.json equal; '--plot-tree' sheets not ported (files README); ios vsch.png")
R(A, "Split view and cross-probing", m=["kicad.watch", "kicad.info"], hub="partial", c="mpp", lane=L["files"], ev="README: not ported yet: web stamps, PWarm, split views; phones PlotView, no shot")
R(A, "3D board viewer", m=["kicad.watch"], r=["/viewer-asset"], hub="partial", pf="solid", c="ddd", lane=L["gview"], ev="solid/mesh e2e files; /viewer-asset route missing in Rust; ios view.png | gpui: viewers/solid.rs mounted from screens/viewer.rs (open 3d): the Rust hub's own solid frame (key 2|<root>|, 197 KB) draws as a lit, orbitable board in the capped desktop run (build/gpui/shots/desktop/mount3-3d.png, llvmpipe software rendering, one run); the page shows only when its key is the viewer's")
R(A, "Mech tab: STEP/GLB parts, renders, FreeCAD", m=["mech.list", "mech.render"], pf="files", c="ppp", lane=L["gview"], ev="files/misc.rs; step2glb child; no client shot | gpui: mounted (part chips mech-part, Refresh over the 3D page); UNPROVEN: in the capped runs (mount3..6) the client sends mech.list once, the hub answers, hub.rs asks for a view.update at once, and the page still says 'Loading the parts...' with no part frame; so the hub's fold of the job answer into viewer.mech (session::record_reply / replied_late, applied on the next view request) is unverified: a hub-side test is needed; no Renders page (button not offered)")
R(A, "Viewer layers: hidden, dimmed, focus (high contrast), All/Dim/Only", hub="n/a", c="dpp", lane=L["gthread"], ev="viewers/layers.rs: the hub's off/dim/lift masks drive what the canvas draws (hidden left out, dimmed to a quarter, lifted last; unit tests) and the Layers menu sends view-layer / view-layer-pick / view-layer-mode; not shown on a real board hub frame yet; the schematic page has the same menu")
R(A, "Schematic sheet navigation (tree, trail, tap a sheet symbol, back/forward)", hub="n/a", c="mpp", lane=L["gthread"], ev="actions view-sheet / sch-go / sch-up / sch-back / sch-fwd and viewer.sheets: not in GPUI; to do")
R(A, "Tap to inspect a part or net (pick card, cross-probe)", hub="n/a", c="mpp", lane=L["gthread"], ev="the plot chunks' info tables and the pick card: not in GPUI; to do")
R(A, "Design history bar (steps, playback, compare versions)", hub="n/a", c="mpp", lane=L["gthread"], ev="history.* requests and the viewer's hist bar: not in GPUI; to do")
R(A, "Review comments pinned to a board or schematic place", hub="n/a", c="mpp", lane=L["gthread"], ev="review.comment / review_comments and the Comments page pins: not in GPUI (a Review page shows text only); to do")
R(A, "Browser page input (bots.browser clicks/keys, take over, teach)", m=["bots.browser", "bots.teach"], c="mpp", lane=L["gthread"], ev="GPUI: the Browser pages show the JPEG frames only: no clicks, keys, handback or teach; to do")
R(A, "Files tab (list, open, images, PDF, notes)", m=["files.dir", "files.list", "pdf.page"], r=["/img", "/files/get", "/files/zip", "/view/get", "/view/ver", "/view/git"], pf="files", c="mpp", lane=L["gview"], ev="parity-2.json: 175/178 identical (3 deliberate); client FilesView.swift/.kt exist, no shot")
R(A, "Diff viewer (git.diff)", m=["git.diff"], pf="git", c="mpp", lane=L["gview"], ev="git-1.json; clients unproven")
R(A, "Design history bar, playback, steps", m=["history.refs"], hub="partial", pf="git", c="mpp", lane=L["files"], ev="Git.step.capture checkpoints not in the Rust list (no step job); history.refs handled")
R(A, "Compare versions, overlay, Old/Diff/New, per layer", m=["history.resolve", "history.refs", "kicad.warm"], hub="partial", pf="review", c="mpp", lane=L["files"], ev="history.resolve/refs/warm registered; hist/cmp UI unproven")
R(A, "Review tab: BOM, Checks, Comments, Release", m=["review.bom", "review.checks", "review.tick", "review.comment", "review.release"], pf="review", c="mpp", lane=L["gview"], ev="Rust: 5 requests with tests; ReviewView.swift/.kt exist; GPUI none")
R(A, "Review comments pinned on board/schematic", m=["review.comment"], t=["review_comment", "review_comments"], c="mpp", lane=L["mcp"], ev="request done, agent tools missing")
R(A, "Browser tab (agent's page), screen frames", r=["/screen.jpg"], pf="term", c="ppp", lane=L["term"], ev="term/driver.rs serves GET /bots/<id>/screen.jpg (newest JPEG frame, no-store) and the shot events; DECISION 2026-11-05: GPUI draws these frames with the viewers (option A, docs/gpui/BROWSER-EMBED.md), lane gpui-screens | gpui: viewers/browser.rs mounted for open browser (rail Browser = view browser, owner = the bot id on a bot screen, else the thread); NOT DRIVEN: no real frame fetched on a hub yet; a bot's own Browser tab draws the same frames, read-only (screens/bots.rs, gpui-bub))")
R(A, "Hub over https/wss (tailscale serve --https): the client link and the Browser page fetch", r=["/ws", "/screen.jpg"], hub="missing", c="mpp", lane=L["gview"], ev="native/gpui/src/hub.rs: tokio-tungstenite has no TLS feature and Config::parse accepts only http:// and ws:// hubs; viewers/browser.rs uses ureq without TLS and says so in the page. GPUI cannot reach a hub that is served over --tailscale-https at all (link or frames). To do: rustls (webpki roots) for tokio-tungstenite and ureq, wss:// and https:// accepted, tested against a TLS fixture; measured nothing yet")
R(A, "Viewer layout (split pane, folding, shortcuts)", hub="n/a", c="mpp", lane=L["gview"], ev="client-only; GPUI has no viewer pane")
R(A, "KiCad over IPC (headless server per project)", m=["kicad.retry", "kicad.install", "kicad.open", "cad.install", "kicad.export10"], t=["kicad_ipc", "kicad_ipc_status"], hub="partial", pf="kipc", c="nnn", lane=L["files"], ev="kicad.retry, kicad.export10 done; kicad.install/open/cad.install not")
R(A, "KiCad components list, render asset", m=["kicad.components", "kicad.renderAsset"], hub="partial", pf="files", c="mpp", lane=L["files"], ev="kicad.components registered; kicad.renderAsset has no handler")
R(A, "KiStack skills in every thread, $ list", hub="partial", c="ppp", lane=L["agents"], ev="deploy/kistack shipped; Rust skills.list missing; prompt gating in agents/prompt.rs")

# ---- Git
A = "Git and PR"
R(A, "Git status, commit, push, PR (git.act)", m=["git.status", "git.act", "git.diff", "git.revert"], pf="git", c="mpp", lane=L["files"], ev="git-1.json; copy guard refuses writes on copies (copy.rs 15 tests); client buttons unproven")
R(A, "Worktrees per thread, checkpoints", m=["thread.create"], c="ppp", lane=L["cmd"], ev="commands/worktree.rs (Git.thread_wt: upstream, subdirectory check, carry of uncommitted KiCad work; copy mode skips at once); real-git test on Boat; checkpoints are files/git.rs")
R(A, "Worktree job reports (worktree, note, local, branch)", m=["thread.worktree", "thread.note", "thread.local", "thread.branch"], c="ppp", lane=L["cmd"], ev="thread.worktree / note / local are the server job's reports: commands/worktree.rs commits WorktreeSet / WorktreeSkipped and the note itself (Hub.job.ready, Wt.start through the agents seam; tests a_made_worktree_is_recorded_and_the_waiting_messages_start, on_a_copy_the_worktree_job_skips_at_once_and_writes_no_git); thread.branch (the text model names the branch) is not ported: it needs OneShot")
R(A, "Job reports that land on a thread (titled, merged, unrooted)", m=["thread.titled", "thread.merged", "thread.unrooted"], c="ppp", lane=L["cmd"], ev="NOT registered, deliberately: these are the server jobs' reports (client 0 in Bend), and no job that sends them is ported (the title writer, the gh merge watch, the worktree remover), so a client must not be able to call them. The planners exist in commands/plan.rs with a test (the_job_reports_titled_unrooted_and_merged_have_planners_and_no_client_method) for when the jobs land; thread.merged would not write Policy.merged's alert record either (the notification records are not ported)")

# ---- Settings
A = "Settings"
R(A, "Setting storage and echo (setting.set)", m=["setting.set"], c="mpp", lane=L["sett"], ev="settings/set.rs handles setting.set (peer.own, account gate, telemetry keys); unit-tested on Boat (49 settings tests); GPUI settings glyph still inert")
R(A, "Appearance (theme, text size, look)", m=["setting.set"], c="mdd", lane=L["gscreens"], ev="GPUI theme tokens fixed light/dark (theme.rs test); not settable")
R(A, "Threads page (settle, notify, snooze defaults, scratch days)", m=["setting.set"], c="mpp", lane=L["sett"], ev="setting.set (settings/set.rs) writes and echoes every key; the page is GPUI/screens work")
R(A, "Agents page (providers, models, CLI install, API keys)", m=["setting.set", "models.refresh", "update.check"], c="mpp", lane=L["sett"], ev="setting.set, models.refresh (re-sends the built-in models info; CLI listings not ported), update.check (managed: empty version) answer; info models is settings/models.rs, byte-equal to the JS hub's for a machine with no CLI")
R(A, "Writing page (text model)", m=["setting.set"], c="mpp", lane=L["sett"], ev="setting.set (settings/set.rs)")
R(A, "CAD page (KiCad install, version, fork)", m=["kicad.install", "cad.install", "kicad.retry"], hub="partial", c="mpp", lane=L["files"], ev="info kicad.* seeded as empty; install not ported")
R(A, "Ordering page and price estimates", m=["setting.set"], c="mpp", lane=L["orders"], ev="no handler")
R(A, "Devices page, paired devices, revoke", m=["device.register", "device.drop"], r=["/auth/devices", "/auth/revoke", "/auth/known", "/auth/cache"], hub="partial", c="mpp", lane=L["net"], ev="LIVE: registry decides every credential, /auth/invitations|devices|revoke|known|enroll (net/, 40 Boat tests); device.register/drop (push) not yet")
R(A, "Network page (Tailscale, listen, pairing link)", r=["/auth/pair", "/auth/enroll", "/auth/invitations"], hub="missing", c="mpp", lane=L["net"], ev="info 'tailnet' seeded as off; no tailscale")
R(A, "Voice page (engine, model, device, dictionary)", m=["bots.voice"], r=["/voice/in"], c="mpp", lane=L["voice"], ev="no Rust handler/route")
R(A, "API keys (add/drop, api.env)", m=["bots.voice"], c="mpp", lane=L["sett"], ev="settings/keys.rs + keys_rpc: api-add/api-del/status answer names only; bots.voice other ops (voice) are the term lane's")
R(A, "Antigravity account and sign-in", m=["bots.voice"], c="mpp", lane=L["msp"], ev="UNPROVEN: pure scripts and state machines in providers/agy.rs (install, probe, sign-in, callback check) unit-tested; no job host runs them, no bots.voice agy handler, no real sign-in")
R(A, "Time zone page", m=["zone.report", "zone.set"], c="mpp", lane=L["sett"], ev="zone.report / zone.set (settings/zone.rs): JS-parity scenario 'settings' identical; POSIX rule evaluation not ported (routines, bots lane)")
R(A, "Usage page and allowance windows", hub="partial", c="mpp", lane=L["sett"], ev="allowance.codex seeded; no updater")
R(A, "About, version, update check, update now", m=["update.check", "update.now", "app.update"], r=["/hello"], hub="partial", c="mpp", lane=L["sett"], ev="/hello and version in hub.rs; no updater (BACKPLANE_NO_UPDATE)")
R(A, "Privacy page and telemetry opt-out / reset", m=["telemetry.choice"], hub="missing", c="mpp", lane=L["sett"], ev="settings/telemetry.rs: choice, receipts, legacy off-only path ported; sender deliberately not ported (nothing is sent, no endpoint baked)")
R(A, "Settings propose / approve (agent changes)", t=["settings_propose", "settings_inspect"], m=["setting.set"], c="mpp", lane=L["sett"], ev="settings_inspect/settings_propose tools, the ask answer hook and write invalidation (settings/proposal.rs, fallback.rs); Private.live check waits for bot private turns")
R(A, "Settings stick across restarts", m=["setting.set"], pf="state", c="ppp", lane=L["sett"], ev="setting.set logs a SettingSet (no event when the value is unchanged, as the JS hub); survives a restart (test the_tours_skip_is_answered_logged_and_survives_a_restart)")

# ---- Accounts, pairing, machines
A = "Accounts, pairing, machines"
R(A, "Pairing token and bearer / cookie auth", r=["/hello", "/ws", "/client/capabilities"], hub="partial", pf="side", c="ddd", lane=L["core"], ev="auth.rs laws; badauth scenario identical (parity-rust6)")
R(A, "Pairing link, QR code, phone scan", r=["/auth/pair"], hub="partial", c="npp", lane=L["net"], ev="/auth/pair, /auth/cache and the QR code of /auth/invitations (net/routes.rs, net/qr.rs); --tailscale-https (net::listener::https: tailscale serve --bg --https=443, the MagicDNS https address first in an invitation's routes and the pairing link, info tailnet https; a failed serve claims nothing; run on a real tailnet 2026-11-05: a test hub with --tailscale-https and BACKPLANE_HTTPS_PORT=3917 (so port 443 stays another hub's) gave https://desktop.tail9e0cf2.ts.net:3917 first in an invitation's routes, and h@backplane enrolled with its code over that address, used its device token, and could not reuse the code); once the tailnet listener is up its address is first in an invitation's routes and the link names it (test tailnet_the_pairing_link_and_qr_name_the_listeners_address_not_the_hosts). NOT wired: the MagicDNS http name; the info keys pair and pair.qr are always empty, as the Bend hub sends them (Hub.pair_msg.of: shared root credentials never leave the hub); phones: Pair.swift/Pair.kt scan; android 01-after-pair.png on JS hub")
R(A, "Wrong pairing, reconnect banners", hub="partial", c="ddd", lane=L["pedge"], ev="badauth identical; android 08a view-failed banner shot")
R(A, "Your machines, sync, far threads (<link>~<id>)", m=["view.action"], r=["/far/push", "/far/log", "/far/rpc"], hub="partial", c="mpp", lane=L["net"], ev="net/farstore.rs, farsync.rs, feed.rs, outbox.rs: mirrors outside the log (Bend file format), /far/push in, /far/log served and pulled, pushes with retry/backoff, FarView pages; tested Rust-to-Rust and against a real JS hub on Boat (pull with pages near 400000 chars, push, mirror file read). Gaps: mcp.call (remote agents' tools, Fm.serve), uber.sync and the viewers' reads over /far/rpc answer 501 (net/rpc.rs: the rest of /far/rpc is ported: a client's request naming <link>~<id> or `on` goes to that owner machine signed and only for Fr.methods and the owner's own machines with sync on, the owner takes it only for what it shares (Fr.allowed) and runs it as a local request, the reply returns with the caller's id and ours thread ids; owner.mail goes to the bots lane's owner_received and its OwnerLink is installed here; tests far_route_*, far_allowed_only_methods, far_no_delete over two hubs; not run against a JS peer); streamed text (live) and desk notes (note) are answered ok and dropped; a new epoch on the owner rebuilds the WHOLE presenter (Hub::forget_mirror: restart, fold the log again, re-feed every mirror), where Bend forgets just that link (Fr.forget): every client of this hub waits for the refold (views answer PRESENTER_UNAVAILABLE meanwhile): 6.2 s at 202k log events (h@backplane, release, 1 sample), so about a minute at ten times that, per peer epoch change; the way out is a per-link forget verb in the presenter (Bend side); Far.taken.cat (a pushed project change pulling the catalog); v3 snapshot (a first pull sends the whole shared history); a 1M-line mirror is rebuilt at every start (13.9 s release, +46 MB, 32-row page 0.07 ms; off the async workers); https hubs are not probed")
R(A, "Agents across machines (mcp.call, thread_send to a machine)", m=["mcp.call"], c="nnn", lane=L["net"], ev="no fmcp port")
R(A, "Viewers for threads elsewhere (far.view)", hub="missing", c="mpp", lane=L["net"], ev="no fview port")
R(A, "Window on another hub (remote view cache)", r=["/view/get", "/view/ver", "/view/git"], pf="files", c="pnn", lane=L["files"], ev="routes done in Rust; GPUI joins a hub by URL only")
R(A, "Multi-hub phone list", hub="n/a", c="npp", lane=L["ios"], ev="client-side Hubs.* in bridge.js; untested against Rust hub")

# ---- Diagnostics, improvements, update, deploy
A = "Diagnostics, deploy, update"
R(A, "Diagnostics page and control (diagnostics.status/control)", m=["diagnostics.status", "diagnostics.control"], r=["/debug/diagnostics.json", "/debug/log"], c="mpm", lane=L["sett"], ev="settings/diagnostics.rs: bounded local recorder, status/control/export; perf feeds and the exit record are not wired (pending reads 0)")
R(A, "Improvements (report a problem, context)", m=["improvements.context", "improvements.report", "improvements.preview", "improvements.template", "improvements.prompt", "improvements.diagnostics"], r=["/debug/diagnostics.json"], c="mpp", lane=L["sett"], ev="improvements.context / improvements.report (settings/improvements.rs); improvements.preview/template/prompt (settings/issue_report.rs) are new wire methods with no Bend counterpart (the Bend clients call Issue.* directly): checked by a test that the embedded template, Unslop and rules equal the Bend literals; GET /debug/diagnostics.json (settings/diagnostics_http.rs, tested through the real dispatch); the thread itself is thread.create with a creationKey (commands lane)")
R(A, "Perf endpoint, trace, log levels", r=["/debug/perf"], c="nnn", lane=L["soak"], ev="no /debug/perf in Rust; soak harness measures from outside")
R(A, "Safe deploy and restart", m=["deploy.restart", "deploy.cancel", "deploy.status"], c="mpp", lane=L["mcp"], ev="no handler; deploy.bend not ported")
R(A, "Named locks (Locks panel, release/cancel)", m=["lock.release", "lock.cancel"], r=["/lock/"], c="mpp", lane=L["mcp"], ev="no handler")
R(A, "Work, wake, task watchers (Status rows)", m=["work.cancel", "wake.cancel", "task.cancel"], c="mpp", lane=L["mcp"], ev="no handler")
R(A, "Device registration and push (APNs, FCM, hooks)", m=["device.register", "device.drop"], hub="missing", c="npp", lane=L["net"], ev="no push.bend port")
R(A, "Notifications: desk look, desktop notify, islands", m=["desk.look"], c="dpp", lane=L["net"], ev="settings/alerts.rs: desk.look keeps each connection's look; an alert.<thread> record with a kind goes to the desk used last as a notify frame, held for ask/done/fail/merged on a thread a focused desk shows, never for a note (hub unit tests, uber-e2e: held while watched, delivered otherwise); GPUI screens/desk.rs sends desk.look and runs notify-send or BACKPLANE_GPUI_NOTIFY_CMD (a real window on a private Xvfb ran the notifier for a note and a done). Missing: the phone push (APNs, Apple credentials) and the Live Activity island, delivery to the owner's other machines")
R(A, "Webhook alerts (push.webhook.url)", hub="done", pf="settings/webhook.rs and settings/alerts.rs tests", c="nnn", lane=L["sett"], ev="settings/webhook.rs: body, HMAC signature, mode 0/1/2, curl delivery to a local listener tested; settings/alerts.rs calls webhook::deliver for each alert record (alerts tests)")
R(A, "Update (self-update, managed installs)", m=["update.now", "update.check"], c="nnn", lane=L["sett"], ev="a packaged hub never replaces itself: update.check answers an empty version, app.update clears update.wait and tells clients (JS managed-install behavior); no updater by design")

# ---- Terminal, browser, voice, secrets
A = "Terminal, browser, voice, secrets"
R(A, "Terminal in the viewer (pty, VT emulation)", m=["term.open", "term.input", "term.resize", "term.snap", "term.restart"], pf="term", c="mpp", lane=L["term"], ev="term/: pty + vt.rs + emb.rs")
R(A, "Embedded browser (CEF), browser.place/embed", m=["browser.embed", "browser.place", "browser.seen"], c="npp", lane=L["term"], ev="DECISION 2026-11-05: not ported; GPUI shows the JPEG-frame Browser tab (option A) and no CEF spike is planned (docs/gpui/BROWSER-EMBED.md). The helper runs headless; engine.live stays empty")
R(A, "Voice dictation, clarify words, Pebble ring", m=["bots.voice"], r=["/voice/in"], c="mpp", lane=L["voice"], ev="no handler; VoiceRecorder.swift/.kt")
R(A, "Secret box (secret.set/del/list/dismiss, vault)", m=["secret.set", "secret.del", "secret.list", "secret.dismiss"], c="mpp", lane=L["sett"], ev="settings/vault.rs (AES-256-GCM, JS file format, fail-closed, staged writes) + secret.rs RPCs; masks in mask.rs, NOT yet called by commit_many / agent lines / MCP answers (core+agents seams)")
R(A, "Rules and task board (rules.set/drop, board.set/drop)", m=["rules.set", "rules.drop", "board.set", "board.drop"], hub="partial", pf="state", c="mpp", lane=L["state"], ev="agents/rules.rs gates tools; the requests are unhandled; state keeps rule./board. settings")
R(A, "Told ledger (already told)", hub="partial", pf="state", c="nnn", lane=L["state"], ev="told.* settings kept; tools missing")

# ---- Hub housekeeping policies (state lane): what the Bend hub writes by itself at a turn's end and on the minute tick
A = "Hub housekeeping"
R(A, "Autosettle: tick, quiet marks, message metadata (Auto.changes, Auto.messages.metadata, Auto.quiet)", hub="done", pf="state", c="nnn", lane=L["state"], ev="state/autosettle.rs + Transform::before_commit; Bend-vs-Rust probe 72 runs (full+lean) 0 differ, 14 autosettle_* laws mapped")
R(A, "Autosettle legacy upgrade (Auto.upgrade: a home below autosettle.version 1)", hub="partial", pf="state", c="nnn", lane=L["state"], ev="ported (state/autosettle.rs `upgrade`: settles by the old mail receipts, autosettle.delivery.* for threads still on a shelf, autosettle.version 1; run once at start after the boot fold by Policy::finish_boot, as Hub.autosettle.upgrade runs in Hub.recover.v; until then a launched thread's receipts are the launch mails the fold keeps, ext.reports = Auto.old.reports). ACTIVE only once the Hub bootstrap stops writing autosettle.version itself (hub.rs, core lane: asked; the real-start test a_legacy_home_is_upgraded_once_at_a_real_start needs it). Probe: auto-legacy log compared with Bend's Auto.upgrade and Auto.changes. Folding an old log is unaffected either way: the migration only writes events")
R(A, "Autosettle: Shelve.auto.fix — Shelve.step wakes (a turn starting, work out again...), Shelve.auto.clean, Pend.fix; Shelve.auto.messages (a settled thread reopens on a new message) is PORTED (autosettle::reopen, probe key reopen, 14 messages x 7 logs)", hub="missing", pf="state", c="nnn", lane=L["state"], ev="not ported; laws autosettle_new_assistant_message_reopens, autosettle_new_turn_reopens stay unported. Includes Pend.fix (PendingChanged for opened/closed asks): the agents actor already writes PendingChanged around its own asks, so the policy must write it only when the count differs from the thread's current one, never twice")
R(A, "Auto.read: a thread opened in a viewer marks its newest reply read (autosettle.read.<t>)", hub="partial", pf="state", c="nnn", lane=L["state"], ev="autosettle::read + Policies::read exist; the view-open and desk-look handlers (parity-session) must call it and commit its events; until then a person's thread with output stays unread and never settles")
R(A, "Shelve.tick.post: the wake (ThreadWoke) when a snooze ends", hub="missing", pf="state", c="nnn", lane=L["state"], ev="not ported (runs with the settle tick)")
R(A, "Liveness: progress writes (liveness.<t> on agent output, owners above, restart.retry clear)", hub="partial", pf="state", c="nnn", lane=L["state"], ev="state/watchdog.rs (Watchdog.after) joins the batch before the quiet marks; Bend-vs-Rust probe 216 runs (full+lean, 3 batches) 0 differ; Watchdog.line is ported (state/watchdog.rs `line`, `Policies::line`) but nobody calls it yet: the agents lane must call it for every provider line and commit its events, or a streaming turn looks silent. NOT ported: Watchdog.progress.k also sends , the stalled-thread tick (Watchdog.tick) and its deliveries")
R(A, "Liveness deliveries: nudge into the stalled thread, notice to its launcher or Bub, work timeout, provider resume, retry tick", hub="missing", pf="state", c="nnn", lane=L["state"], ev="not ported; touches the agents and bots seams (Lr.send, Bots.wake): ask their owners first. ORDER: Watchdog.line (streamed deltas and tool output with no committed message, 5 s precision, also moves liveness.<t>) must land before the stalled-thread tick, or the tick nudges threads that are working")
R(A, "Alert, told and summary writes at a turn's end (alert.<t>, told.<key>, summary.at.<id>)", hub="partial", pf="state", c="nnn", lane=L["state"], ev="state/turnend.rs in Policy::derive, only for a TurnChanged the agents mark _end = true (the three Bend callers of Ended.out: agent lines and agent gone; Stop, spawn failure, restart cut and task cancel do not; the Transform strips the marker; SEAM ASKED of agents, until they set it nothing is written): summary.at.<t> clock, alert.<t> (Policy.end: person message, ready, notify.settled), told marks (Told.alerts), the ask alert records (Alerts.asks); hub-crate unit tests pass (h@backplane, provisional); the Bend-vs-Rust probe (Ended.out + Alerts.changes, 16 end batches) is written and waits for one Boat start. a failed end's failed.<msg> mark is moved after the alert as Stamp.fail.mark does. NOT ported: Triage.managed (reads false until mail lands), notify.all (Notice.of, reads off), Run.settle (mail runs), Told.prune (told.* is not kept in the lean state until it is), the summary text itself (Sum.out / summary.<id> and summary.at.<project>: needs the agents lane's summarize seam, asked)")

# ---- Onboarding, nudge, i18n
A = "Onboarding and polish"
R(A, "First-run tour", m=["setting.set"], c="mdd", lane=L["gscreens"], ev="onboard.done is a setting (no handler); ios build-tour.log, tour-timeline.png; android pair shot shows tour")
R(A, "Policies stay passive side by side (BACKPLANE_PASSIVE=1 / Policy::set_passive / start_with)", hub="partial", pf="state", c="nnn", lane=L["state"], ev="the mechanism is done and tested: no minute-tick write, no boot commit (Auto.upgrade included, real Hub start on a legacy log), no summary asked for (tests passive_mode_ticks_nothing_and_asks_for_no_summary, a_passive_start_commits_nothing_on_a_legacy_home); NOTHING TURNS IT ON yet: only the env var read by Policy::new. When BACKPLANE_SIDE_BY_SIDE lands the hub must derive passive from it (start_with(hub, Some(true))); until then switch.sh side mode must export BACKPLANE_PASSIVE=1 (switchover/agents)")
R(A, "Star nudge", m=["star.nudge"], pf="state", c="mpp", lane=L["state"], ev="state/star.rs (4 tests, 3 star_nudge_* laws mapped); the star.nudge request (open / dismiss) is ANSWERED by statehost (RpcHandler); the count is `Policies::star_prompt(msg_id, now)` (events to commit with the prompt + the targeted info frame) and is called by bots.say / bots.post (main.rs wires Bots::set_star to it); turn.start (agents) has its call written but not landed, bots.tell is not ported")
R(A, "Command palette and shortcuts (alt/ctrl digits, hints)", hub="n/a", c="pnn", lane=L["gscreens"], ev="GPUI input.rs bindings only; palette missing")
R(A, "i18n (en, ja)", hub="partial", c="mdd", lane=L["i18n"], ev="Rust prompt language T::PROMPT_LANG_K; iOS en/ja lproj, Android NativeLocale; GPUI strings are English literals")
R(A, "Light and dark themes, tokens", hub="n/a", c="dpp", lane=L["gscreens"], ev="GPUI theme.rs tokens, shots light/dark; phones' BackplaneTheme/Design")
R(A, "Logo and cats art", hub="n/a", c="ddd", lane=L["gscreens"], ev="shots/logo-cats checked by test/gpui/visual/check_logo_cats.py (manifests)")
R(A, "Window sizes, iPad split, phone layouts", hub="n/a", c="pdd", lane=L["gscreens"], ev="ipad-split.png, android emulator; GPUI fixed layout")
R(A, "Typing keeps up / streaming smoothness", hub="n/a", c="dpp", lane=L["gthread"], ev="GPUI input latency metrics (README contract); phones measured in results/ios/hubperf, results/android/measure.txt")

# ---- Core hub
A = "Hub core"
R(A, "Event store: events.jsonl compatible, torn tail, windowed index", hub="done", pf="state", c="nnn", lane=L["core"], ev="store.rs 28 bytes/event, 1M events 38 MB RSS (HUB-M1.md), restart-boat.txt; read model equals store.db")
R(A, "WebSocket join, log/changes/live/info frames, ping", r=["/ws"], hub="partial", c="ddd", lane=L["pedge"], ev="join and badauth identical; reconnect 1 difference")
R(A, "CBOR framing, view.control segmentation (256 KB)", hub="done", pf="hub.rs tests (seg)", c="ddd", lane=L["core"], ev="hub.rs segmented-reply test; android verify-segmented.out on JS hub")
R(A, "Keyed view deltas (viewDelta)", hub="partial", c="dnn", lane=L["pses"], ev="delta.rs + gpui delta.rs tests; session-gpuid 977 differences (uncapped, parity-rust7)")
R(A, "Copy guard (BACKPLANE_COPY)", hub="done", pf="copy.rs 15 tests", c="nnn", lane=L["core"], ev="copy.rs, test/gpui/hub/copy_gate.sh")
R(A, "One hub per home (hub.lock flock)", hub="done", pf="main.rs", c="nnn", lane=L["core"], ev="main.rs takes hub.lock; no e2e result file read")
R(A, "Startup and memory at scale", hub="done", pf="HUB-M1.md, results/hub", c="nnn", lane=L["soak"], ev="1M events 0.73 s; the 24 h soak (M7) was dropped by the person, 2026-11-05: gpui-soak lane holds a harness only")
R(A, "Bend rules sidecar (presenter) process", hub="partial", c="nnn", lane=L["core"], ev="presenter.rs calls bp-policy/window children; without flags the hub answers a canned reply (main.rs)")
R(A, "Cross-hub import of a Bend hub's log (and back)", hub="done", pf="state", c="nnn", lane=L["state"], ev="model.rs vs store.db fixtures; reverse direction unmeasured")
# FEATURES_END


# ---------------------------------------------------------------- output
STAT = {"d": "done", "p": "partial", "m": "missing", "n": "n/a"}
AREAS = []
for _r in F:
    if _r["area"] not in AREAS:
        AREAS.append(_r["area"])
COLS = ["Rust hub", "GPUI desktop", "iOS", "Android"]


def resolve(S):
    rows = []
    for r in F:
        hub, hnote = hub_status(S, r)
        cs = [STAT[ch] for ch in r["c"]]
        rows.append(dict(r, hub_s=hub, hub_note=hnote, cl=cs))
    return rows


def counts(rows):
    out = {}
    for i, name in enumerate(COLS):
        c = {"done": 0, "partial": 0, "missing": 0, "n/a": 0}
        for r in rows:
            c[r["hub_s"] if i == 0 else r["cl"][i - 1]] += 1
        out[name] = c
    return out


def blocked(r):
    return r["hub_s"] in ("partial", "missing") and any(c in ("done", "partial") for c in r["cl"]) and r["hub_s"] != "n/a"


def covered(rows):
    ms = {m for r in rows for m in r["m"]}
    ts = {t for r in rows for t in r["t"]}
    rs = {x for r in rows for x in r["r"]}
    return ms, ts, rs


def gate():
    d = os.path.join(ROOT, "build/gpui/results/parity-rust7/replay-diff.txt")
    if not os.path.exists(d):
        return None
    rows = re.findall(r"^(\S+) (identical|\d+ differences)", open(d).read(), re.M)
    return [(a, 0 if b == "identical" else int(b.split()[0])) for a, b in rows]


def render(S, rows):
    base = git("rev-parse", "--short", BASE).stdout.strip()
    head = git("log", "-1", "--format=%h %cs", BASE).stdout.strip()
    today = datetime.date.today().isoformat()
    cnt = counts(rows)
    o = []
    w = o.append
    w("# Feature parity: Rust hub and native clients\n")
    w(f"Generated by `scripts/gpui/parity-features.py --write` on {today} from commit `{base}` (branch `{BASE}`, {head}), "
      f"{len(rows)} rows. Do not edit the tables by hand: edit `FEATURES` in the script and run it again. "
      "Hub columns come from the source (registered requests, routes, tools, providers on `gpui` and every `gpui-*` lane branch); "
      "client columns and evidence are written by hand next to each row in the script.\n")
    w("Goal (2026-11-05): everything a user can do in today's Backplane works on the Rust hub (`native/host`) with the GPUI desktop, "
      "iOS and Android clients. No web page; the web client is only the reference for what exists.\n")
    gates = json.load(open(os.path.join(ROOT, "docs/gpui/perf-gates.json")))
    must = json.load(open(os.path.join(ROOT, "docs/gpui/must-pass.json")))
    w(f"Port target: `main` frozen at `{json.load(open('docs/gpui/port-target.json'))['main']}` (`docs/gpui/port-target.json`). Later `main` commits wait for the second pass.\n")
    w("## Performance gates\n")
    w("Performance is a first-class target beside feature parity. All must pass. Numbers come from `docs/gpui/perf-gates.json`, updated at each milestone.\n")
    w("| Gate | Target | Status | Latest |")
    w("|---|---|---|---|")
    for g in gates["gates"]:
        w(f"| {g['gate']} | {g['target']} | {g['status']} | {g['latest']} |")
    w("")
    final = json.load(open(os.path.join(ROOT, "docs/gpui/final-gate.json")))
    w("## Final gate: end-to-end journeys and fun to use\n")
    w("Run on each client against the Rust hub. Status lives in `docs/gpui/final-gate.json`; captures go in the gallery.\n")
    w("| Journey | GPUI | iOS | Android | Capture |")
    w("|---|---|---|---|---|")
    for m in final["journeys"]:
        w(f"| {m['item']} | {m['gpui']} | {m['ios']} | {m['android']} | {m['capture']} |")
    w("")
    w("| Fun to use | GPUI | iOS | Android | Evidence |")
    w("|---|---|---|---|---|")
    for m in final["fun"]:
        w(f"| {m['item']} | {m['gpui']} | {m['ios']} | {m['android']} | {m['evidence']} |")
    w("")
    w("## Must-pass interactions\n")
    w("Each needs a test, a check against the web and the reviewer's OK before it counts. Status lives in `docs/gpui/must-pass.json`.\n")
    w("| Item | GPUI | iOS | Android | Test |")
    w("|---|---|---|---|---|")
    for m in must["items"]:
        w(f"| {m['item']} | {m['gpui']} | {m['ios']} | {m['android']} | {m['test']} |")
    w("")
    w("## Counts\n")
    w("| Column | done | partial | missing | n/a | rows counted |")
    w("|---|---:|---:|---:|---:|---:|")
    for k, c in cnt.items():
        w(f"| {k} | {c['done']} | {c['partial']} | {c['missing']} | {c['n/a']} | {len(rows) - c['n/a']} |")
    nb = sum(1 for r in rows if blocked(r))
    w(f"\n{nb} rows carry **[hub-blocked]**: a client column is `done` or `partial` but the Rust hub row is not `done`, so the user cannot do it on the Rust hub yet.\n")
    w("## Rules for each status\n")
    w("- **done**: the code is present AND there is evidence it works: a test, a parity scenario, a results file or a screenshot, named in the evidence note. "
      "A feature only counts for a client when it works against the hub it is judged with.")
    w("- **partial**: code present without proof (no test, scenario or shot), or proof of only part of the row, or code that exists only on a lane branch that has not landed on `gpui`, "
      "or a row whose hub path is not yet at protocol parity (see the gate).")
    w("- **missing**: no code, a stub of 3 lines or less, or the request answers `unknown method`.")
    w("- **n/a**: the column truly does not apply (a desktop-only or phone-only feature, or a hub-internal row for a client).")
    w("- **Rust hub column** is derived: `missing` when none of the row's requests/tools/routes/providers is registered anywhere; `partial` when only some are, "
      "or all are registered but the row names no proof, or the only code is on a lane branch; `done` when every one is registered on `gpui` and the row names a proof. "
      "Rows without requests carry a hand-written hub status under the same rules. A client never makes a hub row better, and the hub row is not credited because Bend's presenter draws the screen.")
    w("- The iOS and Android apps run today's Bend client (`bridge.js`) inside native screens, so the code exists for most rows; their proof so far is on the JS hub "
      "(`build/gpui/shots/{ios,android}`, `build/gpui/results/{ios,android}`), none on the Rust hub. A phone cell is `done` only where a shot or result file shows that feature, otherwise `partial`.\n")
    w("## Protocol gate (parity differences)\n")
    g = gate()
    if g:
        tot = sum(n for _, n in g)
        sess = sum(n for a, n in g if a.startswith("session"))
        w("Source: `build/gpui/results/parity-rust7/replay-diff.txt` (recording A replayed against the Rust hub with the presenter, gpui 0688158f6, uncapped compare with --all, Boat job j20261105072654-320657, TZ pinned). True total 1621 differences (the earlier 127 was a 40-per-scenario cap in compare.mjs).\n")
        w("| scenario | differences |")
        w("|---|---:|")
        for a, n in g:
            w(f"| {a} | {n} |")
        w(f"| **total** | **{tot}** |\n")
        w(f"The two figures in use reconcile: **{sess}** are the three session scenarios (3 x 42) and **{tot}** is that plus the one `reconnect` difference "
          "(origin is a hex hash in Rust, the decimal `at` of the first change in Bend; harness note N19). `badauth` and `join` are identical. "
          "Per session scenario, 30 of the 42 are the bounded sidebar fields (`nativeSidebar`, `sidebarPages`, `threadsPage`, `settledPage`, `archivedPage`, `snoozedPage`, counts) "
          "that the JS hub sends and the Rust reply lacks, which is slim's lane (`sidebar.rs`); the rest are empty scratch keys and `view.*` info keys (`build/gpui/results/parity-rust5/GAPS.md`). "
          "I found no newer diff file than rust6 and no number from `slim` after it: `results/slim` holds timings only, so I cannot confirm that slim's 126 is the same 126 until the coordinator asks `slim` for its latest run. "
          "The gate is not zero, so every row that depends on `view.*` is at most `partial` on the hub.\n")
    else:
        w("`build/gpui/results/parity-rust7/replay-diff.txt` not found; not reconciled.\n")
    # lanes
    w("## Lanes\n")
    w("Owner per group of `missing` or `partial` rows. Existing lanes are from `docs/gpui/LANES.md`; **NEW** lanes are proposals with file ownership "
      "(one owner per file, own worktree `~/Documents/i2cjak/bp-gpui-lanes/<lane>`, branch `gpui-<lane>`). A lane that needs a request in `main.rs`/`hub.rs` asks the core lane for one `mod` line.\n")
    LANEFILES = {
        L["cmd"]: "`native/host/src/commands/` (thread.*, project.*, queue.*, fs.*, attach is files): registers one RpcHandler per family over `Hub::commit_many`; keeps the Bend rules (Fm.safe, settle, fork, queue) in Rust or calls the presenter child",
        L["sett"]: "`native/host/src/settings/` (setting.set and echo, accounts and fallback, models.refresh, telemetry, diagnostics, improvements, update, secrets vault, API keys, zones, usage, webhook)",
        L["bots"]: "`native/host/src/bots/` (bots.*, rooms, routines and clock, hooks, space, memory, mail, Google, uber ops, bot tools; `bots` MCP tools registered through the agents lane's tool table)",
        L["net"]: "`native/host/src/net/` (pairing and /auth/*, QR info, devices, push, far/machines and links, fmcp, /bots/* peers, desk.look, notifications)",
        L["term"]: "`native/host/src/term/` (pty, VT emulator, terminal tools, embedded terminals, the browser helper process, preview_* tools, teach and task runs)",
        L["mcp"]: "`native/host/src/agents/tools/` extension file set agreed with the agents lane: work/wake/lock/deploy/secret/settings/task tools and their requests (`work.cancel`, `wake.cancel`, `lock.*`, `deploy.*`)",
        L["orders"]: "`native/host/src/orders/` (fab.intent, order.buy, orderio watcher, project.home/start, order_review)",
        L["voice"]: "`native/host/src/voice/` (/voice/in, bots.voice ops, dictionary, key file)",
        L["i18n"]: "`native/host/src/text/` plus `native/gpui/src/strings.rs` (one string table, en and ja, from `src/core/i18n*`)",
        L["gscreens"]: "`native/gpui/src/screens/` (settings, onboarding tour, bots, rooms, forms, search field, palette, menus)",
        L["gview"]: "`native/gpui/src/viewers/` (board, schematic, 3D, Mech, Files, Diff, Review canvases and panes; reads the plot/GLB frames the files lane already serves)",
        L["gthread"]: "`native/gpui/src/thread/` (timeline rows, folds, asks and approvals, queue, images, attachments, multi-line composer)",
        L["ios"]: "`test/gpui/ios-rust/` + `mobile/ios` fixes: run the iOS app against the Rust hub on the store-copy-free fixtures and file a shot per row",
        L["andr"]: "`test/gpui/android-rust/` + `mobile/android` fixes: same for Android",
    }
    gl = {"Viewers": L["gview"], "Threads and timeline": L["gthread"], "Git and PR": L["gview"]}
    by = {}
    for r in rows:
        if r["hub_s"] in ("missing", "partial"):
            by.setdefault(r["lane"], []).append((r, "hub", r["hub_s"]))
        if r["cl"][0] in ("missing", "partial"):
            gln = r["lane"] if r["lane"].startswith("NEW gpui") else gl.get(r["area"], L["gscreens"])
            by.setdefault(gln, []).append((r, "gpui", r["cl"][0]))
    for lane in sorted(by, key=lambda x: (x.startswith("NEW"), x)):
        rs = by[lane]
        hm = sum(1 for r, k, s in rs if k == "hub" and s == "missing")
        hp = sum(1 for r, k, s in rs if k == "hub" and s == "partial")
        gm = sum(1 for r, k, s in rs if k == "gpui" and s == "missing")
        gp = sum(1 for r, k, s in rs if k == "gpui" and s == "partial")
        w(f"### {lane}\n")
        if lane.startswith("NEW"):
            w(f"Proposed. Owns: {LANEFILES.get(lane, '(files to be agreed)')}.\n")
        w(f"Rust hub: {hm} missing, {hp} partial. GPUI desktop: {gm} missing, {gp} partial.\n")
        for r, k, s in rs:
            w(f"- {r['area']} / {r['name']}: {k} {s}")
        w("")
    ph = [r for r in rows if any(c in ("missing", "partial") for c in r["cl"][1:])]
    w(f"### NEW ios-rust and NEW android-rust\n")
    w(f"Proposed. Own {LANEFILES[L['ios']]}; and {LANEFILES[L['andr']]}.\n")
    w(f"{len(ph)} rows have a phone cell that is not `done`; the cell is `partial` because the shipped app has the code but nothing was run against the Rust hub, or `missing` where the app has no code "
      "(Android Diagnostics). Their work is not blocked on new hub code for the rows whose hub cell is already `done` (files, git, review, kicad IPC, send/stop/ask): run those first. "
      "Every other row waits for its hub lane above.\n")
    w("Existing lanes with nothing to do on this list: the `soak` lane is closed (the 24 h run was dropped); `parity-session` and `parity-edge` own the 1621-difference gate.\n")
    # tables
    w("## Checklist\n")
    w("Columns: Rust hub, GPUI desktop, iOS, Android. `[hub-blocked]` as defined above.\n")
    for area in AREAS:
        w(f"### {area}\n")
        w("| Feature | Rust hub | GPUI | iOS | Android | Evidence |")
        w("|---|---|---|---|---|---|")
        for r in rows:
            if r["area"] != area:
                continue
            flag = " **[hub-blocked]**" if blocked(r) else ""
            ev = r["ev"].replace("|", "/")
            if r["hub_note"]:
                ev += " (hub: " + r["hub_note"].replace("|", "/") + ")"
            if r["proof"] and r["hub_s"] == "done":
                ev += " Proof: " + str(r["proof"]).replace("|", "/") + "."
            w(f"| {r['name'].replace('|', '/')}{flag} | {r['hub_s']} | {r['cl'][0]} | {r['cl'][1]} | {r['cl'][2]} | {ev} |")
        w("")
    # coverage appendices
    ms, ts, rs = covered(rows)
    w("## Appendix: requests, tools and routes (generated)\n")
    w("Every Bend request method, MCP tool and HTTP route, and where the Rust hub has it (`gpui` = landed, `gpui-<lane>` = only on that branch, blank = unknown method).\n")
    w("| Request | Bend source | Rust |")
    w("|---|---|---|")
    for m, src in S["bend_methods"].items():
        wh = where(S, "m", m)
        w(f"| `{m}` | {', '.join(src)} | {wh.replace('base', 'gpui') or '**none**'} |")
    w("\n| MCP tool | Rust |")
    w("|---|---|")
    for t_ in sorted(S["bend_tools"]):
        wh = where(S, "t", t_)
        w(f"| `{t_}` | {wh.replace('base', 'gpui') or '**none**'} |")
    w("\n| Route | Rust |")
    w("|---|---|")
    rr = set(S["bend_routes"]) | set(S["rust"]["routes"])
    for x in sorted(rr):
        wh = where(S, "r", x)
        w(f"| `{x}` | {wh.replace('base', 'gpui') or ('core (wire/auth)' if x in ('/hello', '/ws', '/client/capabilities') else '**none**')} |")
    w("\n| Provider | gpui lines | lane branches with code |")
    w("|---|---:|---|")
    for p_ in sorted(set(S["rust"]["providers"]) | {"claude"}):
        ln = S["rust"]["providers"].get(p_, "core (agents/proc.rs)")
        lb = ",".join(b for b, v in S["lanes"].items() if v["providers"].get(p_, 0) > 5 and not (S["rust"]["providers"].get(p_, 0) > 5))
        w(f"| {p_} | {ln} | {lb} |")
    return "\n".join(o) + "\n"


def check(S, rows):
    ms, ts, rs = covered(rows)
    bad = []
    skip_m = {"view.get", "view.ver", "view.send", "view.body", "mobile.screen", "mobile.history", "frontend.screen", "frontend.history",
              "view.error", "view.show"}
    unc_m = [m for m in S["bend_methods"] if m not in ms and m not in skip_m]
    unc_t = [t for t in S["bend_tools"] if t not in ts]
    unc_r = [r for r in S["bend_routes"] if r not in rs and r not in ("/plot/info",)]  # /plot/info: the web client only
    unk = [x for x in ms if x not in S["bend_methods"]] + [x for x in ts if x not in S["bend_tools"]]
    for h in F:
        if h["hub"] == "done" and not h["proof"] and not (h["m"] or h["t"] or h["r"] or h["prov"]):
            bad.append("hub done without proof: " + h["name"])
    return unc_m, unc_t, unc_r, unk, bad


if __name__ == "__main__":
    S = collect()
    rows = resolve(S)
    if "--sets" in sys.argv:
        print(json.dumps(S, indent=1))
    elif "--check" in sys.argv:
        um, ut, ur, unk, bad = check(S, rows)
        print("uncovered requests:", um)
        print("uncovered tools:", ut)
        print("uncovered routes:", ur)
        print("rows naming something Bend does not have:", unk)
        print("problems:", bad)
        sys.exit(1 if (um or ut or ur or unk or bad) else 0)
    else:
        text = render(S, rows)
        if "--write" in sys.argv:
            open(os.path.join(ROOT, "docs/gpui/PARITY.md"), "w").write(text)
            print("wrote docs/gpui/PARITY.md", len(text), "bytes;", {k: v for k, v in counts(rows).items()})
        else:
            print(text)
