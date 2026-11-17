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
    subprocess.run([sys.executable, XINPUT, *map(str, a)], check=False, capture_output=True)


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


def canvas_point(fx, fy):
    f = frame()
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


if __name__ == "__main__":
    cmd = sys.argv[1]
    if not frame():
        print(f"ROW {editor} all nothing no {TITLE[editor]} frame shown", flush=True)
        sys.exit(1)
    if cmd == "table":
        table(sys.argv[6])
    elif cmd == "actor-before":
        actor_before()
    elif cmd == "actor-after":
        actor_after(sys.argv[6], int(sys.argv[7]))
