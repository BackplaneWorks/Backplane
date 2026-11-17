#!/usr/bin/env python3
"""The edit-keys table (lib/edit_keys_table.json) on Linux: real X key events (XTest) with the keyboard in the embedded editor's canvas,
judged from KiCad's own edit events (the hub's "kicad embed event:" lines, BACKPLANE_KICAD_DEBUG=1), KiCad's selection and windows and
the files on disk. Never pixels, never the modified flag.

  edit_keys_drive.py table EDITOR OUT SOCK KPID DOCFILE      EDITOR pcb|sch: every row but per_actor_undo, in an order that needs undo
                                                             only after the undo rows (cut then paste restores itself; copy adds)
  edit_keys_drive.py actor-before EDITOR OUT SOCK KPID       the person's delete of everything (per_actor_undo's first half)
  edit_keys_drive.py actor-after EDITOR OUT SOCK KPID DOCFILE SEQ   Ctrl+Z after the agent's commit (made in between by the suite)

Prints "ROW <editor> <id> <works|nothing|wrong> <evidence>" per row and "SEQ <n>" for actor-before.
"""
import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
XINPUT = os.path.join(REPO, "test", "cef", "xinput.py")
DRIVER = os.path.join(HERE, "x11_embed_driver.py")
IPC = os.path.join(HERE, "gui_ipc_probe.py")
WAIT_S = json.load(open(os.path.join(HERE, "edit_keys_table.json")))["wait_s"]
TITLE = {"pcb": "PCB Editor", "sch": "Schematic Editor"}

editor, OUT, SOCK, KPID = sys.argv[2], sys.argv[3], sys.argv[4], int(sys.argv[5])
HUBLOG = os.path.join(OUT, "hub.log")


def x(*a):
    if a and a[0] in ("key", "chord"):
        quiet()
    subprocess.run([sys.executable, XINPUT, *map(str, a)], check=False, capture_output=True)


def quiet(t=40):
    """KiCad idle before a key: the check that follows the latest edit has reported (an ET_CHECKS_DONE after the latest ET_DOCUMENT_EDITED)
    and no check is in progress since. A key pressed while a check runs is handled when it ends, seconds later, and its edit would be
    taken for the next row's (the redo 'nothing' on bp.13.1 was that)."""
    end = time.time() + t
    while time.time() < end:
        es = evs()
        last_edit = max((int(e.get("seq", 0)) for e in es if e.get("type") == "ET_DOCUMENT_EDITED"), default=0)
        done = max((int(e.get("seq", 0)) for e in es if e.get("type") == "ET_CHECKS_DONE"), default=0)
        prog = max((int(e.get("seq", 0)) for e in es if e.get("type") == "ET_PROGRESS"), default=0)
        if (last_edit == 0 or done > last_edit) and prog < done:
            return True
        time.sleep(0.2)
    print(f"INFO quiet: KiCad still checking after {t} s", flush=True)
    return False


def evs():
    out = []
    for ln in open(HUBLOG, errors="replace"):
        i = ln.find("kicad embed event: ")
        if i < 0:
            continue
        try:
            out.append(json.loads(ln[i + 19:]))
        except ValueError:
            pass
    return out


def seq():
    return max((int(e.get("seq", 0)) for e in evs()), default=0)


def edits_since(n):
    """KiCad's own edit events after seq n: [(created, deleted)]."""
    r = []
    for e in evs():
        if int(e.get("seq", 0)) > n and e.get("type") == "ET_DOCUMENT_EDITED":
            try:
                d = json.loads(e.get("detail") or "{}")
            except ValueError:
                d = {}
            r.append((len(d.get("created") or []), len(d.get("deleted") or [])))
    return r


def wait_edits(n, t=WAIT_S):
    end = time.time() + t
    while time.time() < end:
        if edits_since(n):
            time.sleep(0.6)  # one key may land as more than one commit
            return edits_since(n)
        time.sleep(0.15)
    return []


def sums(ed):
    return sum(c for c, d in ed), sum(d for c, d in ed)


def checks_idle(n, t=15):
    """KiCad's checks after an edit have reported (ET_CHECKS_DONE after n), so the next key is not taken while they run."""
    end = time.time() + t
    while time.time() < end:
        if any(int(e.get("seq", 0)) > n and e.get("type") == "ET_CHECKS_DONE" for e in evs()):
            return True
        time.sleep(0.2)
    return False


def frame():
    try:
        ws = json.loads(subprocess.run([sys.executable, DRIVER, "list-windows", "--match", TITLE[editor]], capture_output=True, text=True).stdout)
    except ValueError:
        return None
    ws = [w for w in ws if w.get("shown") and w["w"] > 300]
    return ws[0] if ws else None


def windows_of_kicad():
    try:
        ws = json.loads(subprocess.run([sys.executable, DRIVER, "list-windows", "--pid", str(KPID)], capture_output=True, text=True).stdout)
    except ValueError:
        return []
    return [w for w in ws if w.get("shown") and w["w"] > 60]


class FrameGone(Exception):
    pass


def canvas_point(fx, fy):
    f = frame()
    if not f:
        raise FrameGone(f"no {TITLE[editor]} frame shown (KiCad closed?)")
    # the canvas: the frame less its toolbars (about 90 px on top, 40 at the left, 30 at the bottom)
    cx, cy, cw, ch = f["root_x"] + 40, f["root_y"] + 90, f["w"] - 80, f["h"] - 130
    return int(cx + cw * fx), int(cy + ch * fy)


def focus():
    """The keyboard in the canvas as a person gives it: Escape, a click on an empty corner of the canvas, Escape."""
    x("key", "Escape")
    time.sleep(0.2)
    x("click", *canvas_point(0.06, 0.92))
    time.sleep(0.5)
    x("key", "Escape")
    time.sleep(0.3)


def selection_count():
    r = subprocess.run([sys.executable, IPC, "selection", SOCK, editor], capture_output=True, text=True)
    try:
        return json.loads(r.stdout.strip().splitlines()[-1])["count"]
    except (ValueError, IndexError, KeyError):
        return -1


def row(rid, verdict, evidence):
    print(f"ROW {editor} {rid} {verdict} {evidence}", flush=True)
    return verdict


def file_stamp(path):
    try:
        st = os.stat(path)
        return (st.st_mtime_ns, subprocess.run(["sha256sum", path], capture_output=True, text=True).stdout.split()[0])
    except OSError:
        return (0, "")


def select_all():
    x("chord", "ctrl", "a")
    time.sleep(0.8)


def delete_with(rid, key):
    focus(); select_all(); n = seq(); x("key", key)
    ed = wait_edits(n); c, d = sums(ed)
    v = "works" if d > 0 and c == 0 else ("wrong" if ed else "nothing")
    row(rid, v, f"edits {ed}")
    if ed:
        checks_idle(n)
    return v


def undo_row(rid="undo"):
    n = seq(); x("chord", "ctrl", "z")
    ed = wait_edits(n); c, d = sums(ed)
    v = "works" if c > 0 and d == 0 else ("wrong" if ed else "nothing")
    row(rid, v, f"edits {ed}")
    if ed:
        checks_idle(n)
    return v


def redo_row(rid, chord):
    n = seq(); x("chord", *chord)
    ed = wait_edits(n); c, d = sums(ed)
    v = "works" if d > 0 and c == 0 else ("wrong" if ed else "nothing")
    row(rid, v, f"edits {ed}")
    if ed:
        checks_idle(n)
    return v


def restore():
    """Ctrl+Z once more to bring back what a row deleted (only used once undo has been judged works)."""
    n = seq(); x("chord", "ctrl", "z"); wait_edits(n); checks_idle(n, 8)


def paste_and_place(at):
    n = seq(); x("chord", "ctrl", "v"); time.sleep(1.0)
    x("move", *at); time.sleep(0.4); x("click", *at)
    ed = wait_edits(n); c, d = sums(ed)
    if not ed:  # a paste still on the pointer (a menu of KiCad's, a tool asking): Escape so the next row starts clean
        x("key", "Escape"); time.sleep(0.3)
    else:
        checks_idle(n)
    return ed, c, d


def table(docfile):
    # select_all: KiCad's own selection after Ctrl+A
    focus(); select_all(); k = selection_count()
    row("select_all", "works" if k > 1 else "nothing", f"KiCad's selection holds {k} items")
    x("key", "Escape"); time.sleep(0.3)
    # find: a new window of KiCad's titled Find
    focus(); before = {w["xid"] for w in windows_of_kicad()}; x("chord", "ctrl", "f")
    end, found = time.time() + WAIT_S, None
    while time.time() < end and not found:
        new = [w for w in windows_of_kicad() if w["xid"] not in before]
        found = next((w for w in new if "find" in (w.get("title") or "").lower()), None) or (new[0] if new else None)
        time.sleep(0.2)
    t = (found or {}).get("title") or ""
    row("find", "works" if found and "find" in t.lower() else ("wrong" if found else "nothing"), f"new window: {t or 'none'}")
    if found:
        x("key", "Escape"); time.sleep(0.6)
    # copy_paste: Ctrl+A, Ctrl+C, Escape, Ctrl+V, a click
    focus(); select_all(); x("chord", "ctrl", "c"); time.sleep(0.4); x("key", "Escape"); time.sleep(0.3)
    ed, c, d = paste_and_place(canvas_point(0.5, 0.5))
    row("copy_paste", "works" if c > 0 and d == 0 else ("wrong" if ed else "nothing"), f"edits {ed}")
    # cut, then cut_paste brings it back (so neither needs undo)
    focus(); select_all(); n = seq(); x("chord", "ctrl", "x")
    ed = wait_edits(n); c, d = sums(ed)
    cut = row("cut", "works" if d > 0 and c == 0 else ("wrong" if ed else "nothing"), f"edits {ed}")
    if ed:
        checks_idle(n)
    if cut == "works":
        focus(); ed, c, d = paste_and_place(canvas_point(0.5, 0.5))
        row("cut_paste", "works" if c > 0 and d == 0 else ("wrong" if ed else "nothing"), f"edits {ed}")
    else:
        row("cut_paste", "nothing", "not judged: the cut did nothing")
    # delete, undo, redo (Ctrl+Y), undo, redo (Ctrl+Shift+Z), undo
    dv = delete_with("delete", "Delete")
    if dv == "works":
        uv = undo_row()
        if uv == "works":
            if redo_row("redo", ("ctrl", "y")) == "works":
                restore()
            if redo_row("redo_shift_z", ("ctrl", "shift", "z")) == "works":
                restore()
        else:
            row("redo", "nothing", "not judged: undo did not work")
            row("redo_shift_z", "nothing", "not judged: undo did not work")
    else:
        for r in ("undo", "redo", "redo_shift_z"):
            row(r, "nothing", "not judged: Delete deleted nothing")
        uv = "nothing"
    # backspace: only on a full board (after a working undo, or when Delete did nothing and the board is whole)
    if uv == "works" or dv != "works":
        if delete_with("backspace", "BackSpace") == "works" and uv == "works":
            restore()
    else:
        row("backspace", "nothing", "not judged: the board is empty and undo did not work")
    # save: the board is modified now (the rows above); Ctrl+S writes the file
    focus(); s0 = file_stamp(docfile); x("chord", "ctrl", "s")
    end = time.time() + WAIT_S + 4
    while time.time() < end and file_stamp(docfile) == s0:
        time.sleep(0.2)
    s1 = file_stamp(docfile)
    row("save", "works" if s1[0] != s0[0] else "nothing", f"{os.path.basename(docfile)} {'written' if s1[0] != s0[0] else 'not written'}")


def actor_before():
    focus(); select_all(); n = seq(); x("key", "Delete")
    ed = wait_edits(n)
    if ed:
        checks_idle(n)
    print(f"INFO per_actor_undo: the person's delete: edits {ed}", flush=True)
    print(f"SEQ {seq()}", flush=True)


def actor_after(docfile, n0):
    # the agent's commit landed between: its edit event after n0 (created 1)
    agent = edits_since(n0)
    print(f"INFO per_actor_undo: the agent's commit: edits {agent}", flush=True)
    focus(); n = seq(); x("chord", "ctrl", "z")
    ed = wait_edits(n); c, d = sums(ed)
    if ed:
        checks_idle(n)
    focus(); s0 = file_stamp(docfile); x("chord", "ctrl", "s")
    end = time.time() + WAIT_S + 4
    while time.time() < end and file_stamp(docfile) == s0:
        time.sleep(0.2)
    text = open(docfile, errors="replace").read()
    has_track = "(start 120 100)" in text and "(end 140 100)" in text
    fps = text.count("(footprint ")
    if not agent:
        v = "nothing"; why = "not judged: the agent's commit made no edit event"
    elif c > 0 and d == 0 and has_track:
        v = "works"; why = f"Ctrl+Z edits {ed}; saved file has the agent's track and {fps} footprints"
    elif d > 0 or not has_track:
        v = "wrong"; why = f"Ctrl+Z edits {ed}; the agent's track {'in' if has_track else 'NOT in'} the saved file; {fps} footprints"
    else:
        v = "nothing"; why = f"Ctrl+Z edits {ed}; agent's track in the file: {has_track}"
    row("per_actor_undo", v, why)


REAL = {"pcb": {"footprint": 1, "shape": 3, "text": 7, "track": 11, "via": 12, "arc": 13, "zone": 16},
        "sch": {"junction": 19, "no_connect": 20, "line": 23, "text": 27, "label": 30, "global_label": 31, "hier_label": 32, "symbol": 35}}
MARKER = {"pcb": 14, "sch": 18}


def count(kot):
    r = subprocess.run([sys.executable, IPC, "items-count", SOCK, editor, str(kot)], capture_output=True, text=True)
    try:
        return json.loads(r.stdout.strip().splitlines()[-1])["count"]
    except (ValueError, IndexError, KeyError):
        return -1


def real_items():
    return {k: count(v) for k, v in REAL[editor].items()}


def run_check():
    """A check in KiCad (RunChecks: DRC or ERC) and its end (ET_CHECKS_DONE): its markers are on the canvas after."""
    n = seq()
    subprocess.run([sys.executable, IPC, "run-checks", SOCK, editor], capture_output=True, text=True)
    return checks_idle(n, 60)


def agent_read(docfile):
    """An agent's read through the hub's kicad_ipc (GetItems), as the stand-in agent makes it: (ok, what it answered)."""
    log = os.path.join(OUT, "standin.log")
    before = sum(1 for l in open(log, errors="replace") if l.startswith("MCPCALL")) if os.path.exists(log) else 0
    kot = "KOT_PCB_FOOTPRINT" if editor == "pcb" else "KOT_SCH_SYMBOL"
    call = {"name": "kicad_ipc", "arguments": {"path": docfile, "type": "kiapi.common.commands.GetItems", "message": {"header": {"document": "$doc"}, "types": [kot]}}}
    steps = json.dumps([{"method": "turn.start", "params": {"thread": "t1", "text": "mcpcall:" + json.dumps(call), "msg": f"mku{time.time_ns()}"}}])
    subprocess.run([os.environ.get("BUN", "bun"), os.environ["HUB_RPC_MJS"], os.environ["HPORT"], os.path.join(OUT, "home", "token"), steps], capture_output=True, text=True)
    end = time.time() + 60
    while time.time() < end:
        lines = [l for l in open(log, errors="replace") if l.startswith("MCPCALL")] if os.path.exists(log) else []
        if len(lines) > before:
            last = lines[-1]
            return ('"isError":true' not in last.replace(" ", "") and "AS_BUSY" not in last), last[:160].strip()
        time.sleep(0.25)
    return False, "no answer in 60 s"


def markers():
    r = subprocess.run([sys.executable, IPC, "get-markers", SOCK, editor], capture_output=True, text=True)
    try:
        return json.loads(r.stdout.strip().splitlines()[-1])["n"]
    except (ValueError, IndexError, KeyError):
        return -1


def undo_markers(docfile):
    """The undo-markers case (fork d2ce4ba5c6): select all with the check's markers in it, Delete, undo, redo, undo, a check run and an
    agent read between each step: what the delete took (its edit's deleted count) comes back whole at each undo (created equal to it),
    goes again at the redo; on the board KiCad's own counts per kind (markers aside) equal the start's after each undo; no dialog."""
    wins0 = {w["xid"] for w in windows_of_kicad()}
    c0 = real_items() if editor == "pcb" else {}
    notes, bad, took = [], [], [0]
    def between(tag):
        ran = run_check(); ok, said = agent_read(docfile)
        notes.append(f"{tag}: check {'done' if ran else 'no ET_CHECKS_DONE'}, {markers()} markers, agent read {'ok' if ok else 'FAILED: ' + said}")
        if not ok:
            bad.append(f"the agent's read after {tag} failed")
    def step(tag, keys, back):
        focus()
        if tag == "delete":
            select_all(); notes.append(f"select all: {selection_count()} selected, {markers()} markers on the {editor}")
        n = seq(); x(*keys); ed = wait_edits(n)
        if ed:
            checks_idle(n, 15)
        c, d = sums(ed)
        if tag == "delete":
            took[0] = d
            if d == 0: bad.append("delete: nothing deleted")
        elif back and (c != took[0] or d):
            bad.append(f"{tag}: {c} items back of the {took[0]} deleted (deleted {d})")
        elif not back and (d != took[0] or c):
            bad.append(f"{tag}: {d} deleted again of {took[0]} (created {c})")
        if back and c0:
            now = real_items()
            if now != c0: bad.append(f"after {tag} KiCad counts {now}, at the start {c0}")
        notes.append(f"{tag}: edits created {c} deleted {d}")
    between("start")
    step("delete", ("key", "Delete"), False); between("delete")
    step("undo", ("chord", "ctrl", "z"), True); between("undo")
    step("redo", ("chord", "ctrl", "y"), False); between("redo")
    step("undo again", ("chord", "ctrl", "z"), True); between("undo again")
    dialogs = [w.get("title") or "?" for w in windows_of_kicad() if w["xid"] not in wins0]
    if dialogs:
        bad.append(f"KiCad showed {dialogs}")
    for nline in notes:
        print(f"INFO undo_markers {editor}: {nline}", flush=True)
    v = "works" if not bad else ("nothing" if took[0] == 0 else "wrong")
    row("undo_markers", v, "; ".join(bad) or f"the {took[0]} deleted came back whole at each undo, no dialog")


def checked(n):
    """Wait for the automatic check after the edits since n (ET_CHECKS_DONE newer than the last edit), as a person pausing would."""
    quiet(40)


def composer_point():
    last = None
    for l in open(os.path.join(OUT, "client.log"), errors="ignore"):
        if "GPUI_METRIC " in l and '"controls":[' in l:
            try: last = json.loads(l.split("GPUI_METRIC ", 1)[1])["controls"]
            except ValueError: pass
    s = next((c["rect"] for c in (last or []) if c.get("id") == "send" and c.get("rect")), None)
    return (int(s["x"] - 300), int(s["y"] - 20)) if s else (600, 980)


def redo_after_check(via_backplane=False):
    """The row the fork's redo fix must turn green: Ctrl+A, Delete, Ctrl+Z, the automatic check finishes, Ctrl+Y by key with no click
    on the canvas between, the check again, Ctrl+Z. Expected (created, deleted): delete (0, N), undo (N, 0), redo (0, N), undo (N, 0).
    via_backplane: before it, the keyboard goes to Backplane (a click in its chat) and back (a click on the canvas), as a person's would."""
    rid = "redo_after_backplane" if via_backplane else "redo_after_check"
    if via_backplane:  # the keyboard to Backplane's composer, then back by a click on the canvas (no Escape while Backplane has the
        # keyboard: there it closes the board's pane)
        x("click", *composer_point()); time.sleep(0.8)
        x("click", *canvas_point(0.06, 0.92)); time.sleep(0.6); x("key", "Escape"); time.sleep(0.3)
    else:
        focus()
    select_all()
    got = []
    for keys in (("key", "Delete"), ("chord", "ctrl", "z"), ("chord", "ctrl", "y"), ("chord", "ctrl", "z")):
        n = seq(); x(*keys); got.append(sums(wait_edits(n))); checked(n)
    N = got[0][1]
    want = [(0, N), (N, 0), (0, N), (N, 0)]
    v = "works" if N > 0 and got == want else ("nothing" if N > 0 and got[2] == (0, 0) else "wrong")
    row(rid, v, f"delete, undo, redo, undo (created, deleted): {got}; expected {want}")
    if got[2] == (0, 0) and got[3] == (0, 0):  # the undo list stuck too: a click brings the board back for the rows after
        x("click", *canvas_point(0.06, 0.92)); time.sleep(0.5); n = seq(); x("chord", "ctrl", "z"); wait_edits(n)


def keys_cycles(via_backplane, cycles=10):
    """The bp.13.1 gate for the progress-reporter fix: `cycles` times Ctrl+A, Delete, Ctrl+Z, Ctrl+Y, Ctrl+Z, keys only (no click between
    them), each key once KiCad's automatic check after the latest edit has reported; then Ctrl+A after that check selects the whole
    board or sheet again. via_backplane: before each cycle the keyboard goes to Backplane's composer and back by a click on the canvas.
    One click on the canvas puts the keyboard in KiCad at the start; none after. Every cycle must give (0,N) (N,0) (0,N) (N,0)."""
    rid = "redo_after_backplane" if via_backplane else "redo_after_check"
    def whole(want):
        """Setup only, never judged: a failed cycle leaves the board emptied; clicks and Ctrl+Z bring it back for the next one."""
        for _ in range(4):
            x("chord", "ctrl", "a"); time.sleep(0.6); k = selection_count(); x("key", "Escape"); time.sleep(0.3)
            if k >= want > 0: return k
            x("click", *canvas_point(0.06, 0.92)); time.sleep(0.5); n = seq(); x("chord", "ctrl", "z"); wait_edits(n); checked(n)
        return k
    focus(); k0 = whole(1)
    bad, N = [], None
    for i in range(1, cycles + 1):
        if via_backplane:
            x("click", *composer_point()); time.sleep(0.8)
            x("click", *canvas_point(0.06, 0.92)); time.sleep(0.6)
        x("key", "Escape"); time.sleep(0.2); x("chord", "ctrl", "a"); time.sleep(0.6)
        got = []
        for keys in (("key", "Delete"), ("chord", "ctrl", "z"), ("chord", "ctrl", "y"), ("chord", "ctrl", "z")):
            n = seq(); x(*keys); got.append(sums(wait_edits(n))); checked(n)
        N = got[0][1] if N is None else N
        want = [(0, N), (N, 0), (0, N), (N, 0)]
        x("chord", "ctrl", "a"); time.sleep(0.8); k = selection_count(); x("key", "Escape"); time.sleep(0.3)
        ok = N > 0 and got == want and k >= k0
        print(f"INFO keys_cycles {editor} {rid} cycle {i}: {got} select-all after the check {k} (start {k0}) {'ok' if ok else 'FAIL'}", flush=True)
        if not ok:
            bad.append(f"cycle {i}: {got}, select all {k}")
            print(f"INFO keys_cycles {editor} {rid}: the board brought back for the next cycle by clicks and Ctrl+Z (setup): {whole(k0)}", flush=True)
    row(rid, "works" if not bad else "nothing", (f"{cycles} of {cycles} cycles (0,{N}) ({N},0) (0,{N}) ({N},0), select all {k0} after each, keys only" if not bad else f"{cycles - len(bad)} of {cycles} cycles right; " + "; ".join(bad[:4])))


def fresh_keys():
    """A freshly opened editor, no click on it at all: once its startup check has reported, Ctrl+A selects (KiCad's selection) and
    Ctrl+C puts KiCad's items on the clipboard (xclip). The keyboard must be KiCad's canvas's without the person clicking it."""
    end = time.time() + 120
    while time.time() < end and not any(e.get("type") == "ET_CHECKS_DONE" for e in evs()):
        time.sleep(0.3)
    started = any(e.get("type") == "ET_CHECKS_DONE" for e in evs())
    quiet(60)
    def clipboard():
        try:
            return subprocess.run(["xclip", "-selection", "clipboard", "-o"], stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=5).stdout
        except (OSError, subprocess.TimeoutExpired):
            return ""
    before = clipboard()
    x("chord", "ctrl", "a"); time.sleep(0.8); k = selection_count()
    x("chord", "ctrl", "c"); time.sleep(1.2)
    clip = clipboard()
    x("key", "Escape"); time.sleep(0.3)
    why = f"startup check {'reported' if started else 'NOT seen'}"
    row("fresh_select_all", "works" if k > 1 else "nothing", f"Ctrl+A with no click selected {k}; {why}")
    has = len(clip) > 20 and ("(" in clip) and clip != before
    row("fresh_copy", "works" if has and k > 1 else "nothing", f"clipboard after Ctrl+C: {len(clip)} chars{', ' + clip[:40].replace(chr(10), ' ') if clip else ''}")


def agent_check(docfile):
    """An agent's check through the hub's kicad_ipc (RunChecks, as an agent), waited to its end."""
    log = os.path.join(OUT, "standin.log")
    before = sum(1 for l in open(log, errors="replace") if l.startswith("MCPCALL")) if os.path.exists(log) else 0
    n = seq()
    call = {"name": "kicad_ipc", "arguments": {"path": docfile, "type": "kiapi.backplane.embed.RunChecks", "message": {"kind": "FK_PCB" if editor == "pcb" else "FK_SCH", "parity": editor == "pcb"}}}
    steps = json.dumps([{"method": "turn.start", "params": {"thread": "t1", "text": "mcpcall:" + json.dumps(call), "msg": f"mkc{time.time_ns()}"}}])
    subprocess.run([os.environ.get("BUN", "bun"), os.environ["HUB_RPC_MJS"], os.environ["HPORT"], os.path.join(OUT, "home", "token"), steps], capture_output=True, text=True)
    done = checks_idle(n, 90)
    lines = [l for l in open(log, errors="replace") if l.startswith("MCPCALL")] if os.path.exists(log) else []
    said = lines[-1][:140].strip() if len(lines) > before else "no answer"
    print(f"INFO agent_check {editor}: {'ET_CHECKS_DONE' if done else 'no ET_CHECKS_DONE'}; the hub answered: {said}", flush=True)
    return done


def snap(name):
    """A picture of the screen for the record (ImageMagick's import), beside the suite's own."""
    try:
        subprocess.run(["import", "-window", "root", os.path.join(OUT, "shots", f"{editor}-{name}.png")], capture_output=True, timeout=20)
    except (OSError, subprocess.TimeoutExpired):
        pass


def xraw(*a):
    """A key with no wait for KiCad's check (typing while one runs)."""
    subprocess.run([sys.executable, XINPUT, *map(str, a)], check=False, capture_output=True)


def props_typing(docfile):
    """Typing in KiCad's Properties panel while an automatic check runs: R1's Value field, its text replaced by "A" and "B" typed while
    the check runs (ET_PROGRESS seen, no ET_CHECKS_DONE yet), "C" after it reported, Enter; saved through the API: the saved board's R1
    must have Value "ABC" (the caret stayed in the field, every keystroke landed there) and R1 itself must still be there."""
    ipc = lambda *a: subprocess.run([sys.executable, IPC, *a], capture_output=True, text=True)
    ipc("run-action", SOCK, "common.Control.showProperties"); time.sleep(2)
    ipc("select-items", SOCK, "pcb", "R1", "0"); time.sleep(1.5)
    f = frame(); at = (f["root_x"] + 174, f["root_y"] + 422)  # R1's Value cell (Basic Properties, Fields: Reference, Value)
    quiet(60)
    xraw("click", *at); time.sleep(0.7); xraw("click", *at); time.sleep(0.8)
    xraw("key", "End"); xraw("chord", "shift", "Home"); time.sleep(0.2)
    snap("props-1-field-open")
    n = seq(); rc = ipc("run-checks", SOCK, "pcb").stdout.strip()
    xraw("type", "AB")   # at once: RunChecks answers when the check has started
    during = not any(e.get("type") == "ET_CHECKS_DONE" for e in evs() if int(e.get("seq", 0)) > n)
    running = during
    print(f"INFO props_typing: RunChecks {rc[:100]}; A,B typed {'before' if during else 'after'} the check reported", flush=True)
    snap("props-2-typed-AB")
    checks_idle(n, 60); time.sleep(0.5)
    xraw("type", "C"); time.sleep(0.3); snap("props-3-typed-C"); xraw("key", "Return"); time.sleep(1.5)
    ipc("save-doc", SOCK, "pcb"); time.sleep(2)
    text = open(docfile, errors="replace").read()
    import re
    m = re.search(r'\(footprint "[^"]*"(?:(?!\(footprint ).)*?\(property "Reference" "R1"(?:(?!\(footprint ).)*?\(property "Value" "([^"]*)"', text, re.S)
    val = m.group(1) if m else None
    ipc("run-action", SOCK, "common.Control.showProperties"); time.sleep(1.5); xraw("key", "Escape")
    ev = f"typed A,B {'while the check ran' if during else 'as the check started (it had already reported)'}, C after; R1's Value saved: {val!r}"
    # works: every keystroke in the field ("ABC"), whether the check was still running at A,B (a small board's check can end first: said)
    row("props_typing", "works" if val == "ABC" else ("wrong" if val not in (None, "1.5K") or m is None else "nothing"), ev)


def props_shot():
    """Discovery: KiCad's Properties panel shown and one footprint selected (R1), for a picture of where its fields are."""
    r = subprocess.run([sys.executable, IPC, "run-action", SOCK, "common.Control.showProperties"], capture_output=True, text=True)
    print(f"INFO props_shot: showProperties {r.stdout.strip()[:100]}", flush=True)
    time.sleep(2)
    r = subprocess.run([sys.executable, IPC, "select-items", SOCK, editor, "R1", "0"], capture_output=True, text=True)
    print(f"INFO props_shot: select R1 {r.stdout.strip()[:160]}", flush=True)
    time.sleep(2)
    f = frame(); print(f"INFO props_shot: frame {f and (f['root_x'], f['root_y'], f['w'], f['h'])}", flush=True)


def selall_probe(docfile):
    """Does select-all die after a check, and does a click on the canvas bring it back? Ctrl+A counts (KiCad's selection): before any
    check, after an explicit RunChecks with no click, after a click; then after an edit's automatic check with no click, after a click."""
    def sa(tag):
        x("chord", "ctrl", "a"); time.sleep(0.8); k = selection_count(); x("key", "Escape"); time.sleep(0.4)
        print(f"INFO selall_probe {editor} {tag}: Ctrl+A selected {k}", flush=True); return k
    focus(); sa("before any check")
    run_check(); sa("after RunChecks, no click")
    x("click", *canvas_point(0.06, 0.92)); time.sleep(0.5); sa("after a click")
    focus(); select_all(); n = seq(); x("key", "Delete"); wait_edits(n); checked(n)
    m = seq(); x("chord", "ctrl", "z"); u = wait_edits(m); checked(m)
    print(f"INFO selall_probe {editor} the undo after Delete and its check, no click: edits {u}", flush=True)
    sa("after an undo and its automatic check, no click")
    x("click", *canvas_point(0.06, 0.92)); time.sleep(0.5); sa("after a click")
    ok, said = agent_read(docfile); sa(f"after an agent read ({'ok' if ok else said[:60]}), no click")


def redo_split():
    """Which factor redo after KiCad's automatic check needs: Ctrl+Y as is, after Escape, after a click on the canvas, after an explicit
    RunChecks, after 10 s; and KiCad's Redo action. Each from a fresh delete and undo, each printed with its delete and undo."""
    for v in ("plain", "escape", "click", "explicit check", "10 s", "action"):
        focus(); select_all(); k = selection_count()
        n = seq(); x("key", "Delete"); d = wait_edits(n); checks_idle(n, 15)
        n = seq(); x("chord", "ctrl", "z"); u = wait_edits(n); checks_idle(n, 15)
        if v == "escape": x("key", "Escape"); time.sleep(0.4)
        elif v == "click": x("click", *canvas_point(0.06, 0.92)); time.sleep(0.5)
        elif v == "explicit check": run_check()
        elif v == "10 s": time.sleep(10)
        n = seq()
        if v == "action": subprocess.run([sys.executable, IPC, "run-action", SOCK, "common.Interactive.redo"], capture_output=True, text=True)
        else: x("chord", "ctrl", "y")
        r = wait_edits(n)
        print(f"INFO redo_split {editor} {v}: selected {k}, delete {sums(d)}, undo {sums(u)}, redo {sums(r)}", flush=True)
        if r:
            checks_idle(n, 15); m = seq(); x("chord", "ctrl", "z"); wait_edits(m); checks_idle(m, 15)


def redo_probe():
    """Why redo is nothing: A Ctrl+Y at once after the undo; B after KiCad's checks that follow the undo (as the table does); C the same
    with KiCad's own Redo action through the API instead of the key (is the redo list empty, or does the key not arrive?)."""
    def cycle(tag, wait_checks, how):
        focus(); select_all(); n = seq(); x("key", "Delete"); ed = wait_edits(n)
        if wait_checks: checks_idle(n, 15)
        n = seq(); x("chord", "ctrl", "z"); ed = wait_edits(n)
        if wait_checks: checks_idle(n, 15)
        between = [e.get("type") for e in evs() if int(e.get("seq", 0)) > n]
        n = seq()
        if how == "key":
            x("chord", "ctrl", "y")
        else:
            r = subprocess.run([sys.executable, IPC, "run-action", SOCK, "common.Interactive.redo"], capture_output=True, text=True)
            print(f"INFO redo_probe {editor} {tag}: RunAction {r.stdout.strip()[:120]}", flush=True)
        ed = wait_edits(n)
        print(f"INFO redo_probe {editor} {tag}: redo edits {ed}; events between the undo and the redo: {between}", flush=True)
        if ed:
            checks_idle(n, 15); m = seq(); x("chord", "ctrl", "z"); wait_edits(m); checks_idle(m, 15)
    cycle("A at once", False, "key")
    cycle("B after the checks", True, "key")
    cycle("C after the checks, KiCad's Redo action", True, "action")


if __name__ == "__main__":
    cmd = sys.argv[1]
    if not frame():
        print(f"ROW {editor} all nothing no {TITLE[editor]} frame shown", flush=True)
        sys.exit(1)
    if cmd == "table":
        table(sys.argv[6])
    elif cmd == "actor-before":
        actor_before()
    elif cmd == "selall-probe":
        selall_probe(sys.argv[6])
    elif cmd == "props-typing":
        props_typing(sys.argv[6])
    elif cmd == "fresh-keys":
        fresh_keys()
    elif cmd == "agent-check":
        agent_check(sys.argv[6])
    elif cmd == "props-shot":
        props_shot()
    elif cmd == "keys-cycles":
        for vb in (False, True):
            try:
                keys_cycles(vb, int(os.environ.get("KEYS_CYCLES", "10")))
            except FrameGone as e:
                row("redo_after_backplane" if vb else "redo_after_check", "wrong", str(e))
    elif cmd == "redo-after-check":
        for vb in (False, True):
            try:
                redo_after_check(vb)
            except FrameGone as e:
                row("redo_after_backplane" if vb else "redo_after_check", "wrong", str(e))
    elif cmd == "redo-split":
        redo_split()
    elif cmd == "redo-probe":
        redo_probe()
    elif cmd == "undo-markers":
        try:
            undo_markers(sys.argv[6])
        except FrameGone as e:
            row("undo_markers", "wrong", f"{e}; the hub: {open(HUBLOG, errors='replace').read().count('SIGSEGV')} SIGSEGV in its log")
    elif cmd == "actor-after":
        actor_after(sys.argv[6], int(sys.argv[7]))
