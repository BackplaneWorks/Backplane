"""Windows Edit focus regression; run under the shared bp-win-ui lock.

Uses the existing suite's isolated home/project and stand-in agents. No canvas
click or IPC focus request precedes the native focus and hotkey checks.
Usage: python win_opening_focus.py CLIENT HUB ASSETS KICAD OUT [PORT]
Set BP_OPENING_FOCUS_ONLY=click_control to run only the canvas-click control
(after the same Ctrl+` handback), e.g. to recheck a baseline after a change to it.
Set BP_OPENING_FOCUS_ONLY=ctrl_k to run only the Ctrl+K-from-KiCad check, straight after
Edit (keys proven KiCad's by plain F2), with no click first.
Set BP_OPENING_FOCUS_ONLY=mouse_return for the route with no hotkey hand-back: a real
click in Backplane's composer, then a real click on the editor tab or the canvas, x3 each.
Set BP_OPENING_FOCUS_ONLY=close for KiCad's own Close (Ctrl+W) in the editor the pane shows
(embed_close_e2e.sh): the pane goes back to View, Edit gives a fresh frame, the save prompt's
Cancel and Save. BP_OPENING_FOCUS_ONLY=savegate for the hub's save gate (embed_load_migration_e2e.sh):
a Board Setup press on a board KiCad migrated on load is not saved, a BOM edit is (no Edit pressed;
KEMBED_PROJ_SRC=<a project folder> seeds another project, as on Linux).
Set BP_OPENING_FOCUS_ONLY=checks for the Checks panel (Linux embed_checks_panel_e2e.sh): the panel opened by a
click, Run in it (RunChecks in KiCad, its answer and ET_CHECKS_DONE), the rows listed, a click on a DRC row and an
ERC row cross-probing into KiCad, Down, Fix with agent, and KiCad's DRC marker clicked (ClickMarker) naming its row.
It seeds a dangling label into the schematic before the hub starts.
Set BP_OPENING_FOCUS_ONLY=plan for the person's Approve in KiCad's plan view (embed_plan_person_e2e.sh's port):
an agent's plan proposed, plan view on, A/L/W/R on its corridors, then plan_get, the Plan panel and the agent's wake
(see plan_check for the BP_PLAN_* options).
Set BP_OPENING_FOCUS_HOST_FG=1 to make Backplane's window foreground from the
harness before that click when the handback left KiCad foreground, so clients
are compared from the same Backplane-foreground state.
"""
import glob
import hashlib
import json
import os
import re
import sys
import time

import win_embed_e2e as E


def focus_snapshot(frame=0):
    focus = E.foreground_focus_hwnd()
    return {
        "focus": focus,
        "focus_class": E.hwnd_class(focus) if focus else "",
        "focus_pid": E.hwnd_pid(focus) if focus else 0,
        "foreground": int(E.u32.GetForegroundWindow() or 0),
        "frame": frame,
        "inside_frame": bool(frame and focus and (focus == frame or E.u32.IsChild(frame, focus))),
    }


def thread_info():
    """The foreground queue's active/focus windows and the foreground window, as
    the fork's F3 diagnostic asks: read just before and ~50 ms after a click."""
    gti = E.GUITHREADINFO()
    gti.cbSize = E.ctypes.sizeof(E.GUITHREADINFO)
    ok = bool(E.u32.GetGUIThreadInfo(0, E.ctypes.byref(gti)))
    act, foc = (int(gti.hwndActive or 0), int(gti.hwndFocus or 0)) if ok else (0, 0)
    return {"foreground": int(E.u32.GetForegroundWindow() or 0), "active": act,
            "active_class": E.hwnd_class(act) if act else "", "focus": foc,
            "focus_class": E.hwnd_class(foc) if foc else ""}


def traced_click(x, y):
    before = thread_info()
    E.screen_click(x, y, 1)  # SendInput: hardware-level input with foreground rights
    time.sleep(0.05)
    return {"before_click": before, "after_click_50ms": thread_info()}


def window_box(hwnd):
    r = E.W.RECT()
    E.u32.GetWindowRect(hwnd, E.ctypes.byref(r))
    return int(r.left), int(r.top), int(r.right - r.left), int(r.bottom - r.top)


def wait_focus_in(frame, timeout_s, cls=None):
    """Poll until Win32 focus is inside `frame` (and of class `cls`) and a
    foreground window exists; a click activates asynchronously, so a fixed
    sleep can read the transient no-foreground state. Returns (snap, ms)."""
    start = time.monotonic()
    snap = focus_snapshot(frame)
    while time.monotonic() - start < timeout_s:
        snap = focus_snapshot(frame)
        if snap["foreground"] and snap["inside_frame"] and (cls is None or snap["focus_class"] == cls):
            return snap, round((time.monotonic() - start) * 1000)
        time.sleep(0.02)
    return snap, None


def request_clauses(h):
    """The window-state clauses of the runtime's canvas-focus guard, read as soon as the
    client's first kicad_focus_request reaches its log (the frame and container named in it)."""
    try:
        with open(h.p("client.log"), encoding="utf-8", errors="replace") as f:
            for line in f:
                if line.startswith("GPUI_LOG ") and "kicad_focus_request" in line:
                    r = json.loads(line[9:])["kicad_focus_request"]
                    fr, ho, pa = int(r["frame"]), int(r["holder"]), int(r["parent"])
                    fg = int(E.u32.GetForegroundWindow() or 0)
                    return {"frame": fr, "holder": ho, "parent": pa,
                            "frame_visible": bool(E.u32.IsWindowVisible(fr)), "holder_visible": bool(E.u32.IsWindowVisible(ho)),
                            "parent_visible": bool(E.u32.IsWindowVisible(pa)), "frame_in_holder": bool(E.u32.IsChild(ho, fr)),
                            "holder_parent_ok": int(E.u32.GetParent(ho) or 0) == pa, "foreground": fg,
                            "foreground_is_frame_or_inside": bool(fg and (fg == fr or E.u32.IsChild(fr, fg)))}
    except (OSError, ValueError, KeyError):
        pass
    return None


def give_kicad_keys(h, cp, frame, kind="PCB"):
    """The keys to the editor on show the way a person does without touching
    its canvas: its tab (kicad_pane: the tab of the editor shown takes the
    keyboard). Edit is no use once editing: it becomes View."""
    # The control list can outlive the viewer (run A5: a stray Escape closed it and the
    # "click" landed on empty space): the editor must be on show, or this fails as such.
    if not (frame and E.u32.IsWindow(frame) and E.u32.IsWindowVisible(frame)):
        snap = focus_snapshot(frame)
        return {"tab_clicked": False, "target_missing": "editor frame not on show", "settle_ms": None, **snap}
    clicked = h.click_control(E.find_main_window(cp.pid), "kicad-tab:" + kind)
    snap, ms = wait_focus_in(frame, 3)
    return {"tab_clicked": clicked, "settle_ms": ms, **snap}


def ctrl_k_check(h, cp, frame, rep, timeline):
    # A focus race (a stale KiCad focus notice took the keys back after the palette opened):
    # BP_CTRL_K_REPEAT runs it N times in one session; each round is its own check.
    rounds = max(1, int(os.environ.get("BP_CTRL_K_REPEAT", "1") or 1))
    for n in range(1, rounds + 1):
        ctrl_k_round(h, cp, frame, rep, timeline, n, rounds)
    return h.finish()


def ctrl_k_round(h, cp, frame, rep, timeline, n, rounds, pre=None):
    # Ctrl+K pressed in KiCad (a host chord): Backplane's window comes forward and its
    # palette takes the keys (Linux real_use ctrl_k_after_comment_ok is the model).
    if pre is None:
        time.sleep(0.5)
        pre = give_kicad_keys(h, cp, frame)
    timeline.append({"stage": "before_ctrl_k", **pre})
    top = int(rep.get("toplevel") or 0)
    wx, wy, ww, wh = window_box(top)
    pane_x = int((rep.get("container") or {}).get("root_x") or (wx + ww))
    box = (wx, wy, max(1, pane_x - wx), min(wh, 480))
    shots = [h.p(f"ctrlk{n}_0_before.png"), h.p(f"ctrlk{n}_1_open.png"), h.p(f"ctrlk{n}_2_typed.png")]
    E.grab_screen(shots[0])
    E.key_chord("ctrl", "k")
    back, back_ms = None, None
    start = time.monotonic()
    while time.monotonic() - start < 2:
        back = focus_snapshot(frame)
        if back["foreground"] == top and back["focus_pid"] == cp.pid:
            back_ms = round((time.monotonic() - start) * 1000)
            break
        time.sleep(0.02)
    time.sleep(0.6)
    E.grab_screen(shots[1])
    for ch in "zq":
        E.key_tap(ch)
    time.sleep(0.6)
    E.grab_screen(shots[2])
    opened = E.img_diff_box(shots[0], shots[1], *box)
    typed = E.img_diff_box(shots[1], shots[2], *box)
    ck = {"round": n, "tab_clicked": pre.get("tab_clicked"), "target_missing": pre.get("target_missing"), "precondition_inside_kicad": pre["inside_frame"], "foreground_ms": back_ms, "after": back,
          "palette_change_px": opened["changed_pixels"], "typed_change_px": typed["changed_pixels"], "box": opened["box"]}
    timeline.append({"stage": "ctrl_k_from_kicad", **ck})
    print("DIAG ctrl_k " + json.dumps(ck), flush=True)
    h.check(f"Ctrl+K from KiCad brings Backplane forward and its palette takes keys ({n}/{rounds})",
            pre["inside_frame"] and back_ms is not None and opened["changed_pixels"] > 500 and typed["changed_pixels"] > 20, ck)
    # One Escape closes the palette; a second reached Backplane's window and closed the KiCad viewer (run A5).
    E.key_tap("Escape")
    time.sleep(0.6)



def window_title(hwnd):
    buf = E.ctypes.create_unicode_buffer(256)
    E.u32.GetWindowTextW(hwnd, buf, 256)
    return buf.value


def kicad_toplevels():
    """Every top-level window of a kicad.exe process (pre-warmed frames are parked as top-levels, not in the pane)."""
    pids = set()
    for line in __import__("subprocess").run(["tasklist", "/FI", "IMAGENAME eq kicad.exe", "/FO", "CSV", "/NH"], capture_output=True, text=True).stdout.splitlines():
        parts = line.split('","')
        if len(parts) > 1 and parts[1].isdigit():
            pids.add(int(parts[1]))
    found = []
    @E.ctypes.WINFUNCTYPE(E.W.BOOL, E.W.HWND, E.W.LPARAM)
    def cb(h, _):
        pid = E.W.DWORD()
        E.u32.GetWindowThreadProcessId(h, E.ctypes.byref(pid))
        if pid.value in pids:
            found.append({"hwnd": int(h), "title": window_title(h), "class": E.hwnd_class(int(h)), "visible": bool(E.u32.IsWindowVisible(h))})
        return True
    E.u32.EnumWindows(cb, 0)
    return found


def prewarm_frames(top):
    """The editors the pane pre-warms (~2 s after the first is placed): frames titled Schematic Editor / 3D Viewer,
    in the pane or parked as KiCad top-levels."""
    want = ("Schematic Editor", "3D Viewer")
    inside = [w for w in E.list_child_windows(top) if any(t in (w.get("title") or "") for t in want)]
    return inside + [w for w in kicad_toplevels() if any(t in w["title"] for t in want)]


def wait_prewarm(h, top, mode, timeout_s=40):
    """mode "after": until the pre-warmed frames exist, then 1 s more; "during": until the client asks for the pre-warm.
    Returns what was seen, for the log."""
    start = time.monotonic()
    while time.monotonic() - start < timeout_s:
        if mode == "during":
            try:
                if "editor.kicad.prewarm" in open(h.p("client.log"), encoding="utf-8", errors="replace").read():
                    return {"prewarm": "requested", "ms": round((time.monotonic() - start) * 1000)}
            except OSError:
                pass
        else:
            pw = prewarm_frames(top)
            if len(pw) >= 1:
                time.sleep(1.0)
                return {"prewarm": "done", "frames": [(w["hwnd"], w["title"]) for w in prewarm_frames(top)], "ms": round((time.monotonic() - start) * 1000)}
        time.sleep(0.05)
    return {"prewarm": "not seen", "ms": round((time.monotonic() - start) * 1000)}


def frozen_typing(h, cp, frame, gl, cbox, kpid, tag):
    """Backplane's composer has the keys, KiCad is frozen with the pointer over its canvas: per-key latency (ms)."""
    top = E.find_main_window(cp.pid)
    keys_back_to_backplane(h, cp, frame, top, "composer", cbox)
    E.screen_move(gl["root_x"] + gl["w"] // 2, gl["root_y"] + gl["h"] // 2)
    time.sleep(0.4)
    suspended = E.suspend_pid(kpid, True)
    try:
        return [key_latency_ms(h, cbox, c, f"{tag}{i}") for i, c in enumerate("abcd")] if suspended else None
    finally:
        if suspended:
            E.suspend_pid(kpid, False)
        time.sleep(0.8)


def f6_check(h, cp, frame, children, timeline, kpid):
    """F6: does frozen typing slow down from canvas activity alone (wheel zooms over the canvas, no click into the
    pane, no return), and does it decay with time? One run, numbers only."""
    gls = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
    gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
    top = E.find_main_window(cp.pid)
    cbox = composer_box(h, top)
    res = {"baseline": frozen_typing(h, cp, frame, gl, cbox, kpid, "f6base")}
    # canvas activity with no click and no return: 20 wheel notches over the canvas (KiCad zooms the view under the pointer)
    E.screen_move(gl["root_x"] + gl["w"] // 2, gl["root_y"] + gl["h"] // 2)
    for i in range(20):
        E._send(E._mouse(0x0800, 120 if i % 2 else -120))
        time.sleep(0.15)
    time.sleep(1.0)
    res["after_wheel"] = frozen_typing(h, cp, frame, gl, cbox, kpid, "f6wheel")
    time.sleep(30)
    res["after_wheel_plus_30s"] = frozen_typing(h, cp, frame, gl, cbox, kpid, "f6decay")
    # then one tab return (the sequence L4 repeats ten times), and again
    keys_back_to_backplane(h, cp, frame, top, "composer", cbox)
    h.click_control(top, "kicad-tab:PCB")
    time.sleep(1.0)
    res["after_one_return"] = frozen_typing(h, cp, frame, gl, cbox, kpid, "f6return")
    print("DIAG f6 " + json.dumps(res), flush=True)
    timeline.append({"stage": "f6", **res})
    h.check("F6 numbers recorded (diagnostic, no bar)", all(v is not None for v in res.values()), res)
    return h.finish()


def message_panel_box(frame):
    """KiCad's message panel, the row above the status bar ("Pads Vias Track Segments ..."), which shows the details of
    a clicked item."""
    r = E.W.RECT()
    E.u32.GetWindowRect(frame, E.ctypes.byref(r))
    return (int(r.left) + 6, int(r.bottom) - 50, max(1, int(r.right - r.left) - 40), 26)


def coord_field_box(frame):
    """KiCad's status-bar cursor coordinates ("X 0.0000 Y 0.0000"), right of the zoom readout."""
    r = E.W.RECT()
    E.u32.GetWindowRect(frame, E.ctypes.byref(r))
    return (int(r.left) + 78, int(r.bottom) - 21, 150, 19)


def field_changed(h, box, act, tag, settle=0.8):
    a, b = h.p(f"{tag}_0.png"), h.p(f"{tag}_1.png")
    E.grab_screen(a)
    act()
    time.sleep(settle)
    E.grab_screen(b)
    return E.img_diff_box(a, b, *box)["changed_pixels"]


def zoom_back():
    """F1 (KiCad's zoom in) undoes the F2. Like every synthetic key it goes through E._send, which refuses a
    foreground that isn't the test's own (an F1 on Explorer opened Windows Help and Edge, bp-win 21:24:40)."""
    E.key_tap("F1")
    time.sleep(0.6)

def zoom_field_box(frame):
    """KiCad's status-bar zoom readout ("Z 2.20"), bottom left of the frame."""
    r = E.W.RECT()
    E.u32.GetWindowRect(frame, E.ctypes.byref(r))
    return (int(r.left) + 8, int(r.bottom) - 21, 70, 19)


def plain_key_acts(h, gl, tag, frame=0):
    """Plain F2 (KiCad's zoom out) acts: the zoom readout changes (a zoom over sparse board redraws only a few
    thousand canvas pixels, run F4d: Z 2.20 -> 1.50 with 2,688 px). F1 zooms back. Returns the canvas px (logged)
    and whether the readout changed (the pass)."""
    x, y, w, hh = int(gl["root_x"]), int(gl["root_y"]), int(gl["w"]), int(gl["h"])
    a, b = h.p(f"{tag}_0.png"), h.p(f"{tag}_1.png")
    E.grab_screen(a)
    E.key_tap("F2")
    time.sleep(1.0)
    E.grab_screen(b)
    px = E.img_diff_box(a, b, x, y, w, hh)["changed_pixels"]
    zoomed = None
    if frame:
        zoomed = E.img_diff_box(a, b, *zoom_field_box(frame))["changed_pixels"] > 15
    # always zoom back (F1 is a no-op where F2 did nothing): the view never drifts
    zoom_back()
    return px if zoomed is None else (px, zoomed)


def composer_box(h, top):
    """Backplane's message box, above the Attach button row (client px -> screen box)."""
    a = h.controls().get("ui:Attach") or h.probes().get("ui:Attach")
    r = (a.get("rect") if a and "rect" in a else a) or {}
    if not r:
        return None
    ox, oy = E.client_origin(top)
    return (int(ox + r["x"] - 320), int(oy + r["y"] - 85), 440, 60)


def typed_in_composer(h, box, text, tag):
    """Typing reaches the composer: its pixels change. The text is then removed."""
    a, b = h.p(f"{tag}_0.png"), h.p(f"{tag}_1.png")
    E.grab_screen(a)
    for ch in text:
        E.key_tap(ch)
    time.sleep(0.4)
    E.grab_screen(b)
    px = E.img_diff_box(a, b, *box)["changed_pixels"]
    for _ in text:
        E.key_tap("BackSpace")
    return px


def key_latency_ms(h, box, ch, tag, limit_s=3.0):
    """One key typed into the composer: ms until its pixels change (None if never within limit_s)."""
    import PIL.ImageGrab as G
    from PIL import ImageChops
    x, y, w, hh = box
    before = G.grab(bbox=(x, y, x + w, y + hh), all_screens=True).convert("RGB")
    t0 = time.monotonic()
    E.key_tap(ch)
    while time.monotonic() - t0 < limit_s:
        now = G.grab(bbox=(x, y, x + w, y + hh), all_screens=True).convert("RGB")
        if ImageChops.difference(before, now).getbbox():
            ms = round((time.monotonic() - t0) * 1000)
            E.key_tap("BackSpace")
            return ms
        time.sleep(0.01)
    return None


def foreground_never_lost(ms=600, step=0.01):
    """Sample the foreground window every ~10 ms for `ms`: any 0 is the F3 drop."""
    end = time.monotonic() + ms / 1000
    zeros, n = 0, 0
    while time.monotonic() < end:
        n += 1
        if not int(E.u32.GetForegroundWindow() or 0):
            zeros += 1
        time.sleep(step)
    return {"samples": n, "no_foreground_samples": zeros}


def keys_back_to_backplane(h, cp, frame, top, entry, cbox):
    """The entry route: Backplane gets foreground and keys, by a composer click or Ctrl+` from KiCad."""
    if entry == "composer":
        E.screen_click(cbox[0] + 120, cbox[1] + 30, 1)
    else:
        E.key_chord("ctrl", "grave")
    start = time.monotonic()
    host = focus_snapshot(frame)
    while time.monotonic() - start < 2 and not (host["foreground"] == top and host["focus_pid"] == cp.pid):
        time.sleep(0.02)
        host = focus_snapshot(frame)
    return host["foreground"] == top and host["focus_pid"] == cp.pid, host


def mouse_return_check(h, cp, frame, children, timeline, rounds=None, kpid=0):
    """F3+F4: from Backplane holding the keys (by a composer click, or by Ctrl+` from KiCad), a real click on
    (a) the editor tab, (b) the canvas gives KiCad the keys (plain F2 acts), Backplane never without a foreground
    window across the click. Then the hover checks (pointer over the pane never moves the keys; a frozen KiCad
    under the pointer never stalls Backplane's typing)."""
    rounds = rounds or max(1, int(os.environ.get("BP_RETURN_ROUNDS", "5") or 5))
    gls = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
    gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
    top = E.find_main_window(cp.pid)
    cbox = composer_box(h, top)
    # The pane pre-warms the other editors ~2 s after the first is placed; one of those frames took the foreground during
    # the first return of run F4c (F5). BP_RETURN_PREWARM=after (default) makes the first return after it, =during on it.
    pw = wait_prewarm(h, top, os.environ.get("BP_RETURN_PREWARM", "after"))
    timeline.append({"stage": "return_prewarm", **pw})
    print("DIAG return_prewarm " + json.dumps(pw), flush=True)
    for entry in ("composer", "hotkey"):
        for n in range(1, rounds + 1):
            for route in [r for r in ("tab", "canvas") if r in os.environ.get("BP_RETURN_ROUTES", "tab,canvas").split(",")]:
                name = f"Return {route} from {entry} {n}/{rounds}: KiCad gets the keys (F2 acts), foreground never lost"
                on_show = bool(frame and E.u32.IsWindowVisible(frame))
                if not (gl and cbox and on_show):
                    h.check(name, False, {"target_missing": {"canvas": bool(gl), "composer": bool(cbox), "editor_on_show": on_show}})
                    continue
                if entry == "hotkey":
                    # Ctrl+` is pressed in KiCad, so KiCad must have the keys first (the previous round's return)
                    kicad_first = focus_snapshot(frame)
                    if not kicad_first["inside_frame"]:
                        # the previous return failed: Edit's own path (the tab while KiCad is in front) puts the keys back
                        give_kicad_keys(h, cp, frame)
                        kicad_first = focus_snapshot(frame)
                    if not kicad_first["inside_frame"]:
                        # Edit's own path: close the viewer, Edit again (the opening fix gives KiCad the keys)
                        E.key_chord("ctrl", "grave")
                        time.sleep(0.3)
                        E.key_tap("Escape")
                        time.sleep(0.8)
                        h.click_control(top, "ui:Edit")
                        wait_focus_in(frame, 6, "wxGLCanvas")
                        kicad_first = focus_snapshot(frame)
                    if not kicad_first["inside_frame"]:
                        h.check(name, False, {"precondition": "KiCad did not have the keys for Ctrl+`", **kicad_first})
                        continue
                host_ok, host = keys_back_to_backplane(h, cp, frame, top, entry, cbox)
                import threading
                hold = {}
                sampler = threading.Thread(target=lambda: hold.update(E.keys_stay_in(frame)))
                if route == "tab":
                    clicked = h.click_control(top, "kicad-tab:PCB")
                    sampler.start()
                    around = {"clicked": clicked}
                    fg = foreground_never_lost()
                else:
                    around = traced_click(gl["root_x"] + 20, gl["root_y"] + gl["h"] - 30)
                    sampler.start()
                    shot = h.p(f"return_{entry}_{route}{n}_50ms.png")
                    E.grab_screen(shot)  # the title bar ~50 ms after the click: inactive = a visible flicker
                    fg = foreground_never_lost(550)
                    around["shot_50ms"] = shot
                sampler.join(2.0)
                after = focus_snapshot(frame)
                if after["foreground"]:
                    after["foreground_title"] = window_title(after["foreground"])
                    after["foreground_class"] = E.hwnd_class(after["foreground"])
                px, zoomed = plain_key_acts(h, gl, f"return_{entry}_{route}{n}", frame)
                d = {"host_first": host_ok, "host": host, "after": after, "f2_changed_px": px, "zoom_readout_changed": zoomed, **fg, **around}
                timeline.append({"stage": f"return_{entry}_{route}_{n}", **d})
                print(f"DIAG return_{entry}_{route}_{n} " + json.dumps(d)[:700], flush=True)
                # a real F2 zoom redraws the whole canvas (~212,000 px here); a hover highlight is a few thousand
                # no key ever reaches a hidden frame: a pre-warmed editor's title gains no "*" (KiCad's modified mark)
                hidden = [w["title"] for w in kicad_toplevels() + E.list_child_windows(top) if "Schematic Editor" in (w.get("title") or "")]
                d["hidden_schematic_titles"] = sorted(set(hidden))
                untouched = not any(t.lstrip().startswith("*") for t in hidden)
                h.check(name, host_ok and zoomed and fg["no_foreground_samples"] == 0 and untouched, d)
                h.check(f"Return {route} from {entry} {n}/{rounds}: the keys stay KiCad's for 500 ms after the hand-in (sampled)",
                        bool(hold.get("held")), hold)
    # Hover never moves the keys: composer focused, the pointer across the pane and back, no click; typing stays in the composer.
    if gl and cbox:
        keys_back_to_backplane(h, cp, frame, top, "composer", cbox)
        for k in range(8):
            E.screen_move(gl["root_x"] + 40 + k * 50, gl["root_y"] + 60 + k * 40)
            time.sleep(0.08)
        E.screen_move(cbox[0] + 200, cbox[1] - 120)
        time.sleep(0.3)
        snap = focus_snapshot(frame)
        px = typed_in_composer(h, cbox, "hv", "hover_typing")
        h.check("Pointer across the pane and back moves no keys: Win32 focus and typing stay in Backplane's composer",
                snap["focus_pid"] == cp.pid and not snap["inside_frame"] and px > 20, {"focus": snap, "typed_px": px})
        # Frozen KiCad under the pointer, the queues attached by hover only: Backplane's own typing must not stall,
        # and Ctrl+` / a click elsewhere leaves the pane within 1.5 s.
        if kpid:
            # The first seconds of a freeze, before IsHungAppWindow says so (~5 s): freeze with the pointer off the pane,
            # then within 1 s move it over the pane and type; then the hover-first order below.
            keys_back_to_backplane(h, cp, frame, top, "composer", cbox)
            E.screen_move(cbox[0] + 200, cbox[1] - 120)
            time.sleep(0.3)
            suspended = E.suspend_pid(kpid, True)
            try:
                t_freeze = time.monotonic()
                E.screen_move(gl["root_x"] + gl["w"] // 2, gl["root_y"] + gl["h"] // 2)
                time.sleep(0.2)
                lat = [key_latency_ms(h, cbox, c, f"early{i}") for i, c in enumerate("abcd")]
                since = round((time.monotonic() - t_freeze) * 1000)
                t0 = time.monotonic()
                E.screen_click(cbox[0] + 120, cbox[1] + 30, 1)
                while time.monotonic() - t0 < 1.5 and focus_snapshot(frame)["focus_pid"] != cp.pid:
                    time.sleep(0.02)
                click_ms = round((time.monotonic() - t0) * 1000)
                E.screen_move(gl["root_x"] + gl["w"] // 2, gl["root_y"] + gl["h"] // 2)
                t0 = time.monotonic()
                E.key_chord("ctrl", "grave")
                while time.monotonic() - t0 < 1.5 and focus_snapshot(frame)["focus_pid"] != cp.pid:
                    time.sleep(0.02)
                grave_ms = round((time.monotonic() - t0) * 1000)
                d = {"suspended": suspended, "key_latency_ms": lat, "typed_by_ms_after_freeze": since, "click_elsewhere_ms": click_ms, "ctrl_grave_ms": grave_ms}
                print("DIAG frozen_early " + json.dumps(d), flush=True)
                # per-key latency against a frozen KiCad is the parked F6 (pre-existing, ~380 ms after many returns): reported,
                # not judged; a key that never arrives, and the 1.5 s click and Ctrl+` answers, still are
                print("KNOWN F6 (parked) frozen per-key latency, early freeze: " + json.dumps(lat), flush=True)
                h.check("Freeze, then pointer over the pane within 1 s: every key reaches the composer (latency: parked F6), click elsewhere and Ctrl+` within 1.5 s",
                        suspended and all(v is not None for v in lat) and click_ms < 1500 and grave_ms < 1500, d)
            finally:
                if suspended:
                    E.suspend_pid(kpid, False)
            time.sleep(1.0)
            keys_back_to_backplane(h, cp, frame, top, "composer", cbox)
            E.screen_move(gl["root_x"] + gl["w"] // 2, gl["root_y"] + gl["h"] // 2)
            time.sleep(0.4)
            suspended = E.suspend_pid(kpid, True)
            try:
                # the composer keeps the keys; the pointer stays over the frozen KiCad while typing
                lat2 = [key_latency_ms(h, cbox, c, f"hoverfirst{i}") for i, c in enumerate("wxyz")]
                print("DIAG frozen_hover_first " + json.dumps({"key_latency_ms": lat2}), flush=True)
                fz = [key_latency_ms(h, cbox, c, f"frozen{i}") for i, c in enumerate("fz")]
                px = 1000 if all(v is not None for v in fz) else 0
                typing_ms = max(v for v in fz if v is not None) if any(v is not None for v in fz) else 99999
                snap = focus_snapshot(frame)
                print("KNOWN F6 (parked) frozen per-key latency, pointer over the pane: " + json.dumps(fz), flush=True)
                h.check("Frozen KiCad under the pointer: every key reaches the composer (latency: parked F6)",
                        suspended and px > 0 and snap["focus_pid"] == cp.pid, {"suspended": suspended, "key_latency_ms": fz, **snap})
                # a click elsewhere in Backplane (the composer), then Ctrl+`: both answered within 1.5 s
                t0 = time.monotonic()
                E.screen_click(cbox[0] + 120, cbox[1] + 30, 1)
                while time.monotonic() - t0 < 1.5 and focus_snapshot(frame)["focus_pid"] != cp.pid:
                    time.sleep(0.02)
                click_ms = round((time.monotonic() - t0) * 1000)
                t0 = time.monotonic()
                E.key_chord("ctrl", "grave")
                while time.monotonic() - t0 < 1.5 and focus_snapshot(frame)["focus_pid"] != cp.pid:
                    time.sleep(0.02)
                ms = round((time.monotonic() - t0) * 1000)
                h.check("Frozen KiCad under the pointer: a click elsewhere and Ctrl+` answered within 1.5 s, the hub answers",
                        suspended and ms < 1500 and click_ms < 1500 and h.hub_hello(), {"ctrl_grave_ms": ms, "click_elsewhere_ms": click_ms})
            finally:
                if suspended:
                    E.suspend_pid(kpid, False)
    return h.finish()


# ---- checks: the Checks panel and KiCad's markers (embed_checks_panel_e2e.sh and the marker click of embed_editor_checks_e2e.sh) ----

LIBTABLE_MINE = {}
BP_LIBS = ["Backplane_Logo", "Backplane_MountingHole", "Backplane_Passives_Least", "Backplane_Passives_Most", "Backplane_Passives_Nominal", "Backplane_Shoggoth"]


def checks_project(h):
    """The seeded project's .kicad_pro (the seed puts it at the root or under boards/ecc83-pp)."""
    pro = h.p("proj", "ecc83-pp.kicad_pro")
    return pro if os.path.isfile(pro) else h.p("proj", "boards", "ecc83-pp", "ecc83-pp.kicad_pro")


def checks_seed(h):
    """embed_checks_panel_e2e.sh's seed: a dangling label in the schematic (ERC label_dangling), committed. The board's own DRC
    findings (the seed board has several naming a part) are the DRC rows. Runs before the hub starts, so its first checks see it."""
    import subprocess
    sch = checks_project(h)[:-len(".kicad_pro")] + ".kicad_sch"
    with open(sch, encoding="utf-8", newline="") as f:
        s = f.read()
    nl = "\r\n" if "\r\n" in s else "\n"
    lab = ('\t(label "DANGLE"\n\t\t(at 30.48 30.48 0)\n\t\t(effects\n\t\t\t(font\n\t\t\t\t(size 1.27 1.27)\n\t\t\t)\n\t\t\t(justify left bottom)\n\t\t)\n'
           '\t\t(uuid "5b0c1e6e-0000-4000-8000-00000000d00d")\n\t)\n').replace("\n", nl)
    put = "\t(sheet_instances" in s
    with open(sch, "w", encoding="utf-8", newline="") as f:
        f.write(s.replace("\t(sheet_instances", lab + "\t(sheet_instances", 1))
    try:
        d = os.path.dirname(sch)
        subprocess.run(["git", "add", "-A"], cwd=d, capture_output=True, timeout=30)
        subprocess.run(["git", "commit", "-qm", "a dangling label"], cwd=d, capture_output=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        pass
    return {"schematic": sch, "label_added": put}


def log_lines(h, name="client.log"):
    try:
        with open(h.p(name), encoding="utf-8", errors="replace") as f:
            return f.read().splitlines()
    except OSError:
        return []


def checks_watch(h, pro):
    """The Checks panel's data as the hub gives the client (mgmt.watch's data.checks), {} when none."""
    for p in (pro, pro.replace("\\", "/")):
        out = h.hub_rpc([{"method": "mgmt.watch", "params": {"project": p}}])
        line = next((l for l in out.splitlines() if l.startswith("{")), "{}")
        try:
            d = ((json.loads(line).get("result") or {}).get("data") or {}).get("checks") or {}
        except ValueError:
            d = {}
        if d:
            return d
    return {}


def checks_fresh(d):
    return (d.get("drc") or {}).get("state") == "fresh" and (d.get("erc") or {}).get("state") == "fresh"


def ctl_point(h, top, cid):
    """A drawn control's screen point (the newest controls metric naming it), as the Linux rect_xy: its left part (min(w/2, 60) in),
    clear of the row's Fix button; None when not drawn."""
    c = h.controls().get(cid)
    r = c.get("rect") if c else None
    if not isinstance(r, dict) or "x" not in r:
        return None
    ox, oy = E.client_origin(top)
    return int(ox + r["x"] + min(r["w"] / 2, 60)), int(oy + r["y"] + r["h"] / 2)


def ipc_module():
    import importlib.util
    spec = importlib.util.spec_from_file_location("gui_ipc_probe", E.IPC_PY)
    g = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(g)
    return g


def click_and_wait_selected(h, g, sock, xy, kind, want, secs=6.0):
    """embed_checks_panel_e2e.sh's click_and_wait: a real click at xy, then KiCad's GetSelection (one IPC client, polled) until it holds
    `want` (a reference), or anything when want is empty. {ok, ms, sel}."""
    import re
    cli = g.KiCadIpc(sock)

    def sel():
        _, val = cli.call("kiapi.common.commands.GetSelection", g.pb_field_msg(1, g.pb_field_msg(1, cli.first_open_doc_bytes(kind))), retries=1)
        refs, n = set(), 0

        def walk(buf, depth):
            if depth > 12:
                return
            try:
                fields = g.pb_decode_fields(buf)
            except Exception:
                return
            for vals in fields.values():
                for v in vals:
                    if not isinstance(v, (bytes, bytearray)):
                        continue
                    try:
                        if re.fullmatch(r"[A-Z]{1,6}[0-9]{1,6}[A-Z]?", v.decode("utf-8")):
                            refs.add(v.decode("utf-8"))
                    except UnicodeDecodeError:
                        pass
                    walk(v, depth + 1)
        for item in g.pb_decode_fields(val).get(1, []):
            n += 1
            walk(item, 0)
        return {"count": n, "refs": sorted(refs)}
    if not xy:
        return {"ok": False, "why": "the row is not drawn"}
    E.screen_click(*xy)
    t1, s = time.monotonic(), {}
    while time.monotonic() - t1 < secs:
        try:
            s = sel()
        except Exception as e:
            s = {"error": str(e)[:160]}
        if (want and want in s.get("refs", [])) or (not want and s.get("count", 0) > 0):
            return {"ok": True, "ms": int((time.monotonic() - t1) * 1000), "sel": s}
        time.sleep(0.005)
    return {"ok": False, "sel": s}


def hub_infos(text, prefix):
    """hub_rpc's INFO lines whose key starts with prefix: [(key, value)] in order (value parsed when it is JSON)."""
    out = []
    for l in text.splitlines():
        if not l.startswith("INFO "):
            continue
        try:
            o = json.loads(l[5:])
        except ValueError:
            continue
        if str(o.get("key", "")).startswith(prefix):
            v = o.get("value")
            if isinstance(v, str):
                try:
                    v = json.loads(v)
                except ValueError:
                    pass
            out.append((o["key"], v if v is not None else o.get("raw")))
    return out


def standin_ask(h, name, args, tag):
    """The Linux suites' ask(): one Backplane MCP call made by the stand-in agent on thread t1 ("mcpcall:"), its MCPCALL line."""
    def n_calls():
        return sum(1 for l in log_lines(h, "standin.log") if l.startswith("MCPCALL"))
    before = n_calls()
    text = "mcpcall:" + json.dumps({"name": name, "arguments": args})
    out = h.hub_rpc([{"method": "turn.start", "params": {"thread": "t1", "text": text, "msg": f"mchk-{tag}-{time.time_ns()}"}}])
    with open(h.p("rpc.log"), "a", encoding="utf-8") as f:
        f.write(out)
    end = time.monotonic() + 120
    while time.monotonic() < end and n_calls() <= before:
        time.sleep(0.25)
    time.sleep(2)
    return next((l for l in reversed(log_lines(h, "standin.log")) if l.startswith(f"MCPCALL {name} ")), "")


def find_markers(v):
    """K5's find(): the "markers" list anywhere in an MCP answer (nested objects, or JSON inside a string)."""
    if isinstance(v, dict):
        if "markers" in v:
            return v["markers"]
        for x in v.values():
            r = find_markers(x)
            if r is not None:
                return r
    if isinstance(v, str) and "markers" in v:
        try:
            return find_markers(json.loads(v))
        except ValueError:
            return None
    if isinstance(v, list):
        for x in v:
            r = find_markers(x)
            if r is not None:
                return r
    return None


def checks_list(h, pro, source):
    """kicad.checks.list, every page: the findings of one check."""
    out, off = [], 0
    while True:
        r = h.hub_rpc([{"method": "kicad.checks.list", "params": {"project": pro, "source": source, "offset": off, "limit": 25}}])
        line = next((l for l in r.splitlines() if l.startswith("{")), "{}")
        try:
            res = json.loads(line).get("result") or {}
        except ValueError:
            res = {}
        fs = res.get("findings") or []
        out += fs
        off += len(fs)
        if not fs or off >= res.get("total", 0):
            return out

# ---- plan: the person's Approve in KiCad's plan view (port of embed_plan_person_e2e.sh, the Linux suite) ----
# Q1 the agent's plan (wide corridors c-p, c-q on F.Cu either side of the board's centre, c-r below, a B.Cu plane) is in KiCad,
#    awaiting approval; plan_approve only asks the person
# Q2 in plan view the person clicks c-p and presses A (approve), L (lock), W (wider); clicks c-q and presses R (reject)
# Q3 plan_get `changes`: c-p approved, locked, edited by the person; c-q rejected
# Q4 the Plan panel (mgmt.plan) shows c-p approved and locked, c-q rejected, live; its keys note matches plan-keys
# Q5 the agent that asked is woken with the person's decision
# Q6 layer approval: KiCad's right-click "Approve Layer" (B.Cu), an agent's RunAction approveLayer / ApprovePlan refused (hub and,
#    from bp.13 batch 4, KiCad itself), its plan_approve only asks, the Plan panel's Approve (plan.approve) approves F.Cu
# Q7 (bp.13 batch 4, rules-insert-position) a click on an F.Cu corridor with B.Cu active selects it, W widens; W with nothing selected
#    changes nothing
# Q9 (plan-view-api) plan view passive to the API: SetActiveLayer, layerNext, togglePlanView done in plan view, the mode read from KiCad
# Q8 Escape with nothing selected: bp.13 leaves plan view and the hotkey starts it again; bp.12.x ends the plan tool (board colours
#    back, plan fainter, A is KiCad's Add Footprint), the hotkey makes A the plan's again
# Env: BP_PLAN_VIA=key (plan view by the hotkey, no canvas click first; default RunAction togglePlanView), BP_PLAN_KEYS=0 (decide from
# the Plan panel, no keys in KiCad), BP_PLAN_FORCE=1 (run without plan-keys), BP_PLAN_XLAYER / BP_PLAN_PASSIVE / BP_PLAN_ESC=expect|xfail.

PLAN_DOC = {
    "layers": [{"layer": "F.Cu", "role": "signal", "status": "proposed"}, {"layer": "B.Cu", "role": "plane", "status": "proposed"}],
    "groups": [{"id": "g-k", "name": "Cathodes", "color": "#e8a33d", "nets": ["Net-(U1A-K)", "Net-(U1B-K)"], "status": "proposed"},
               {"id": "g-r", "name": "Spare", "color": "#3de86b", "nets": ["Net-(U1A-G)"], "status": "proposed"}],
    "corridors": [{"id": "c-p", "group": "g-k", "layer": "F.Cu", "order": 1, "path": [[122, 113.3], [140, 113.3]], "width": 12.0, "status": "proposed"},
                  {"id": "c-q", "group": "g-k", "layer": "F.Cu", "order": 2, "path": [[155, 113.3], [173, 113.3]], "width": 12.0, "status": "proposed"},
                  {"id": "c-r", "group": "g-r", "layer": "F.Cu", "order": 3, "path": [[136, 129], [159, 129]], "width": 3.0, "status": "proposed"}],
    "planes": [{"id": "p-gnd", "net": "GND", "layer": "B.Cu", "outline": [[[122, 91], [173, 91], [173, 108], [122, 108]]], "priority": 1, "status": "proposed"}],
}
PLAN_ENV = {"BP_STREAM_N": "1", "BP_STREAM_MS": "10", "BP_STANDIN_MCPCALL_MAX": "200000"}


def plan_ask(h, name, args):
    """One Backplane MCP call made by the stand-in agent of thread t1 (a turn "mcpcall:{json}"): its MCPCALL line from standin.log."""
    def calls():
        try:
            return [l.rstrip("\r\n") for l in open(h.p("standin.log"), encoding="utf-8", errors="replace") if l.startswith("MCPCALL")]
        except OSError:
            return []
    before = len(calls())
    text = "mcpcall:" + json.dumps({"name": name, "arguments": args})
    out = h.hub_rpc([{"method": "turn.start", "params": {"thread": "t1", "text": text, "msg": f"mperson{name}-{time.time_ns()}"}}])
    with open(h.p("rpc.log"), "a", encoding="utf-8") as f:
        f.write(out or "")
    end = time.monotonic() + 60
    while time.monotonic() < end and len(calls()) <= before:
        time.sleep(0.25)
    time.sleep(2)
    return next((l for l in reversed(calls()) if l.startswith(f"MCPCALL {name} ")), "")


def plan_payload(line):
    """The tool's own answer inside an MCPCALL line (JSON-RPC result -> content[0].text, decoded), or {}."""
    try:
        o = json.loads(line.split(" ", 2)[2])
        return json.loads(o["result"]["content"][0]["text"])
    except (IndexError, KeyError, TypeError, ValueError):
        return {}


def plan_layer_status(h, layer):
    """A layer's status from plan_get: the plan's layers list (corridors name layers too, so found by shape, not a regex)."""
    def walk(o):
        if isinstance(o, str):
            try:
                o = json.loads(o)
            except ValueError:
                return None
            return walk(o) if isinstance(o, (dict, list)) else None
        if isinstance(o, dict):
            ls = o.get("layers")
            if isinstance(ls, list) and ls and all(isinstance(l, dict) and "layer" in l and "id" not in l for l in ls):
                return ls
            o = list(o.values())
        if isinstance(o, list):
            for v in o:
                r = walk(v)
                if r:
                    return r
        return None
    line = plan_ask(h, "plan_get", {})
    try:
        ls = walk(json.loads(line.split(" ", 2)[2])) or []
    except (IndexError, ValueError):
        ls = []
    return next((l.get("status", "?") for l in ls if l.get("layer") == layer), "?")


def plan_field(h, what):
    """plan_get: the plan's revision ("rev") or a corridor's width, "?" when unreadable."""
    r = plan_payload(plan_ask(h, "plan_get", {}))
    if not r:
        return "?"
    if what == "rev":
        return str((r.get("summary") or {}).get("revision", "?"))
    return str(next((c.get("width") for c in (r.get("plan") or {}).get("corridors", []) if c.get("id") == what), "?"))


def plan_wider(a, b):
    try:
        return a != "?" and b != "?" and float(b) > float(a) + 1e-9
    except ValueError:
        return False


def plan_info(h, pro):
    """The Plan panel's data (mgmt.watch -> mgmt.plan), as the client gets it."""
    f = {}
    for l in h.hub_rpc([{"method": "mgmt.watch", "params": {"project": pro}}]).splitlines():
        try:
            o = json.loads(l)
        except ValueError:
            continue
        if isinstance(o, dict):
            f = ((o.get("result") or {}).get("data") or {}).get("plan", f)
    return f if isinstance(f, dict) else {}


def plan_dialogs(h):
    """KiCad's dialogs now, the client's last word (its kicad_info lines), with any visible KiCad dialog top-level beside it."""
    last = []
    try:
        for l in open(h.p("client.log"), encoding="utf-8", errors="replace"):
            if '"kicad_info"' in l and '"dialogs"' in l:
                try:
                    last = json.loads(l[l.index("{"):]).get("dialogs") or []
                except ValueError:
                    pass
    except OSError:
        pass
    native = [w["title"] for w in kicad_toplevels() if w["visible"] and w["class"] in ("#32770", "wxWindowNR")
              and "Editor" not in w["title"] and "KiCad" not in w["title"] and w["title"]]
    return "; ".join(list(last) + [t for t in native if t not in last])


def plan_grab(h, name, box):
    """A screenshot of `box` (screen x, y, w, h), saved under shots/; the PIL image."""
    from PIL import Image
    x, y, w, hh = box
    path = h.p("shots", f"{name}.png")
    E.grab_screen(path, bbox=(x, y, x + w, y + hh))
    return Image.open(path).convert("RGB")


def _orange(r, g, b):
    # the group's #e8a33d as KiCad draws it (the Linux suite's mask)
    return r > 120 and r - b > 70 and g > 60 and g < r - 10 and g > b + 20


def _green(r, g, b):
    return g > 150 and g - r > 60 and g - b > 30


def plan_spots(img, ox, oy):
    """c-p and c-q on the screen: the corridor colour's pixels in the canvas image, its columns split into runs (gaps <= 30 px merged),
    the two biggest kept in x order; each the median of its pixels, in screen coordinates. [] when not two."""
    w, hh = img.size
    raw = img.tobytes()
    cols = [0] * w
    ys = [[] for _ in range(w)]
    for y in range(4, hh - 4):
        row = y * w * 3
        for x in range(4, w - 4):
            i = row + x * 3
            if _orange(raw[i], raw[i + 1], raw[i + 2]):
                cols[x] += 1
                ys[x].append(y)
    if sum(cols) < 50:
        return []
    runs, start = [], None
    for x, v in enumerate([c > 3 for c in cols] + [False]):
        if v and start is None:
            start = x
        if not v and start is not None:
            runs.append((start, x))
            start = None
    merged = []
    for r0, r1 in runs:
        if merged and r0 - merged[-1][1] <= 30:
            merged[-1] = (merged[-1][0], r1)
        else:
            merged.append((r0, r1))
    runs = sorted(sorted(merged, key=lambda r: -sum(cols[r[0]:r[1]]))[:2])
    if len(runs) < 2:
        return []
    out = []
    for x0, x1 in runs:
        xs_ = sorted(x for x in range(x0, x1) for _ in range(cols[x]))
        ys_ = sorted(y for x in range(x0, x1) for y in ys[x])
        out.append((xs_[len(xs_) // 2] + ox, ys_[len(ys_) // 2] + oy))
    return out


def plan_px(img):
    """The plan's pixels in the canvas image: the corridors' orange and green."""
    raw = img.tobytes()
    return sum(1 for i in range(0, len(raw), 3) if _orange(raw[i], raw[i + 1], raw[i + 2]) or _green(raw[i], raw[i + 1], raw[i + 2]))


def plan_lum(img):
    raw = img.tobytes()
    return int(sum(raw) / max(1, len(raw)))


def plan_diff(a, b, thr=60):
    from PIL import ImageChops
    raw = ImageChops.difference(a, b).tobytes()
    return sum(1 for i in range(0, len(raw), 3) if raw[i] + raw[i + 1] + raw[i + 2] > thr)


def plan_canvas(frame):
    """KiCad's GL canvas in the shown frame (on Windows a child window of its own): its screen box, or None."""
    gls = [w for w in E.list_child_windows(frame) if w["visible"] and w["class"] == "wxGLCanvas"]
    gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
    return (int(gl["root_x"]), int(gl["root_y"]), int(gl["w"]), int(gl["h"])) if gl else None


def plan_check(h, cp, frame, rep, timeline):
    top = int(rep.get("toplevel") or 0) or E.find_main_window(cp.pid)
    proj = h.p("proj")
    pcb, pro = os.path.join(proj, "ecc83-pp.kicad_pcb"), os.path.join(proj, "ecc83-pp.kicad_pro")
    bindir = os.path.join(h.kicad_dir, "bin")
    kiface = b""
    for name in ("_pcbnew.kiface", "_pcbnew.dll"):
        try:
            with open(os.path.join(bindir, name), "rb") as f:
                kiface = f.read()
            break
        except OSError:
            pass
    try:
        manifest = open(os.path.join(h.kicad_dir, "manifest.json"), encoding="utf-8", errors="replace").read()
    except OSError:
        manifest = ""
    sock = rep.get("sock") or ""
    try:
        caps = h.ipc_cmd("caps", sock).get("capabilities") or [] if sock else []
    except Exception:  # noqa: BLE001
        caps = []
    has = lambda cap: f'"{cap}"' in manifest or cap in caps  # noqa: E731
    edits = b"planAccept" in kiface or b"Plan.accept" in kiface
    keys_env = os.environ.get("BP_PLAN_KEYS", "1") != "0"
    plan_keys = has("plan-keys")
    rt = {"kiface": bool(kiface), "plan_view_edits": edits, "plan-keys": plan_keys, "plan-view-api": has("plan-view-api"),
          "rules-insert-position": has("rules-insert-position"), "sock": sock}
    print("DIAG plan_runtime " + json.dumps(rt), flush=True)
    timeline.append({"stage": "plan_runtime", **rt})
    if not h.check("plan: this KiCad has plan view edits (pcbnew.Plan.accept, bp.13+)", edits, rt):
        return
    if keys_env and not plan_keys and os.environ.get("BP_PLAN_FORCE") != "1":
        # bp.12.1 registered the plan tool as pcbnew.PlanTool: A opened Add Footprint (Linux: Nov 13)
        h.check("plan: the runtime has plan-keys (A, R, L, W are the plan view's; BP_PLAN_KEYS=0 decides from the Plan panel)", False, rt)
        return

    # the editor full size, as on Linux (the pane's Full control), then its canvas
    full = h.click_control(top, "kicad:full")
    time.sleep(2.5)
    fx, fy, fw, fh = window_box(frame)
    cv = plan_canvas(frame)
    if not cv:
        h.shot("plan_no_canvas")
        h.check("plan: the board is open in KiCad's editor (its GL canvas found)", False, {"frame": frame, "box": [fx, fy, fw, fh]})
        return
    gx, gy, gw, gh = cv
    h.check("plan: the board editor is shown full size", fw > 700, {"full_clicked": full, "frame": [fx, fy, fw, fh], "canvas": list(cv)})

    # Q1
    r1 = plan_ask(h, "plan_propose", {"plan": PLAN_DOC, "mode": "merge"})
    print("INFO plan_propose " + r1[:200], flush=True)
    r2 = plan_ask(h, "plan_get", {})
    h.check("plan: the agent's plan is in KiCad, awaiting approval", "c-q" in r2 and "awaiting_approval" in r2, r2[:300])
    r3 = plan_ask(h, "plan_approve", {"ids": ["c-p", "c-q"], "note": "the two cathode corridors"})
    h.check("plan: plan_approve only asks the person", "asked" in r3, r3[:300])
    time.sleep(3)

    hotkey_name = "Ctrl+Shift+R" if b"Routing Plan View" in kiface else "Alt+P"

    def hotkey():
        if hotkey_name == "Ctrl+Shift+R":
            E.key_chord("ctrl", "shift", "r")
        else:
            E.key_chord("alt", "p")
        time.sleep(0.7)

    def key(name, wait=0.7):
        E.key_tap(name)
        time.sleep(wait)

    def click(x, y, button=1, wait=0.7):
        E.screen_click(int(x), int(y), button)
        time.sleep(wait)

    def step_ok(what):
        """A step that types into KiCad starts only with no KiCad dialog open; with one it fails, naming it, and types nothing."""
        d = plan_dialogs(h)
        if not d:
            return True
        h.check(f"plan {what}: no KiCad dialog is open when it starts", False, f"KiCad shows: {d}")
        return False

    def kicad_ipc(type_, message):
        return plan_ask(h, "kicad_ipc", {"path": pcb, "type": type_, "message": message})

    def panel(params):
        out = h.hub_rpc([{"method": "plan.approve", "params": {"pro": pro, **params}}])
        return "\n".join([l for l in out.splitlines() if not l.startswith("INFO")][-2:])

    # plan view, as the Linux suite reaches it
    if os.environ.get("BP_PLAN_VIA", "action") == "key":
        hotkey()  # straight after the editor opened: no canvas click first
        via = f"{hotkey_name} (pressed without a canvas click)"
    else:
        # the keyboard to KiCad: a click on the canvas's centre (the board's: nothing of the plan there), then Escape
        click(gx + gw // 2, gy + gh // 2)
        key("Escape")
        ra = kicad_ipc("kiapi.common.commands.RunAction", {"action": "pcbnew.Plan.togglePlanView"})
        print("INFO RunAction togglePlanView: " + (re.search(r"RAS_[A-Z_]*", ra) or [""])[0], flush=True)
        if "RAS_OK" in ra:
            via = "action"
        else:
            via = hotkey_name
            hotkey()
    print(f"INFO plan view by {via}", flush=True)
    time.sleep(1.5)
    key("Home", 1.5)  # zoom to fit
    img = plan_grab(h, "q1_plan_view", cv)
    spots = plan_spots(img, gx, gy)
    (px_, py_), (qx_, qy_) = spots if len(spots) == 2 else ((0, 0), (0, 0))
    print(f"INFO corridors on screen: c-p ({px_},{py_}) c-q ({qx_},{qy_})", flush=True)
    timeline.append({"stage": "plan_view", "via": via, "c_p": [px_, py_], "c_q": [qx_, qy_]})
    if not h.check("plan: both corridors are drawn in KiCad's plan view", px_ > 0 and qx_ > px_, "see shots/q1_plan_view.png"):
        return
    s = max(1, (qx_ - px_) // 33)  # screen px per mm: c-p's middle at x 131, c-q's at 164
    off_x = max(gx + 30, px_ - 22 * s)  # left of the board: nothing of the plan there

    # Q2: the person's Approve (A), lock (L), wider (W) on c-p; reject (R) on c-q
    if keys_env:
        if step_ok("Q2 A, L, W on c-p"):
            click(px_, py_)
            key("a")
        if step_ok("Q2 L after A"):
            key("l")
        if step_ok("Q2 W after L"):
            key("w")
        time.sleep(1)
        plan_grab(h, "q2_cp_done", cv)
        if step_ok("Q3 R on c-q"):
            click(qx_, qy_)
            key("r")
        time.sleep(2)
        plan_grab(h, "q3_cq_rejected", cv)
    else:
        print("INFO panel approve c-p: " + panel({"ids": ["c-p"], "approve": True})[:300], flush=True)
        print("INFO panel reject c-q: " + panel({"ids": ["c-q"], "approve": False})[:300], flush=True)
        time.sleep(2)

    # Q3: what Approve led to, in the agent's plan_get
    r4 = plan_ask(h, "plan_get", {})
    j = r4[len("MCPCALL plan_get "):]
    h.check("plan: plan_get: the person approved c-p in KiCad", "c-p" in j and "approved" in j, r4[:300])
    if keys_env:
        h.check("plan: plan_get: the person locked c-p",
                bool(re.search(r"c-p[^}]*locked", j)), r4[:300])
        h.check("plan: plan_get: c-p edited by the person (wider)",
                bool(re.search(r"c-p[^}]*edited by the person", j) or re.search(r"constraint[^}]*wide", j)), r4[:300])
    h.check("plan: plan_get: the person rejected c-q", "rejected" in j and "c-q" in j, r4[:300])

    # Q4: the Plan panel, live
    pinfo = plan_info(h, pro)
    items = {i.get("id"): i for l in pinfo.get("layers", []) for i in (l.get("items") or [])}
    p, q = items.get("c-p", {}), items.get("c-q", {})
    h.check("plan: the Plan panel shows c-p approved and locked, c-q rejected (live)",
            p.get("state") == "approved" and (p.get("locked") is True or not keys_env) and q.get("state") == "rejected",
            json.dumps(pinfo)[:400])
    kn = pinfo.get("keys_note") or ""
    if not plan_keys:
        h.check("plan: the Plan panel says to approve there (no plan-keys)", "approve from this panel" in kn, f"keys_note: {kn}")
    else:
        h.check("plan: the Plan panel has no keys note (plan-keys)", not kn, f"keys_note: {kn}")

    # Q5: the agent that asked is woken
    def standin():
        try:
            return open(h.p("standin.log"), encoding="utf-8", errors="replace").read()
        except OSError:
            return ""
    end = time.monotonic() + 30
    while time.monotonic() < end and "The person judged" not in standin():
        time.sleep(0.5)
    woke = re.findall(r'The person judged[^"\n]*', standin())
    h.check("plan: the agent that asked is woken with the person's decision in KiCad",
            bool(re.search(r"The person judged.*c-p", standin())), woke[:2])
    h.shot("q4_panel")

    # Q6: layer approval
    if b"pcbnew.Plan.approveLayer" in kiface:
        key("Escape", 0.5)
        key("Page_Down", 1)  # B.Cu active
        if not keys_env:
            print("INFO Q6 right-click Approve Layer not tried: without plan-keys the menu is the selection tool's", flush=True)
        elif step_ok("Q6 right-click Approve Layer"):
            # nothing selected (off the board), then the menu by keyboard: Cancel, Add Note, Draw Corridor, Draw Plane Region, Approve Layer
            click(off_x, py_, 3, 1)
            h.shot("q5_menu")
            for _ in range(5):
                key("Down", 0.15)
            time.sleep(0.5)
            h.shot("q5_menu_approve")
            key("Return", 2)
            h.shot("q5_layer_b")
        lb = plan_layer_status(h, "B.Cu")
        if keys_env:
            h.check("plan: layer approval from KiCad's right-click menu (Approve Layer on B.Cu)", lb == "approved", f"B.Cu: {lb} (shots/q5_menu_approve.png)")
        key("Page_Up", 1)  # F.Cu stays the person's
        ra = kicad_ipc("kiapi.common.commands.RunAction", {"action": "pcbnew.Plan.approveLayer"})
        h.check("plan: an agent's RunAction pcbnew.Plan.approveLayer is refused", "the person's alone" in ra, ra[:200])
        rb = kicad_ipc("kiapi.backplane.plan.ApprovePlan", {"board": {"type": "DOCTYPE_PCB"}, "layer": "F.Cu", "approve": True})
        h.check("plan: an agent's ApprovePlan is refused", "the person's alone" in rb, rb[:200])
        # past the hub's guard, straight to KiCad as an agent's client: the fork refuses (bp.13 batch 4); earlier builds not sent
        b4 = "rules-insert-position" in kicad_ipc("kiapi.backplane.GetCapabilities", {})
        if b4 and sock:
            for act in ("pcbnew.Plan.approveLayer", "pcbnew.Plan.rejectLayer", "pcbnew.Plan.accept", "pcbnew.Plan.reject"):
                try:
                    a = __import__("subprocess").run([sys.executable, os.path.join(E.HERE, "lib", "agent_runaction.py"), sock, act, "agent:t1"],
                                                     capture_output=True, text=True, timeout=30)
                    out = ((a.stdout or "") + (a.stderr or "")).strip().splitlines()
                    a = out[-1] if out else ""
                except Exception as e:  # noqa: BLE001
                    a = f"error: {e}"
                h.check(f"plan: KiCad itself refuses an agent's RunAction {act}", "approval is the person's" in a, a[:200])
        else:
            print(f"XFAIL KiCad itself refusing an agent's approval actions: not this build (bp.13 batch 4), or no socket ({sock})", flush=True)
        plan_ask(h, "plan_approve", {"layer": "F.Cu", "note": "F.Cu"})
        time.sleep(2)
        lf0 = plan_layer_status(h, "F.Cu")
        h.check("plan: F.Cu is still not approved after the agent's attempts and its plan_approve", lf0 != "approved", f"F.Cu: {lf0}")
        step_ok("Q6 the Plan panel's Approve")
        pa = panel({"layer": "F.Cu", "approve": True})
        time.sleep(2)
        lf = plan_layer_status(h, "F.Cu")
        h.shot("q6_layers")
        h.check("plan: the Plan panel's Approve (ApprovePlan as the person) approves F.Cu in plan view", lf == "approved",
                f"F.Cu: {lf}; plan.approve: {pa[:200]}")
    else:
        print("SKIP layer approval: this KiCad has no pcbnew.Plan.approveLayer", flush=True)

    # Q7: the plan's other layers are hit (bp.13 batch 4)
    caps7 = kicad_ipc("kiapi.backplane.GetCapabilities", {})
    xl = os.environ.get("BP_PLAN_XLAYER") or ("expect" if "rules-insert-position" in caps7 else "xfail")
    board = {"type": "DOCTYPE_PCB", "board_filename": "ecc83-pp.kicad_pcb", "project": {"name": "ecc83-pp", "path": proj}}

    def active_layer():
        m = re.search(r"BL_[A-Za-z_]*", kicad_ipc("kiapi.board.commands.GetActiveLayer", {"board": board}))
        return m.group(0) if m else ""

    def xcheck(what, ok, ev):
        if xl == "expect" or ok:
            h.check(what, ok, ev)
        else:
            print(f"XFAIL {what} (this build's hit test sees only the active layer; fixed in bp.13 batch 4): {ev}", flush=True)

    key("Page_Down", 1)  # B.Cu active, nothing selected (no Escape: it would end the plan tool)
    rx, ry = round(px_ + (147.5 - 131) * s), round(py_ + (129 - 113.3) * s)  # c-r's middle; the B.Cu plane stops at y 108
    w0, l0 = plan_field(h, "c-r"), active_layer()
    print(f"INFO Q7 ({xl}): c-r at ({rx},{ry}), width {w0}, active {l0}", flush=True)
    click(rx, ry, wait=0.8)
    key("w", 1.5)
    plan_grab(h, "q7_cross_layer", cv)
    w1, l1 = plan_field(h, "c-r"), active_layer()
    xcheck("plan: with B.Cu active, a click on an F.Cu corridor and W widen it", plan_wider(w0, w1), f"c-r width {w0} -> {w1}")
    xcheck("plan: the click makes the corridor's layer (F.Cu) active", l1 == "BL_F_Cu", f"active {l0} -> {l1}")
    click(off_x, py_, wait=0.8)  # nothing selected
    rev1 = plan_field(h, "rev")
    band = (gx + 40, max(fy, gy - 10), max(1, gw - 80), 60)  # the infobar opens at the top of the canvas
    ib0 = plan_grab(h, "q7_before_w", band)
    key("w", 1.0)
    ib1 = plan_grab(h, "q7_after_w", band)
    rev2 = plan_field(h, "rev")
    bpx = plan_diff(ib0, ib1)
    xcheck("plan: W with nothing selected says so on KiCad's infobar", bpx > 1500, f"{bpx} pixels changed at the top of the canvas")
    h.check("plan: W with nothing selected changes nothing in the plan", rev1 != "?" and rev1 == rev2, f"revision {rev1} -> {rev2}")

    # Q9: plan view passive to the API (plan-view-api)
    def ipc_status(type_, message):
        o = kicad_ipc(type_, message)
        if '"isError":true' in o:
            m = re.search(r"AS_[A-Z_]*", o)
            return "REFUSED " + (m.group(0) if m else "")
        m = re.search(r"RAS_[A-Z_]*", o)
        if m and "RAS_OK" not in o:
            return "REFUSED " + m.group(0)
        return "OK"

    passive = os.environ.get("BP_PLAN_PASSIVE") or ("expect" if "plan-view-api" in caps7 else "xfail")

    def plan_mode():
        """KiCad's own word on plan view: GetPlan's in_plan_view (proto3 leaves a false bool out: off where the build has it)."""
        r = plan_payload(kicad_ipc("kiapi.backplane.plan.GetPlan", {"board": board, "derived": False}))
        if not r:
            return "?"
        m = r.get("message", r)
        v = m.get("in_plan_view") if isinstance(m, dict) else None
        if v is None and isinstance(m, dict):
            try:
                v = json.loads(m.get("json", "{}")).get("in_plan_view")
            except ValueError:
                v = None
        return ("off" if passive == "expect" else "?") if v is None else ("on" if v else "off")

    def pcheck(what, ok, ev):
        if passive == "expect" or ok:
            h.check(what, ok, ev)
        else:
            print(f"XFAIL {what} (plan view is busy to the API before bp.13 batch 11, plan-view-api): {ev}", flush=True)

    if passive != "expect":
        print(f"INFO Q9: the runtime lacks plan-view-api; its checks are told as XFAIL", flush=True)
    la, m0 = active_layer(), plan_mode()
    print(f"INFO Q9 ({passive}): plan view {m0}, active {la}", flush=True)
    s1 = ipc_status("kiapi.board.commands.SetActiveLayer", {"board": board, "layer": "BL_B_Cu"})
    l1 = active_layer()
    pcheck("plan: in plan view SetActiveLayer(B.Cu) is done", s1 == "OK" and l1 == "BL_B_Cu", f"{s1}, active {la} -> {l1}")
    if s1 != "OK":
        h.check("plan: a refused SetActiveLayer left the layer as it was", l1 == la, f"{s1}, active {la} -> {l1}")
    s2 = ipc_status("kiapi.common.commands.RunAction", {"action": "pcbnew.Control.layerNext"})
    l2 = active_layer()
    pcheck("plan: in plan view RunAction layerNext is done (the layer moves)", s2 == "OK" and l2 != l1, f"{s2}, active {l1} -> {l2}")
    if s2 != "OK":
        h.check("plan: a refused layerNext left the layer as it was", l2 == l1, f"{s2}, active {l1} -> {l2}")
    s3 = ipc_status("kiapi.board.commands.SetActiveLayer", {"board": board, "layer": "BL_F_Cu"})
    if s3 != "OK":
        key("Page_Up")
    time.sleep(0.5)
    w3 = plan_field(h, "c-r")
    click(rx, ry, wait=0.8)
    key("w", 1.5)
    w4 = plan_field(h, "c-r")
    pcheck("plan: after the API changed the layer, a click on that layer's corridor selects it (W widens it)", plan_wider(w3, w4), f"{s3}; c-r width {w3} -> {w4}")
    s4 = ipc_status("kiapi.common.commands.RunAction", {"action": "pcbnew.Plan.togglePlanView"})
    time.sleep(1)
    m4 = plan_mode()
    pcheck("plan: RunAction togglePlanView turns plan view off (KiCad says so)", s4 == "OK" and m4 == "off", f"{s4}, plan view {m0} -> {m4}")
    if s4 == "OK":
        click(gx + gw // 2, gy + gh // 2, wait=0.8)  # U1, the board's centre
        sel = kicad_ipc("kiapi.common.commands.GetSelection", {"header": {"document": board}})
        w5 = plan_field(h, "c-r")
        key("w", 1)
        w6 = plan_field(h, "c-r")
        kinds = sorted(set(re.findall(r"kiapi\.board\.types\.[A-Za-z]*", sel)))
        pcheck("plan: with plan view off a click selects a board item, not a corridor",
               ("FootprintInstance" in sel or "kiapi.board.types" in sel) and w5 == w6, f"selection: {' '.join(kinds)}; c-r {w5} -> {w6} on W")
        key("Escape", 0.3)
        s5 = ipc_status("kiapi.common.commands.RunAction", {"action": "pcbnew.Plan.togglePlanView"})
        time.sleep(1)
        m5 = plan_mode()
        pcheck("plan: RunAction togglePlanView turns plan view on again", s5 == "OK" and m5 == "on", f"{s5}, plan view {m4} -> {m5}")
        if m5 != "on":
            hotkey()
            time.sleep(0.5)
    else:
        h.check("plan: a refused togglePlanView left plan view as it was", m4 == m0, f"{s4}, plan view {m0} -> {m4}")

    # Q8: Escape with nothing selected
    click(off_x, py_, wait=0.6)  # nothing selected (Escape would end the tool)
    key("Page_Up", 0.8)  # F.Cu active again: its corridors drawn
    i0 = plan_grab(h, "q8_before_esc", cv)
    p0, lum0 = plan_px(i0), plan_lum(i0)
    if step_ok("Q8 Escape with nothing selected"):
        key("Escape")
    time.sleep(1.2)
    i1 = plan_grab(h, "q8_after_esc", cv)
    p1, lum1 = plan_px(i1), plan_lum(i1)
    if "rules-insert-position" in caps7:
        esc = os.environ.get("BP_PLAN_ESC", "xfail")
        print(f"INFO Q8 bp.13 ({esc}): plan pixels {p0} before Escape, {p1} after", flush=True)
        if esc == "expect" or p1 < p0 // 10 + 1:
            h.check("plan: Escape with nothing selected leaves plan view (the plan no longer drawn)", p0 > 500 and p1 < p0 // 10 + 1,
                    f"plan pixels {p0} -> {p1} (q8_before_esc, q8_after_esc)")
            hotkey()
            time.sleep(0.5)
            p2 = plan_px(plan_grab(h, "q8_again", cv))
            h.check(f"plan: {hotkey_name} starts plan view again", p2 > p0 // 2, f"plan pixels {p2}")
        else:
            print(f"XFAIL Escape with nothing selected leaves plan view (bp.13 batch 5): this build ends only the tool; the plan stays drawn ({p0} -> {p1})", flush=True)
    elif keys_env:
        print(f"INFO Q8 bp.12.x: plan pixels {p0} -> {p1}, canvas luminance {lum0} -> {lum1}", flush=True)
        h.check("plan: Escape with nothing selected brings the board's colours back (bp.12.x)", lum1 > lum0 * 6 // 5,
                f"canvas luminance {lum0} -> {lum1} (q8_before_esc, q8_after_esc)")
        h.check("plan: the plan stays drawn, fainter (bp.12.x)", 0 < p1 < p0 * 4 // 5, f"plan pixels {p0} -> {p1}")
        key("a")  # the plan tool has ended: A is KiCad's own (Add Footprint)
        dl = ""
        for _ in range(20):
            dl = plan_dialogs(h)
            if dl:
                break
            time.sleep(0.25)
        h.shot("q8_a_after_esc")
        h.check("plan: the plan tool has ended: A opens KiCad's Footprint Chooser (bp.12.x)", "footprint" in dl.lower(), f"KiCad shows: {dl or 'nothing'}")
        if dl:
            key("Escape")
            for _ in range(20):
                if not plan_dialogs(h):
                    break
                time.sleep(0.25)
        if step_ok("Q8 plan view again"):
            hotkey()
            time.sleep(0.5)
            key("a", 2)
            dl2 = plan_dialogs(h)
            h.shot("q8_again")
            h.check("plan: plan view again: A is the plan's (no KiCad dialog) (bp.12.x)", not dl2, f"KiCad shows: {dl2}")
            if dl2:
                key("Escape")


def run(h):
    only = os.environ.get("BP_OPENING_FOCUS_ONLY", "")
    only_click = only == "click_control"
    only_ctrl_k = only == "ctrl_k"
    h.init_dirs()
    seeded = None
    if only == "libtable":
        # the person's own global tables, from before Backplane's library: KiCad's nested table and a library of theirs, no Backplane entries
        cfg = h.p("fh", "AppData", "Roaming", "kicad", "10.99")
        os.makedirs(cfg, exist_ok=True)
        LIBTABLE_MINE["fp-lib-table"] = ('(fp_lib_table\n\t(version 7)\n\t(lib (name "KiCad")(type "Table")(uri "${KICAD10_TEMPLATE_DIR}/fp-lib-table")(options "")(descr "KiCad Default Libraries"))\n'
                                         '\t(lib (name "My_Footprints")(type "KiCad")(uri "C:/Users/me/kicad/My_Footprints.pretty")(options "")(descr "mine"))\n)\n')
        LIBTABLE_MINE["sym-lib-table"] = ('(sym_lib_table\n\t(version 7)\n\t(lib (name "KiCad")(type "Table")(uri "${KICAD10_TEMPLATE_DIR}/sym-lib-table")(options "")(descr "KiCad Default Libraries"))\n'
                                          '\t(lib (name "My_Symbols")(type "KiCad")(uri "C:/Users/me/kicad/My_Symbols.kicad_sym")(options "")(descr "mine"))\n)\n')
        for t, text in LIBTABLE_MINE.items():
            with open(os.path.join(cfg, t), "w", encoding="utf-8", newline="") as f:
                f.write(text)
    if only == "checks":
        seeded = checks_seed(h)
        # the hub logs each request it sends KiCad (RunChecks) and KiCad's events (ET_PROGRESS, ET_CHECKS_DONE); the stand-in's
        # MCPCALL lines whole (GetMarkers' answer is long). base_env copies os.environ into the hub and so into the stand-in.
        os.environ["BACKPLANE_KICAD_DEBUG"] = "1"
        os.environ["BP_STANDIN_MCPCALL_MAX"] = "200000"
    timeline = []
    try:
        provenance = {}
        for name, path in (("client", h.client_exe), ("hub", h.hub_exe), ("kicad", h.kicad_bin)):
            provenance[name] = {"path": path, "sha256": E.sha256_file(path)}
        with open(h.p("binaries.json"), "w", encoding="utf-8") as f:
            json.dump(provenance, f, indent=2)
        if only == "savegate":
            # embed_load_migration_e2e.sh (savegate.rs): what KiCad rewrites on its own is never saved by the hub. With the project's
            # KiCad warmed by the shown thread (no Edit: the person never took it):
            #   (2) the Board Setup press on a board KiCad migrated on load (3D .wrl -> .step, teardrops rebuilt: runtimes without
            #       no-load-migration) is set in KiCad and not saved, saying what KiCad changed, the board byte-identical;
            #   (3) a BOM edit, a real change, is saved and says when KiCad upgraded the file format in the same save.
            # The seed is the harness's own (seed_project.py: ecc83-pp, 11 .wrl references, a 20241229 board and 20250114
            # schematic); KEMBED_PROJ_SRC works here as on Linux (seed_project.py reads it on any OS), e.g. a copy of the fork's
            # demos/complex_hierarchy for teardrops. Not ported: the Linux script's ask/asksave helpers (the stand-in agent's
            # kicad_ipc saves) are defined there but never called; that case is the hub's unit test an_agents_save_writes_only_real_changes.
            pros = []
            for root, dirs, files in os.walk(h.p("proj")):
                rel = os.path.relpath(root, h.p("proj"))
                depth = 0 if rel == "." else rel.count(os.sep) + 1
                dirs[:] = [d for d in dirs if d not in (".backplane", ".git")] if depth < 2 else []
                pros += [os.path.join(root, f) for f in files if f.endswith(".kicad_pro") and "mic_flex" not in f]
            pro = sorted(pros)[0] if pros else ""
            if not pro:
                h.check("savegate: the seeded project has a .kicad_pro", False, {"proj": h.p("proj")})
                return h.finish()
            pcb, kdir = pro[: -len(".kicad_pro")] + ".kicad_pcb", os.path.dirname(pro)
            # a Board Setup suggestion to press: the clearance minimum under the fab's (committed, so the board is the same as git's)
            with open(pro, encoding="utf-8") as f:
                pj = json.load(f)
            pj.setdefault("board", {}).setdefault("design_settings", {}).setdefault("rules", {})["min_clearance"] = 0.0
            with open(pro, "w", encoding="utf-8", newline="\n") as f:
                json.dump(pj, f, indent=2)
            import subprocess
            subprocess.run(["git", "-C", h.p("proj"), "commit", "-qam", "clearance under the fab's"], capture_output=True)

            def sha(path, n=16):
                with open(path, "rb") as f:
                    return hashlib.sha256(f.read()).hexdigest()[:n]

            def counts(path):
                t = open(path, encoding="utf-8", errors="ignore").read()
                return t.count(".wrl"), t.count(".step") + t.count(".STEP"), t.count("(teardrop")

            def schs():
                return " ".join(sha(p, 12) for p in sorted(glob.glob(os.path.join(kdir, "*.kicad_sch"))))

            def rpc_line(out, method):
                return next((l for l in out.splitlines() if l.startswith('{"m":"%s"' % method)), "")

            w0, s0, t0 = counts(pcb)
            h0, sh0 = sha(pcb), schs()
            print("INFO board before: " + json.dumps({"wrl": w0, "step": s0, "teardrops": t0, "sha": h0, "pro": pro}), flush=True)
            h.start_hub(embed_mode="fork")
            cp = h.start_client([{"select": 0}, {"wait": 2000}, {"click": "rail:Board"}, {"wait": 2500}, {"mark": "shown"},
                                 {"wait": 90000}, {"mark": "done"}], hold=True, exit_after=300)
            # the project's KiCad warmed by the shown thread, its board loaded (the hub logs the same line on Windows)
            warm_ms, t_warm = None, time.monotonic()
            while time.monotonic() - t_warm < 120 and cp.poll() is None:
                try:
                    if re.search(r"kicad embed: start \+\d+ms ready", open(h.p("hub.log"), encoding="utf-8", errors="replace").read()):
                        warm_ms = round((time.monotonic() - t_warm) * 1000)
                        break
                except OSError:
                    pass
                time.sleep(0.1)
            time.sleep(4)
            running = re.findall(r"kicad embed: running \S*", open(h.p("hub.log"), encoding="utf-8", errors="replace").read())
            print("INFO KiCad: " + json.dumps({"warm_ms": warm_ms, "running": running[-1:]}), flush=True)
            # (2) the Board Setup press
            ap = rpc_line(h.hub_rpc([{"method": "fab.profile", "params": {"pro": pro, "action": "apply", "key": "min_clearance"}, "timeout_ms": 45000}]), "fab.profile")
            print("INFO Board Setup press: " + ap[:500], flush=True)
            time.sleep(2)
            w1, s1, t1 = counts(pcb)
            h1 = sha(pcb)
            print("INFO board after: " + json.dumps({"wrl": w1, "step": s1, "teardrops": t1, "sha": h1}), flush=True)
            m = re.search(r'KiCad changed the board when it opened it[^"]*', ap)
            mig = m.group(0) if m else ""
            print("INFO " + (f"KiCad migrated on load, said to the person: {mig}" if mig else "no load migration said on this runtime"), flush=True)
            h.check("savegate: the board's 3D references are as before (no .wrl -> .step written)", w1 == w0 and s1 == s0, {"before": [w0, s0], "after": [w1, s1]})
            h.check("savegate: the board's teardrops are as before", t1 == t0, {"before": t0, "after": t1})
            try:
                saved = json.loads(ap)["result"].get("saved")
            except (ValueError, KeyError, TypeError, AttributeError):
                saved = "?"
            h.check("savegate: the Board Setup press saved only a board KiCad did not change on load",
                    (bool(mig) and saved is False and h1 == h0) or (not mig and saved is True),
                    {"saved": saved, "migration_said": bool(mig), "board_same": h1 == h0, "warm_ms": warm_ms})
            h.check("savegate: the Board Setup press leaves the schematics alone", schs() == sh0, {"before": sh0, "after": schs()})
            # (3) a BOM edit: a real change, saved (KiCad not on screen), saying when the format was upgraded with it
            ref = ""
            for p in sorted(glob.glob(os.path.join(kdir, "*.kicad_sch"))):
                m = re.search(r'\(property "Reference" "([A-Z]+[0-9]+)"', open(p, encoding="utf-8", errors="ignore").read())
                if m:
                    ref = m.group(1)
                    break
            be = rpc_line(h.hub_rpc([{"method": "mgmt.bom.edit", "params": {"project": pro, "field": "mpn", "refs": [ref], "value": "E2E-SAVEGATE"}, "timeout_ms": 45000}]), "mgmt.bom.edit")
            print(f"INFO BOM edit ({ref}): " + be[:400], flush=True)
            time.sleep(2)
            bom_saved = any("E2E-SAVEGATE" in open(p, encoding="utf-8", errors="ignore").read() for p in glob.glob(os.path.join(kdir, "*.kicad_sch")))
            h.check("savegate: the BOM edit is saved", bom_saved, {"ref": ref, "reply": be[:200]})
            m = re.search(r'KiCad also upgraded the file format[^"]*', be)
            print("INFO BOM edit's format line: " + (m.group(0) if m else "none (the files were current)"), flush=True)
            h.check("savegate: the board is untouched by the BOM edit", sha(pcb) == h1, {"before": h1, "after": sha(pcb)})
            return h.finish()
        if only == "plan":
            os.environ.update(PLAN_ENV)  # the stand-in's MCP answers whole in standin.log, as on Linux
        h.start_hub(embed_mode="fork")
        cp = h.start_client([
            {"select": 0}, {"wait": 2800}, {"click": "rail:Board"},
            {"wait": 2800}, {"mark": "ready_for_edit"}, {"wait": 1800},
            {"kicad": {"wait_painted": 45000, "kind": "PCB", "tag": "first_paint"}},
            {"wait": 2000}, {"kicad": {"report": True, "tag": "settled"}},
            {"mark": "opening_settled"},
        ], hold=True, exit_after=900 if only == "plan" else 600)  # a safety net only (cleanup ends it): mouse_return x5 with a 40 s pre-warm wait outlived 120 s (MS)
        started = time.monotonic()
        previous = None
        settled = False
        clicked_edit = False
        ready_seen = 0.0
        box_checked = 0.0
        at_request = None
        trace_idle = os.environ.get("BP_OPENING_FOCUS_ONLY") == "idle_trace"
        t_edit, t_prewarm, last_log_poll = 0.0, 0.0, 0.0
        # idle_hotkey: KiCad suspended from 0.5 s after its first paint (before the client's pre-warm request at ~2 s)
        stall_s = float(os.environ.get("BP_PREWARM_STALL_S", "0") or 0) if os.environ.get("BP_OPENING_FOCUS_ONLY") == "idle_hotkey" else 0
        stall_started, stall_pid = None, 0
        # BP_STARTUP_BOX_OBSERVE=1 (the bp.10 self-lock rate, Nov 15): a box before the first frame is recorded, not the end
        # of the run: when it came, what it says, whether it went away, and whether the editor came up (the hub's retry)
        observe = os.environ.get("BP_STARTUP_BOX_OBSERVE") == "1"
        box_obs = {}
        box_since = None
        while cp.poll() is None and time.monotonic() - started < (100 if observe else 65):
            snap = focus_snapshot()
            if snap != previous:
                timeline.append({"ms": round((time.monotonic() - started) * 1000), **snap})
                previous = snap
            if not at_request:
                at_request = request_clauses(h)
                if at_request:
                    timeline.append({"stage": "guard_clauses_at_request", "ms": round((time.monotonic() - started) * 1000), **at_request})
                    print("DIAG guard_at_request " + json.dumps(timeline[-1]), flush=True)
            if stall_s and not stall_started:
                fp = [m for m in h.metrics() if m.get("event") == "kicad" and m.get("tag") == "first_paint" and m.get("pid")]
                if fp:
                    stall_pid = int(fp[0]["pid"])
                    time.sleep(0.5)
                    stall_started = E.suspend_pid(stall_pid, True) and time.monotonic()
                    timeline.append({"stage": "stall_started", "pid": stall_pid, "ok": bool(stall_started)})
            if stall_started and time.monotonic() - stall_started >= stall_s:
                E.suspend_pid(stall_pid, False)
                timeline.append({"stage": "stall_ended", "s": stall_s})
                stall_started, stall_s = None, 0
            ms_ = h.metrics()
            marks = {m.get("name") for m in ms_ if m.get("event") == "mark"}
            # the board view drawn first: Edit pressed before it is a press on a view still loading and does nothing (b14-base-open:
            # bp-win's first plot after its Windows update took 15 s, painted at 18.4 s, the press went at 6.3 s; MO1 painted at 3.2 s)
            painted = any(m.get("event") == "plot_painted" for m in ms_)
            if "ready_for_edit" in marks and not painted and not ready_seen:
                ready_seen = time.monotonic()
            if trace_idle and t_edit and not t_prewarm and time.monotonic() - last_log_poll > 0.2:
                last_log_poll = time.monotonic()
                try:
                    if "editor.kicad.prewarm" in open(h.p("client.log"), encoding="utf-8", errors="replace").read():
                        t_prewarm = time.monotonic()
                        timeline.append({"stage": "prewarm_requested", "ms": round((t_prewarm - started) * 1000)})
                except OSError:
                    pass
            # BP_EDIT_EARLY=1: Edit pressed as soon as it is drawn, before the viewer may know its file (Lane B's 33cd306c2 keeps such
            # a press and opens the editor once the key arrives; the client logs "kicad_edit":"pending" with the key it saw)
            early = os.environ.get("BP_EDIT_EARLY") == "1" and not clicked_edit and bool(h.controls().get("ui:Edit"))
            if early:
                timeline.append({"stage": "edit_early", "painted": painted, "ready_for_edit": "ready_for_edit" in marks})
            if not clicked_edit and (early or ("ready_for_edit" in marks and (painted or (ready_seen and time.monotonic() - ready_seen > 40)))):
                if ready_seen:
                    timeline.append({"stage": "edit_waited_for_plot", "ms": round((time.monotonic() - ready_seen) * 1000), "painted": painted})
                clicked_edit = h.click_control(E.find_main_window(cp.pid), "ui:Edit")
                timeline.append({"stage": "native_edit_click", "clicked": clicked_edit, **focus_snapshot()})
                t_edit = time.monotonic()
            # a box up before the first frame is shown (first_paint) is the failure, named, not a timeout later
            if clicked_edit and "first_paint" not in {r for r in h.kicad_reports()} and time.monotonic() - box_checked > 1.0:
                box_checked = time.monotonic()
                box = E.startup_box()
                # a box that waits for someone fails the start; a progress window that goes by itself does not (bp.10's
                # "Load PCB / Finalizing board / Elapsed time", gone in 1.1-1.3 s, 4 of 10 opens Nov 15): failed once up 3 s
                box_since = (box_since or time.monotonic()) if box else None
                if box and not observe and time.monotonic() - box_since >= 3.0:
                    print("DIAG startup_box " + json.dumps(box), flush=True)
                    h.shot("startup_box")
                    h.check(f"KiCad starts without a box before its first frame (it showed {box['title']!r}: {' | '.join(box['text'])[:200]})", False, box)
                    return h.finish()
                if box and "first_ms" not in box_obs:
                    box_obs.update(first_ms=round((time.monotonic() - started) * 1000), box=box)
                    h.shot("startup_box")
                elif box_obs and not box and "gone_ms" not in box_obs:
                    box_obs["gone_ms"] = round((time.monotonic() - started) * 1000)
            if "opening_settled" in marks:
                settled = True
                break
            time.sleep(0.02 if trace_idle else 0.05)
        if observe:
            fp = h.kicad_reports().get("first_paint") or {}
            box_obs.update(settled=settled, first_paint_state=fp.get("state"), first_paint_say=(fp.get("say") or "")[:160],
                           editor_up=fp.get("state") == "ready", run_ms=round((time.monotonic() - started) * 1000))
            if box_obs.get("box"):
                h.shot("after_box")
            print("DIAG startup_box_observe " + json.dumps(box_obs), flush=True)
            # a window gone within 3 s by itself is progress (bp.10's "Load PCB"), not a box waiting for someone
            lasted = (box_obs.get("gone_ms") or box_obs.get("run_ms", 0)) - box_obs.get("first_ms", 0) if box_obs.get("box") else 0
            box_obs["lasted_ms"] = lasted
            box_obs["kind"] = "none" if not box_obs.get("box") else ("progress" if box_obs.get("gone_ms") and lasted < 3000 else "prompt")
            h.check("Start without a box waiting for someone before the first frame" + (f" (it showed {box_obs['box']['title']!r} for {lasted} ms)" if box_obs.get("box") else ""),
                    box_obs["kind"] != "prompt", box_obs)
            h.check("The editor came up in the end", bool(box_obs.get("editor_up")), box_obs)
            return h.finish()
        if stall_started:
            rest = stall_s - (time.monotonic() - stall_started)
            if rest > 0:
                time.sleep(rest)
            E.suspend_pid(stall_pid, False)
            timeline.append({"stage": "stall_ended_after_settle", "s": stall_s})
        reps = h.kicad_reports()
        first, rep = reps.get("first_paint", {}), reps.get("settled", {})
        frame, pid = int(rep.get("shown") or 0), int(rep.get("pid") or 0)
        h.track_pid(pid)
        snap = focus_snapshot(frame)
        children = E.list_child_windows(frame)
        with open(h.p("opening_children.json"), "w", encoding="utf-8") as f:
            json.dump(children, f, indent=2)
        timeline.append({"stage": "before_canvas_input", "first_keys": first.get("keys"), "settled_keys": rep.get("keys"), **snap})
        print("DIAG opening_focus " + json.dumps(timeline[-1]), flush=True)
        if snap["focus_pid"] == pid and not snap["inside_frame"] and snap["focus"]:
            # focus in KiCad's process but outside the shown frame (bp.10, run Kb10O: a "Button"): name it, its top-level,
            # and KiCad's visible top-levels at this instant, so the next occurrence names itself
            f = snap["focus"]
            ftop = int(E.u32.GetAncestor(f, 2) or 0)  # GA_ROOT
            stray = {"focus_class": E.hwnd_class(f), "focus_title": window_title(f), "toplevel": ftop,
                     "toplevel_class": E.hwnd_class(ftop) if ftop else "", "toplevel_title": window_title(ftop) if ftop else "",
                     "kicad_visible_toplevels": [(w["hwnd"], w["class"], w["title"]) for w in kicad_toplevels() if w["visible"]]}
            timeline.append({"stage": "focus_outside_shown_frame", **stray})
            print("DIAG focus_outside_shown_frame " + json.dumps(stray), flush=True)
        # Which of the runtime's guard clauses can hold now (B' ignored the opening request
        # though the frame itself was foreground): native visibility walks ancestors, and the
        # client keeps its container hidden until the frame paints.
        reqs = []
        with open(h.p("client.log"), encoding="utf-8", errors="replace") as f:
            for line in f:
                if line.startswith("GPUI_LOG ") and "kicad_focus_request" in line:
                    try:
                        reqs.append(json.loads(line[9:])["kicad_focus_request"])
                    except ValueError:
                        pass
        holder = int(reqs[0]["holder"]) if reqs else 0
        parent = int(reqs[0]["parent"]) if reqs else 0
        top = int(rep.get("toplevel") or 0)
        vis = {"holder": holder, "parent": parent, "holder_visible": bool(holder and E.u32.IsWindowVisible(holder)),
               "frame_visible": bool(frame and E.u32.IsWindowVisible(frame)), "parent_visible": bool(parent and E.u32.IsWindowVisible(parent)),
               "frame_in_holder": bool(holder and frame and E.u32.IsChild(holder, frame)), "frame_in_top": bool(top and frame and E.u32.IsChild(top, frame)),
               "holder_parent_ok": bool(holder and parent and int(E.u32.GetParent(holder) or 0) == parent), "frame_enabled": bool(frame and E.u32.IsWindowEnabled(frame))}
        timeline.append({"stage": "guard_clauses_at_settle", **vis})
        print("DIAG guard_clauses " + json.dumps(vis), flush=True)
        if os.environ.get("BP_OPENING_FOCUS_REPOST") == "1" and holder and frame and parent:
            # Diagnostic only: the client's own request, posted again once the pane is shown.
            ok = E.u32.PostMessageW(holder, 0x8000 + 0x103, frame, parent)
            start = time.monotonic()
            snap, ms = focus_snapshot(frame), None
            while time.monotonic() - start < 2:
                snap = focus_snapshot(frame)
                if snap["inside_frame"] and snap["focus_class"] != "Edit":
                    ms = round((time.monotonic() - start) * 1000)
                    break
                time.sleep(0.02)
            timeline.append({"stage": "after_repost", "posted": bool(ok), "settle_ms": ms, **snap})
            print("DIAG repost " + json.dumps(timeline[-1]), flush=True)
        h.shot("01_before_input")
        if only_ctrl_k:
            # The keys are KiCad's straight after Edit (the opening fix): proven by plain F2
            # acting, with no click anywhere first. Clicking Backplane's window (the tab) would
            # take them away, and getting them back is a separate, known gap (F4).
            gls = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
            gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
            px, zoomed = plain_key_acts(h, gl, "ctrlk_pre", frame) if gl else (0, False)
            pre = {"tab_clicked": None, "f2_changed_px": px, "zoom_readout_changed": zoomed, **focus_snapshot(frame)}
            pre["inside_frame"] = pre["inside_frame"] and bool(zoomed)
            timeline.append({"stage": "before_ctrl_k_from_edit", **pre})
            ctrl_k_round(h, cp, frame, rep, timeline, 1, 1, pre=pre)
            return h.finish()
        if only == "checks":
            # The Checks panel under the embedded PCB editor and KiCad's markers: embed_checks_panel_e2e.sh (P1-P5) plus the Run in the
            # panel (RunChecks in KiCad, as embed_marker_click_e2e.sh starts it with kicad.checks.run) and K5 of embed_editor_checks_e2e.sh
            # (KiCad's DRC marker clicked: the panel names its row). embed_review_markers_e2e.sh has no click of its own (it checks the
            # design review's host markers by GetMarkers); the marker click its brief names is K5's. Linux steps not ported:
            #   - the Linux suite's script waits for the editor and opens the panel itself; here the opening loop above did Edit and the
            #     panel is opened by a real click on mgmt:checks (the same control id), so "the panel opened" is a check of its own here.
            #   - shot names follow Linux (p1_panel ...); the X display's window list (xwininfo) is E.client_origin here.
            import re
            top = E.find_main_window(cp.pid)
            sock = rep.get("sock") or first.get("sock") or ""
            pro = checks_project(h)
            pcb = pro[:-len(".kicad_pro")] + ".kicad_pcb"
            h.check("checks: the seed put a dangling label in the schematic", bool(seeded and seeded["label_added"]), seeded)
            h.check("checks: KiCad's IPC answers", h.wait_for_ipc(sock, 80), {"sock": sock})
            # C1 (Linux: the script's click on mgmt:checks): the panel opens, its Run button drawn
            n0 = len(log_lines(h))
            opened_click = h.click_control(top, "mgmt:checks")
            end = time.monotonic() + 10
            while time.monotonic() < end and "mgmt:checks:run" not in h.controls():
                time.sleep(0.2)
            toggles = [l[9:200] for l in log_lines(h)[n0:] if l.startswith("GPUI_LOG ") and "mgmt" in l and "checks" in l][-2:]
            h.check("checks: a click on Checks opens the panel (its Run button is drawn)", opened_click and "mgmt:checks:run" in h.controls(),
                    {"clicked": opened_click, "log": toggles})
            # P1: the panel's data is the hub's, both checks fresh
            d, end = {}, time.monotonic() + 120
            while time.monotonic() < end:
                d = checks_watch(h, pro)
                if checks_fresh(d):
                    break
                time.sleep(1)
            h.check("checks P1: the panel's data is the hub's, both checks fresh",
                    (d.get("drc") or {}).get("state") == "fresh" and d.get("total", 0) > 0 and (d.get("badge") or {}).get("n", 0) > 0,
                    {"chip": d.get("chip", ""), "drc": (d.get("drc") or {}).get("state"), "erc": (d.get("erc") or {}).get("state"),
                     "total": d.get("total"), "badge": d.get("badge")})
            time.sleep(1.5)
            h.shot("p1_panel")
            # Run in the panel: the hub sends KiCad RunChecks (its answer: the run's progress id) and waits for ET_CHECKS_DONE (the finish);
            # the panel's DRC result is then the editor's (in_editor), newer than the press. Read from the hub's BACKPLANE_KICAD_DEBUG
            # lines and, beside them, KiCad's own event queue (NextEvents; reading it takes nothing from the hub).
            hub_n = len(log_lines(h, "hub.log"))
            try:
                since = int(h.ipc_cmd("events", sock, "0").get("next", 0))
            except Exception:
                since = None
            t0ms = int(time.time() * 1000)
            ran_click = h.click_control(top, "mgmt:checks:run")
            states, d, end = [], {}, time.monotonic() + 300
            while time.monotonic() < end:
                d = checks_watch(h, pro)
                st = ((d.get("drc") or {}).get("state"), bool((d.get("drc") or {}).get("in_editor")))
                if not states or states[-1] != st:
                    states.append(st)
                if checks_fresh(d) and int((d.get("drc") or {}).get("at") or 0) > t0ms:
                    break
                time.sleep(0.5)
            new = log_lines(h, "hub.log")[hub_n:]
            req = [l[:200] for l in new if "request kiapi.backplane.embed.RunChecks" in l and "FK_PCB" in l]
            started = [l[:200] for l in new if "checks: drc in KiCad starts" in l]
            to_cli = [l[:200] for l in new if "checks: drc with kicad-cli" in l]
            done = [l for l in new if "kicad embed event:" in l and ("ET_CHECKS_DONE" in l or '"type":27' in l)]
            done_ids = [m.group(1) for l in done for m in [re.search(r'"progress_id"\s*:\s*"?([0-9]+)', l)] if m and m.group(1) != "0"]
            kev = []
            if since is not None:
                try:
                    kev = [e for e in h.ipc_cmd("events", sock, str(since)).get("events", []) if e.get("type") == 27]
                except Exception as e:
                    kev = [{"error": str(e)[:120]}]
            h.check("checks: Run in the panel sends KiCad RunChecks for the board", ran_click and bool(req) and bool(started),
                    {"clicked": ran_click, "request": req[-1:], "start": started[-1:]})
            h.check("checks: KiCad takes the run (RunChecks answered with a progress id, no fall back to kicad-cli)",
                    bool(done_ids) and not to_cli, {"progress_ids": done_ids[-2:], "kicad_cli": to_cli[-1:]})
            dd = d.get("drc") or {}
            h.check("checks: the run finishes (ET_CHECKS_DONE) and the panel shows the editor's result, newer than the press",
                    bool(done) and dd.get("state") == "fresh" and bool(dd.get("in_editor")) and int(dd.get("at") or 0) > t0ms,
                    {"done": [l[l.find("{"):][:160] for l in done[-1:]], "kicad_events": [e.get("checks", e) for e in kev[-1:]],
                     "states": states, "drc": {k: dd.get(k) for k in ("state", "in_editor", "unsaved", "at", "took_ms", "counts")}})
            time.sleep(1.0)
            h.shot("run_done")
            # the results listed: a DRC row naming a part and the ERC dangling label, among the rows a shut group shows (3, SHUT_ROWS)
            rows = [r for g_ in d.get("groups") or [] for r in (g_.get("rows") or [])[:3]]
            drc = next((r for r in rows if r.get("source") == "drc" and re.search(r"\b[A-Z]{1,4}[0-9]+\b", r.get("names") or "")), None)
            erc = next((r for r in rows if r.get("source") == "erc" and r.get("rule") == "label_dangling"), None)
            drc_id, erc_id = (drc or {}).get("id", ""), (erc or {}).get("id", "")
            drc_ref = re.search(r"\b([A-Z]{1,4}[0-9]+)\b", drc["names"]).group(1) if drc else ""
            print(f"INFO rows: drc {drc_id} ({drc_ref}), erc {erc_id}", flush=True)
            h.check("checks: the results are listed (a DRC row naming a part, the ERC dangling label)", bool(drc and erc),
                    {"drc": drc_id, "ref": drc_ref, "erc": erc_id, "rows": [(r.get("source"), r.get("rule")) for r in rows][:8]})
            end = time.monotonic() + 5
            while time.monotonic() < end and not (ctl_point(h, top, f"mgmt:checks:row:{drc_id}") and ctl_point(h, top, f"mgmt:checks:row:{erc_id}")):
                time.sleep(0.2)
            h.check("checks P1: the DRC and ERC rows are drawn",
                    bool(drc and erc and ctl_point(h, top, f"mgmt:checks:row:{drc_id}") and ctl_point(h, top, f"mgmt:checks:row:{erc_id}")),
                    {"drc": ctl_point(h, top, f"mgmt:checks:row:{drc_id}"), "erc": ctl_point(h, top, f"mgmt:checks:row:{erc_id}")})
            if not (drc and erc):
                return h.finish()
            # K5: KiCad's own DRC marker clicked (ClickMarker, click-marker): the hub names its row to the panel (review.clicked.<digest>)
            try:
                caps = json.dumps(h.ipc_cmd("caps", sock))
            except Exception as e:
                caps = str(e)
            ans = standin_ask(h, "kicad_ipc", {"path": pcb, "type": "kiapi.backplane.embed.GetMarkers", "message": {"kind": "FK_PCB"}}, "markers")
            try:
                allm = [m for m in (find_markers(json.loads(ans[ans.find("{"):])) or []) if m.get("at_nm")]
            except ValueError:
                allm = []
            errs = [m for m in allm if m.get("severity") == "error" and m.get("items") and not m.get("excluded")]
            xy = lambda m: (int(m["at_nm"].get("x_nm", 0)), int(m["at_nm"].get("y_nm", 0)))
            alone = [m for m in errs if all(n is m or (xy(n)[0] - xy(m)[0]) ** 2 + (xy(n)[1] - xy(m)[1]) ** 2 > 3000000 ** 2 for n in allm)]
            mk = (alone or errs or [None])[0]
            mid, want = "", ""
            if mk:
                mid, mtype = (mk.get("id") or {}).get("value") or "", mk.get("type", "")
                items = {x.get("value") for x in mk["items"]}
                want = next((f["id"] for f in checks_list(h, pro, "drc") if f.get("rule") == mtype
                             and {a.get("uuid") for a in f.get("anchors") or [] if a.get("uuid")} == items), "")
                print(f"INFO the marker to click: {mtype} {mid} at {xy(mk)} nm, items {sorted(items)}; its row {want}", flush=True)
            if mk and mid and "click-marker" in caps:
                cn = len(log_lines(h))
                ckr = standin_ask(h, "kicad_ipc", {"path": pcb, "type": "kiapi.backplane.embed.ClickMarker",
                                                   "message": {"kind": "FK_PCB", "marker": {"value": mid}}}, "click")
                got, end = "", time.monotonic() + 10
                while time.monotonic() < end and not got:
                    rpc_txt = "\n".join(log_lines(h, "rpc.log")) + "\n" + h.hub_rpc([])
                    clk = hub_infos(rpc_txt, "review.clicked.")
                    got = ((clk[-1][1] or {}).get("id") or "") if clk and isinstance(clk[-1][1], dict) else ""
                    if not got:
                        time.sleep(0.5)
                rev = [l[9:120] for l in log_lines(h)[cn:] if l.startswith("GPUI_LOG ") and "checks_reveal" in l]
                hub_said = [l[:160] for l in log_lines(h, "hub.log") if "marker clicked" in l][-1:]
                h.check("checks K5: ClickMarker on KiCad's DRC marker shows its row in the panel",
                        bool(want) and got == want, {"want": want, "showed": got or "none", "reveal": rev[-1:], "hub": hub_said, "answer": ckr[:120]})
                h.shot("k5_marker")
            elif mk:
                # Linux makes the real click informational too: it is placed by colour on a centred view and does not reliably land
                # on the marker (markers stacked on vias); the marker-to-row step is unit-tested (edchecks row_of)
                print(f"INFO K5 (not a check): this KiCad has no click-marker; marker {mid}, row {want}", flush=True)
            else:
                print(f"INFO no error marker with a place and items (GetMarkers: {ans[:200]}): the marker click is left to the unit tests", flush=True)
            # P2: a real click on the DRC row selects its part in the PCB editor (GetSelection over KiCad's IPC); click -> selected in ms
            g = ipc_module()
            try:
                h.ipc_cmd("run-action", sock, "common.Interactive.selectionClear")
            except Exception:
                pass
            r2 = click_and_wait_selected(h, g, sock, ctl_point(h, top, f"mgmt:checks:row:{drc_id}"), "pcb", drc_ref)
            h.check(f"checks P2: clicking the DRC row selected {drc_ref} in the PCB editor", r2["ok"], r2)
            time.sleep(1)
            h.shot("p2_drc_row")
            # P3: the ERC row: the editor switches to the schematic with the label selected
            r3 = click_and_wait_selected(h, g, sock, ctl_point(h, top, f"mgmt:checks:row:{erc_id}"), "sch", "")
            time.sleep(1.5)
            ed = hub_infos(h.hub_rpc([]), "editor.kicad.")
            active = next(((v or {}).get("active", "") for _, v in reversed(ed) if isinstance(v, dict)), "")
            h.check("checks P3: clicking the ERC row showed the schematic with the label selected", r3["ok"] and active == "SCH",
                    {"active": active, **r3})
            h.shot("p3_erc_row")
            # P4: Down from the DRC row goes to the next row and cross-probes it
            pt = ctl_point(h, top, f"mgmt:checks:row:{drc_id}")
            if pt:
                E.screen_click(*pt)
            time.sleep(1.2)
            n4 = len(log_lines(h))
            E.key_tap("Down")
            time.sleep(1.2)
            nxt = {}
            for l in log_lines(h)[n4:]:
                if l.startswith("GPUI_LOG ") and '"checks_select"' in l:
                    try:
                        nxt = json.loads(l[9:])
                    except ValueError:
                        pass
            h.check("checks P4: Down moved to the next row and cross-probed it", bool(nxt.get("id")) and nxt.get("id") != drc_id,
                    {"from": drc_id, "to": nxt.get("id"), "focus": focus_snapshot(frame)})
            # P5: Fix with agent: the stand-in agent gets the finding
            pt = ctl_point(h, top, f"mgmt:checks:row:{erc_id}")
            if pt:
                E.screen_move(*pt)
            time.sleep(0.6)
            fixed = h.click_control(top, f"mgmt:checks:fix:{erc_id}")
            got5, end = False, time.monotonic() + 20
            while time.monotonic() < end and not got5:
                got5 = any(erc_id in l for l in log_lines(h, "standin.log")) or any(erc_id in l for l in log_lines(h, os.path.join("home", "events.jsonl")))
                if not got5:
                    time.sleep(0.5)
            h.check("checks P5: Fix with agent sent the finding to the thread's agent", got5, {"id": erc_id, "fix_clicked": fixed})
            h.shot("p5_fix")
            return h.finish()
        if only == "libtable":
            # a person whose global tables predate Backplane's library: the hub adds the runtime's Backplane entries, nothing else changes
            cfg = h.p("fh", "AppData", "Roaming", "kicad", "10.99")
            time.sleep(3)
            fp = open(os.path.join(cfg, "fp-lib-table"), encoding="utf-8", newline="").read()
            sym = open(os.path.join(cfg, "sym-lib-table"), encoding="utf-8", newline="").read()
            mine_fp = LIBTABLE_MINE["fp-lib-table"]
            head = mine_fp.rstrip()[:-1].rstrip("\n\r\t ")
            added = [n for n in BP_LIBS if f'(name "{n}")' in fp]
            h.check("libtable: the hub added every Backplane footprint library to the person's fp-lib-table", added == BP_LIBS, {"added": added, "table": fp[-1500:]})
            h.check("libtable: the person's own entries are kept as they were (KiCad's table, My_Footprints, byte for byte)", fp.startswith(head), {"start": fp[:400]})
            extra = [l for l in fp[len(head):].splitlines() if l.strip() and l.strip() != ")" and not any(f'(name "{n}")' in l for n in BP_LIBS)]
            h.check("libtable: nothing else is added or changed in fp-lib-table", not extra, extra)
            h.check("libtable: sym-lib-table is unchanged (the Backplane library has no symbols)", sym == LIBTABLE_MINE["sym-lib-table"], sym[-600:])
            backups = glob.glob(h.p("home", "backups", "kicad-libtables", "*"))
            h.check("libtable: a copy of the person's table was kept before it was changed", bool(backups), backups[:4])
            return h.finish()
        if only == "libs":
            # bp.13 / 0.16.0 gate (the person, Nov 16): on a clean home with only the managed runtime (this harness's own APPDATA,
            # LOCALAPPDATA and KICAD_CONFIG_HOME, no other KiCad; KICAD = the runtime the hub's own installer put in place), the standard
            # libraries and Backplane's own (Backplane_KiCad_Library, footprints and 3D models) are there, registered, and usable:
            # an agent's kicad_edit places a standard part and a Backplane-library footprint, the 3D model is found and exported,
            # both choosers list libraries, and KiCad's checks find nothing about libraries.
            import re as _re
            import subprocess
            sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
            import libgate_design as LG
            top = int(rep.get("toplevel") or 0) or E.find_main_window(cp.pid)
            pro = checks_project(h)
            sch, pcb = pro[:-len(".kicad_pro")] + ".kicad_sch", pro[:-len(".kicad_pro")] + ".kicad_pcb"
            root = h.kicad_dir
            share = os.path.join(root, "share", "kicad")
            listing = sorted(os.listdir(share)) if os.path.isdir(share) else []
            bpl = os.path.join(share, "backplane-library")
            pretty = sorted(os.listdir(os.path.join(bpl, "libraries"))) if os.path.isdir(os.path.join(bpl, "libraries")) else []
            model = os.path.join(bpl, "3dmodels", "Capacitor_SMD.3dshapes", "C_0805_2012Metric.step")
            def read(pth):
                try:
                    return open(pth, encoding="utf-8", errors="replace").read()
                except OSError:
                    return ""
            dfp, dsym = read(os.path.join(share, "fp-lib-table")), read(os.path.join(share, "sym-lib-table"))
            h.check("libs: the runtime has the standard symbols, footprints and template", all(x in listing for x in ("symbols", "footprints", "template")), {"share": share, "has": listing})
            h.check("libs: the runtime's default global tables hold the standard KiCad table and every Backplane footprint library",
                    '(name "KiCad")' in dfp and '(name "KiCad")' in dsym and all(f'(name "{n}")' in dfp for n in BP_LIBS),
                    {"fp": dfp[-1200:], "sym": dsym[-400:]})
            h.check("libs: the Backplane library is packaged (6 .pretty) with its 3D model C_0805_2012Metric.step",
                    len([x for x in pretty if x.endswith(".pretty")]) >= 6 and os.path.isfile(model), {"pretty": pretty, "model": model, "model_found": os.path.isfile(model)})
            try:
                man = json.load(open(os.path.join(root, "manifest.json"), encoding="utf-8"))
            except (OSError, ValueError):
                man = {}
            bl = man.get("backplaneLibrary") or {}
            srcs = read(os.path.join(root, "KICAD_LIBRARY_SOURCES.txt")) or read(os.path.join(share, "KICAD_LIBRARY_SOURCES.txt"))
            h.check("libs: manifest backplaneLibrary matches KICAD_LIBRARY_SOURCES.txt (ref, commit, sha256)",
                    bool(bl) and all(str(bl.get(k, "")) and str(bl.get(k)) in srcs for k in ("ref", "commit", "sha256")), {"backplaneLibrary": bl, "sources": srcs[:800]})
            cfg = h.p("fh", "AppData", "Roaming", "kicad", "10.99")
            ufp, usym = read(os.path.join(cfg, "fp-lib-table")), read(os.path.join(cfg, "sym-lib-table"))
            h.check("libs: the person's global tables (made at first launch) contain the standard and the Backplane libraries",
                    '(name "KiCad")' in ufp and '(name "KiCad")' in usym and all(f'(name "{n}")' in ufp for n in BP_LIBS), {"fp": ufp[-1200:], "sym": usym[-400:]})
            env = h.base_env()
            def cli_report(kind, path, outp, extra=()):
                cmd = [h.kicad_cli, kind, "erc" if kind == "sch" else "drc", "--format", "json", "--severity-all", "-o", outp, *extra, path]
                r = subprocess.run(cmd, capture_output=True, text=True, env=env, timeout=600)
                try:
                    return r.returncode, json.load(open(outp, encoding="utf-8")), (r.stdout + r.stderr)[-400:]
                except (OSError, ValueError):
                    return r.returncode, None, (r.stdout + r.stderr)[-400:]
            # information only: KiCad's checks on the seed board as it is (it has findings of its own)
            for kind, path in (("sch", sch), ("pcb", pcb)):
                rc, rpt, _ = cli_report(kind, path, h.p(f"libs_seed_{kind}.json"), ("--schematic-parity",) if kind == "pcb" else ())
                print(f"INFO libs seed {kind}: " + json.dumps(LG.count_cli(rpt) if rpt else {"rc": rc}), flush=True)
            # the gate's fresh design (lib/libgate_design.py: the same steps on Linux, macOS and Windows)
            gdir = os.path.join(os.path.dirname(pro), "libgate")
            gpro, gsch, gpcb = LG.make_project(gdir, pro, pcb, sch)
            subprocess.run(["git", "-C", os.path.dirname(pro), "add", "libgate"], capture_output=True, text=True)
            subprocess.run(["git", "-C", os.path.dirname(pro), "commit", "-qm", "libs gate: a new project"], capture_output=True, text=True)
            a1 = standin_ask(h, "kicad_edit", {"path": gsch, "message": "libs gate: C1, C2 in parallel", "ops": LG.schematic_ops()}, "lg-sch")
            st = open(gsch, encoding="utf-8", errors="replace").read()
            h.check("libs: the agent's kicad_edit puts C1 (Device:C, Capacitor_SMD:C_0603_1608Metric) and C2 (Device:C, Backplane_Passives_Nominal:C_0805_2012Metric) in parallel on N1/N2",
                    st.count('(lib_id "Device:C")') >= 2 and '"Capacitor_SMD:C_0603_1608Metric"' in st and '"Backplane_Passives_Nominal:C_0805_2012Metric"' in st
                    and '"N1"' in st and '"N2"' in st, {"answer": a1[:700]})
            net = os.path.join(gdir, "libgate.net")
            ex = subprocess.run([h.kicad_cli, "sch", "export", "netlist", "--format", "kicadsexpr", "-o", net, gsch], capture_output=True, text=True, env=env, timeout=300)
            a2 = standin_ask(h, "kicad_edit", {"path": gpcb, "message": "libs gate: update from the schematic", "ops": LG.netlist_ops(net.replace("\\", "/"))}, "lg-net")
            a3 = standin_ask(h, "kicad_edit", {"path": gpcb, "message": "libs gate: place and route", "ops": LG.board_ops()}, "lg-pcb")
            bt = open(gpcb, encoding="utf-8", errors="replace").read()
            h.check("libs: the board is updated from the schematic (both footprints, from their libraries)",
                    "Capacitor_SMD:C_0603_1608Metric" in bt and "Backplane_Passives_Nominal:C_0805_2012Metric" in bt, {"netlist_rc": ex.returncode, "answer": a2[:700]})
            h.check("libs: the agent places both inside the board outline and routes N1 and N2", bt.count("(segment") >= 2 and "failed" not in a3, {"answer": a3[:900]})
            h.check("libs: the Backplane footprint brings its 3D model (${BACKPLANE_KICAD_LIBRARY}/3dmodels/Capacitor_SMD.3dshapes/C_0805_2012Metric.step)",
                    "Capacitor_SMD.3dshapes/C_0805_2012Metric.step" in bt, "present" if "C_0805_2012Metric.step" in bt else "absent")
            step, bstep = h.p("libgate.step"), h.p("libgate_board.step")
            ex = subprocess.run([h.kicad_cli, "pcb", "export", "step", "--force", "-o", step, gpcb], capture_output=True, text=True, env=env, timeout=600)
            exb = subprocess.run([h.kicad_cli, "pcb", "export", "step", "--force", "--board-only", "-o", bstep, gpcb], capture_output=True, text=True, env=env, timeout=600)
            rd = lambda f: open(f, encoding="utf-8", errors="replace").read() if os.path.isfile(f) else ""
            res = LG.step_has_model(rd(step), rd(bstep), rd(model))
            h.check("libs: the board's STEP includes C2's Backplane 3D model (its product name, or two more solids than the board alone)",
                    ex.returncode == 0 and exb.returncode == 0 and res["ok"], {**res, "rc": [ex.returncode, exb.returncode], "out": (ex.stdout + ex.stderr)[-400:]})
            rc, rpt, o = cli_report("sch", gsch, h.p("libgate_erc.json"))
            n = LG.count_cli(rpt) if rpt else None
            h.check("libs: kicad-cli ERC of the fresh design: 0 errors, 0 warnings", bool(n) and n["violations"] == 0, {"rc": rc, "count": n, "out": o})
            rc, rpt, o = cli_report("pcb", gpcb, h.p("libgate_drc.json"), ("--schematic-parity",))
            n = LG.count_cli(rpt) if rpt else None
            h.check("libs: kicad-cli DRC with schematic parity: 0 violations, 0 unconnected, 0 parity", bool(n) and n["violations"] == n["unconnected"] == n["parity"] == 0,
                    {"rc": rc, "count": n, "out": o})
            for kind, path, extra in (("FK_PCB", gpcb, {"parity": True}), ("FK_SCH", gsch, {})):
                standin_ask(h, "kicad_ipc", {"path": path, "type": "kiapi.backplane.embed.RunChecks", "message": {"kind": kind, **extra}}, f"lg-run-{kind}")
                time.sleep(20)
                ans = standin_ask(h, "kicad_ipc", {"path": path, "type": "kiapi.backplane.embed.GetMarkers", "message": {"kind": kind}}, f"lg-mk-{kind}")
                try:
                    payload = json.loads(ans.split(" ", 2)[2]) if ans else None
                except (ValueError, IndexError):
                    payload = ans
                m = LG.count_markers(payload)
                h.check(f"libs: in the editor, KiCad's {'DRC with parity' if kind == 'FK_PCB' else 'ERC'} of the fresh design leaves no marker",
                        m is not None and m["count"] == 0, {"markers": m, "answer": (ans or "")[:500]})
            def chooser(tab, prefix):
                h.click_control(top, tab)
                time.sleep(4.0)
                E.key_tap("a")
                found, end = None, time.monotonic() + 20
                while time.monotonic() < end and not found:
                    found = next((w for w in kicad_toplevels() if w["visible"] and (w["title"] or "").startswith(prefix)), None)
                    time.sleep(0.5)
                time.sleep(4.0)  # the count in the title grows while it loads
                title = window_title(found["hwnd"]) if found else ""
                m = _re.search(r"\((\d+) items? loaded\)", title)
                n = int(m.group(1)) if m else (0 if found else None)
                shot = h.p(f"libs_{prefix.split()[0].lower()}.png")
                E.grab_screen(shot)
                if found:
                    E.key_tap("Escape")
                    time.sleep(0.8)
                return {"title": title, "items": n, "shot": shot}
            sym = chooser("kicad-tab:SCH", "Choose Symbol")
            h.check("libs: the embedded schematic editor's symbol chooser lists the standard libraries (more than 1000 symbols)", (sym["items"] or 0) > 1000, sym)
            fpc = chooser("kicad-tab:PCB", "Footprint Chooser")
            h.check("libs: the embedded board editor's footprint chooser lists the standard and Backplane libraries (more than 1000 footprints)", (fpc["items"] or 0) > 1000, fpc)
            return h.finish()
        if only == "lend":
            # A5 / Climb B's lend (e9af1ffe5): a reserved key pressed in KiCad lends Backplane the keyboard, and the palette
            # dismissed where it was opened gives it back (embed_lend_e2e.sh on Linux):
            #   L1 Ctrl+K in KiCad opens the palette, Backplane has the keys; L2 Escape shuts it and a plain F2 acts in KiCad
            #   with no click; L3 a second Escape goes to KiCad (the viewer stays); L4 Ctrl+K, another thread: keys stay Backplane's
            gls = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
            gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
            top = int(rep.get("toplevel") or 0) or E.find_main_window(cp.pid)
            px0, zoomed0 = plain_key_acts(h, gl, "lend_pre", frame) if gl else (0, False)
            pre = focus_snapshot(frame)
            h.check("lend: the editor has the keys after Edit (F2 acts, no click)", bool(zoomed0) and pre["inside_frame"], {"f2_px": px0, **pre})
            wx, wy, ww, wh = window_box(top)
            pane_x = int((rep.get("container") or {}).get("root_x") or (wx + ww))
            box = (wx, wy, max(1, pane_x - wx), min(wh, 480))
            a, b = h.p("lend_0.png"), h.p("lend_1.png")
            n0 = len(open(h.p("client.log"), encoding="utf-8", errors="replace").read().splitlines())
            E.grab_screen(a)
            E.key_chord("ctrl", "k")
            time.sleep(1.0)
            E.grab_screen(b)
            l1 = focus_snapshot(frame)
            opened = E.img_diff_box(a, b, *box)["changed_pixels"]
            def log_since(n):
                return open(h.p("client.log"), encoding="utf-8", errors="replace").read().splitlines()[n:]
            disp = any('"kicad_key_dispatch":"ctrl-k"' in l for l in log_since(n0))
            h.check("lend L1: Ctrl+K pressed in KiCad opens the palette and Backplane has the keys",
                    opened > 500 and not l1["inside_frame"] and l1["foreground"] == top, {"palette_px": opened, "dispatch_logged": disp, **l1})
            E.key_tap("Escape")
            time.sleep(0.8)
            l2 = focus_snapshot(frame)
            back = [l.strip()[:200] for l in log_since(n0) if "kicad_loan_back" in l]
            h.check("lend L2: Escape shuts the palette and KiCad has the keyboard again", l2["inside_frame"] and l2["foreground"] == top, {"loan_back": back[-1:], **l2})
            px2, zoomed2 = plain_key_acts(h, gl, "lend_l2", frame) if gl else (0, False)
            h.check("lend L2: then a plain F2 acts in KiCad's canvas with no click", bool(zoomed2), {"f2_px": px2})
            E.key_tap("Escape")
            time.sleep(1.2)
            l3 = focus_snapshot(frame)
            still = bool(frame and E.u32.IsWindow(frame) and E.u32.IsWindowVisible(frame))
            h.check("lend L3: a second Escape goes to KiCad: the viewer stays open", still and l3["inside_frame"], {"frame_visible": still, **l3})
            n2 = len(log_since(0))
            E.key_chord("ctrl", "k")
            time.sleep(0.8)
            cur = None
            for l in log_since(0):
                m = __import__("re").search(r'"thread":"(t[0-9]+)","open":"board"', l)
                if m:
                    cur = m.group(1)
            # the whole title, as the Linux step types it ("embedded" alone did not reach t1: lend2/lend3, Nov 16)
            other, title = ("t1", "Embedded KiCad E2E") if cur == "t2" else ("t2", "Other thread 2")
            for ch in title.lower():
                E.key_tap("space" if ch == " " else ch)
            time.sleep(0.8)
            E.grab_screen(h.p("lend_l4_typed.png"))
            E.key_tap("Return")
            time.sleep(1.5)
            l4 = focus_snapshot(frame)
            fgw = l4.get("foreground") or 0
            l4["foreground_title"] = window_title(fgw) if fgw else ""
            l4["foreground_class"] = E.hwnd_class(fgw) if fgw else ""
            l4["foreground_pid"] = E.hwnd_pid(fgw) if fgw and hasattr(E, "hwnd_pid") else None
            # do typed keys reach Backplane? (its window left of the pane changes)
            ta, tb = h.p("lend_l4_k0.png"), h.p("lend_l4_k1.png")
            E.grab_screen(ta)
            for ch in "zq":
                E.key_tap(ch)
            time.sleep(0.6)
            E.grab_screen(tb)
            l4["typed_px"] = E.img_diff_box(ta, tb, wx, wy + wh * 3 // 4, max(1, pane_x - wx), wh // 4)["changed_pixels"]  # the composer, low down (not the header: its Buy it button changes on a thread jump)
            went = any(f'"thread":"{other}"' in l for l in log_since(n2))
            back4 = [l.strip()[:200] for l in log_since(n2) if "kicad_loan_back" in l]
            h.check("lend L4: Ctrl+K then another thread: the keyboard stays with Backplane",
                    went and not l4["inside_frame"] and l4["focus_pid"] == cp.pid and l4["typed_px"] > 20, {"from": cur, "to": other, "went": went, "loan_back": back4[-1:], **l4})
            return h.finish()
        if only == "close":
            # embed_close_e2e.sh (47b00d199): KiCad's own Close (Ctrl+W) in the embedded board editor; the pane follows KiCad,
            # never "Starting KiCad…" for ever:
            #   C1 nothing unsaved: Ctrl+W closes the PCB frame; the pane leaves Edit for View (Edit offered again); Edit opens a fresh frame
            #   C2 an unsaved edit, Ctrl+W, KiCad's prompt answered Cancel (Escape): the editor stays, still modified, the pane in Edit
            #   C3 the same, the prompt answered Save (Return, its default): the board is written, the frame closes, the pane goes to View
            # Keys reach KiCad by the editor's tab (give_kicad_keys), as every Windows mode does, where Linux clicks the frame's canvas.
            top = int(rep.get("toplevel") or 0) or E.find_main_window(cp.pid)
            sock = rep.get("sock") or ""
            pcbs = sorted(glob.glob(h.p("proj", "*.kicad_pcb"))) or sorted(glob.glob(h.p("proj", "boards", "*", "*.kicad_pcb")))
            pcb = pcbs[0] if pcbs else ""

            def log_since(n):
                return open(h.p("client.log"), encoding="utf-8", errors="replace").read().splitlines()[n:]

            def pcb_frame():
                # the PCB frame on show now (the report's frame list is only taken at script steps: Win32's own list here),
                # embedded under Backplane's window or, if KiCad put it out on its own, a top-level of KiCad's
                ws = [w for w in E.list_child_windows(top, pid) if w["visible"] and "pcb editor" in w["title"].lower() and w["w"] > 300]
                ws += [w for w in E.list_top_windows(pid) if "pcb editor" in w["title"].lower() and w["w"] > 300]
                return int(ws[0]["hwnd"]) if ws else 0

            def edit_rect():
                # Edit drawn (offered) in the latest controls snapshot: the pane is in View
                c = h.controls().get("ui:Edit") or {}
                r = c.get("rect")
                return r if isinstance(r, dict) and "x" in r else None

            def modified():
                try:
                    return h.ipc_cmd("modified", sock, "pcb").get("modified")
                except Exception as e:  # noqa: BLE001 - evidence, not a crash
                    return f"? {e}"[:120]

            def keys_to_kicad():
                f = pcb_frame()
                k = give_kicad_keys(h, cp, f) if f else {"tab_clicked": False, "target_missing": "no PCB frame on show"}
                time.sleep(0.8)
                return k

            def wait_until(cond, s):
                end = time.monotonic() + s
                while time.monotonic() < end:
                    if cond():
                        return True
                    time.sleep(0.1)
                return bool(cond())

            def closed_since(n):
                return [l.strip()[:200] for l in log_since(n) if '"kicad_frame_closed":"PCB"' in l]

            def prompt_windows():
                # KiCad's save prompt: a shown top-level of KiCad's that is not an editor frame
                return [{"hwnd": w["hwnd"], "title": w["title"], "class": w["class"], "foreground": int(E.u32.GetForegroundWindow() or 0) == w["hwnd"]}
                        for w in E.list_top_windows(pid) if w["w"] > 200 and "editor" not in w["title"].lower()]

            def sha_pcb():
                with open(pcb, "rb") as f:
                    return hashlib.sha256(f.read()).hexdigest()

            # C1
            n = h.log_line_count()
            k1 = keys_to_kicad()
            E.key_chord("ctrl", "w")
            gone = wait_until(lambda: not pcb_frame(), 3)
            h.shot("close_c1_after_ctrl_w")
            h.check("close C1: Ctrl+W with nothing unsaved closes KiCad's board editor", gone, {"frame": pcb_frame(), **k1})
            to_view = wait_until(lambda: bool(closed_since(n)) and bool(edit_rect()), 5)
            h.check("close C1: the pane leaves Edit for View (Edit offered again), no wait for the frame", to_view,
                    {"closed": closed_since(n)[:1], "edit_rect": edit_rect()})
            fresh = 0
            if edit_rect() and h.click_control(E.find_main_window(cp.pid), "ui:Edit"):
                wait_until(lambda: bool(pcb_frame()), 30)
                time.sleep(2)
                fresh = pcb_frame()
            h.shot("close_c1_edit_again")
            h.check("close C1: Edit again opens a fresh board editor in the pane", bool(fresh), {"frame": fresh, "first_frame": frame})
            # C2: an unsaved edit, Ctrl+W, Cancel
            try:
                add = h.ipc_cmd("agent-add-track", sock, "100", "100", "104", "100", "0.25", "3")
            except Exception as e:  # noqa: BLE001 - Linux ignores it too (|| true); the modified state below tells
                add = {"error": str(e)[:160]}
            time.sleep(1)
            print("INFO modified before Ctrl+W: " + json.dumps({"modified": modified(), "add": add}), flush=True)
            n = h.log_line_count()
            k2 = keys_to_kicad()
            E.key_chord("ctrl", "w")
            time.sleep(1.5)
            prompt = prompt_windows()
            print("INFO KiCad's save prompt: " + json.dumps(prompt or "none"), flush=True)
            h.shot("close_c2_prompt")
            E.key_tap("Escape")
            time.sleep(1.5)
            mod2 = modified()
            h.check("close C2: Cancel in KiCad's save prompt keeps the editor: the frame shown, still modified, the pane in Edit",
                    bool(pcb_frame()) and mod2 is True and not closed_since(n),
                    {"prompt": prompt[:1], "modified": mod2, "frame": pcb_frame(), "closed": closed_since(n)[:1], **k2})
            # C3: Ctrl+W, Save
            sha0 = sha_pcb()
            n = h.log_line_count()
            k3 = keys_to_kicad()
            E.key_chord("ctrl", "w")
            time.sleep(1.5)
            prompt = prompt_windows()
            h.shot("close_c3_prompt")
            E.key_tap("Return")
            time.sleep(3)
            written = sha_pcb() != sha0
            to_view = wait_until(lambda: not pcb_frame() and bool(closed_since(n)) and bool(edit_rect()), 3)
            h.check("close C3: Save in KiCad's prompt writes the board and closes the editor; the pane goes to View",
                    written and to_view, {"board_written": written, "prompt": prompt[:1], "frame": pcb_frame(), "closed": closed_since(n)[:1],
                                          "edit_rect": edit_rect(), **k3})
            return h.finish()
        if only == "prewarm_idle":
            # F5: Edit, keys in the PCB canvas, then nothing but the pre-warm: does the PCB editor keep the keys?
            gls = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
            gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
            top = E.find_main_window(cp.pid)
            before = focus_snapshot(frame)
            samples = []
            t0 = time.monotonic()
            while time.monotonic() - t0 < 12:
                snap = focus_snapshot(frame)
                if not samples or (snap["focus"], snap["foreground"]) != (samples[-1]["focus"], samples[-1]["foreground"]):
                    samples.append({"ms": round((time.monotonic() - t0) * 1000), **snap})
                time.sleep(0.02)
            pw = prewarm_frames(top)
            px, zoomed = plain_key_acts(h, gl, "prewarm_idle", frame) if gl else (0, False)
            after = focus_snapshot(frame)
            d = {"before": before, "changes": samples, "prewarmed": [(w["hwnd"], w["title"]) for w in pw], "after": after, "zoom_readout_changed": zoomed}
            print("DIAG prewarm_idle " + json.dumps(d)[:1500], flush=True)
            h.check("F5: Edit, then the pre-warm with no input: the PCB canvas keeps the keys (F2 acts by the zoom readout)",
                    bool(pw) and after["inside_frame"] and bool(zoomed), d)
            return h.finish()
        if only in ("stage2", "stage2_buttons", "pane_behaviours", "stage2_sweep", "stage2_middle"):
            import subprocess
            gls = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
            gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
            top = E.find_main_window(cp.pid)
            cbox = composer_box(h, top)
            cx_, cy_ = gl["root_x"] + gl["w"] // 2, gl["root_y"] + gl["h"] // 2
            def shield_clicks():
                return sum(1 for l in open(h.p("client.log"), encoding="utf-8", errors="replace") if "kicad_shield_click" in l)
            def deselect():
                # KiCad has the keys: a click on an empty spot of the board clears its selection (Escape did not, run ST2)
                if focus_snapshot(frame)["inside_frame"]:
                    E.screen_click(gl["root_x"] + 30, gl["root_y"] + gl["h"] - 40, 1)
                    time.sleep(0.5)
            E.INPUT_EXES.add("notepad.exe")  # this mode's own Notepad may take input; the pre-run check makes sure no other is open
            np_proc = subprocess.Popen(["notepad.exe"])
            np = 0
            for _ in range(50):
                np = E.find_main_window(np_proc.pid)
                if np:
                    break
                time.sleep(0.2)
            E.u32.SetWindowPos(np, 0, 1480, 120, 420, 500, 0x0040)
            def backplane_keys(entry):
                keys_back_to_backplane(h, cp, frame, top, "composer", cbox)
                if entry == "other_app":
                    E.screen_click(1480 + 200, 120 + 250, 1)
                time.sleep(0.4)
            def kicad_popups():
                return [w for w in kicad_toplevels() if w["visible"] and (w["class"] in ("#32768", "#32770") or "Properties" in w["title"])]
            def press(btn_down, btn_up, x, y, hold_s, to=None):
                E.screen_move(x, y)
                E._send(E._mouse(btn_down))
                t0 = time.monotonic()
                if to:
                    for i in range(1, 11):
                        E.screen_move(int(x + (to[0] - x) * i / 10), int(y + (to[1] - y) * i / 10))
                        time.sleep(hold_s / 10)
                else:
                    time.sleep(hold_s)
                E._send(E._mouse(btn_up))
                return round((time.monotonic() - t0) * 1000)
            if only == "stage2_buttons":
                res = {}
                deselect(); backplane_keys("composer")
                n0 = len(kicad_popups())
                panel = field_changed(h, message_panel_box(frame), lambda: E.screen_click(cx_, cy_, 1), "b_single")
                res["single"] = {"selected_px": panel, "popups_opened": len(kicad_popups()) - n0, "popups": [(w["class"], w["title"]) for w in kicad_popups()]}
                deselect(); backplane_keys("composer")
                n0 = len(kicad_popups())
                E.screen_move(cx_, cy_)
                E._send(E._mouse(2)); time.sleep(0.03); E._send(E._mouse(4)); time.sleep(0.06)
                E._send(E._mouse(2)); time.sleep(0.03); E._send(E._mouse(4))
                time.sleep(1.5)
                pops = kicad_popups()
                res["double"] = {"popups_opened": len(pops) - n0, "popups": [(w["class"], w["title"]) for w in pops]}
                for _ in pops:
                    E.key_tap("Escape"); time.sleep(0.5)
                deselect(); backplane_keys("composer")
                a, b = h.p("b_drag_0.png"), h.p("b_drag_1.png")
                E.grab_screen(a)
                # 60 px down: KiCad's infobar ("This file was created by an older version...") lies over the canvas's top
                # edge on some opens, and a drag started at +12 pressed its text instead (s2-buttons2, under "Static")
                held = press(2, 4, gl["root_x"] + 12, gl["root_y"] + 60, 0.3, (gl["root_x"] + gl["w"] - 12, gl["root_y"] + gl["h"] - 12))
                time.sleep(0.6)
                E.grab_screen(b)
                sel = E.img_diff_box(a, b, *message_panel_box(frame))["changed_pixels"]
                after_click = field_changed(h, message_panel_box(frame), lambda: E.screen_click(gl["root_x"] + 30, gl["root_y"] + gl["h"] - 40, 1), "b_drag_after")
                res["left_drag"] = {"held_ms": held, "selected_px": sel, "next_click_acts_px": after_click}
                deselect()
                backplane_keys("composer")
                n0 = len([w for w in kicad_toplevels() if w["visible"] and w["class"] == "#32768"])
                E.screen_click(cx_ + 60, cy_ + 40, 3)
                time.sleep(1.0)
                menus = [w for w in kicad_toplevels() if w["visible"] and w["class"] == "#32768"]
                res["right_click"] = {"menus_opened": len(menus) - n0}
                E.key_tap("Escape"); time.sleep(0.5)
                deselect(); backplane_keys("composer")
                a, b, c2 = h.p("b_mid_0.png"), h.p("b_mid_1.png"), h.p("b_mid_2.png")
                E.grab_screen(a)
                held = press(0x20, 0x40, cx_, cy_, 0.3, (cx_ + 120, cy_ + 80))
                time.sleep(0.6)
                E.grab_screen(b)
                E.screen_move(cx_ - 150, cy_ - 100)
                time.sleep(0.6)
                E.grab_screen(c2)
                res["middle_drag"] = {"held_ms": held, "panned_px": E.img_diff_box(a, b, gl["root_x"], gl["root_y"], gl["w"], gl["h"])["changed_pixels"],
                                      "after_release_move_px": E.img_diff_box(b, c2, gl["root_x"] + 40, gl["root_y"] + 40, gl["w"] - 80, gl["h"] - 80)["changed_pixels"]}
                if focus_snapshot(frame)["inside_frame"]:
                    E.key_tap("Home")
                time.sleep(0.6)
                deselect(); backplane_keys("composer")
                E.screen_move(cx_, cy_)
                E._send(E._mouse(0x0800, -120)); time.sleep(0.4); E._send(E._mouse(0x0800, 120)); time.sleep(0.4)
                res["composer_after_wheel"] = {"key_ms": key_latency_ms(h, cbox, "w", "b_wheel_type"), **focus_snapshot(frame)}
                print("DIAG stage2_buttons " + json.dumps(res), flush=True)
                h.check("Stage 2 single click: U1 selected, no dialog", res["single"]["selected_px"] > 200 and res["single"]["popups_opened"] == 0, res["single"])
                h.check("Stage 2 real double-click: at most one Properties dialog (no triple press)", res["double"]["popups_opened"] <= 1, res["double"])
                h.check("Stage 2 left drag ~300 ms: selects, and the next plain click acts (no stuck drag)", res["left_drag"]["selected_px"] > 200 and res["left_drag"]["next_click_acts_px"] > 200, res["left_drag"])
                h.check("Stage 2 right click: KiCad's context menu opens once", res["right_click"]["menus_opened"] == 1, res["right_click"])
                h.check("Stage 2 middle drag pans, and stops at release", res["middle_drag"]["panned_px"] > 2000, res["middle_drag"])
                h.check("Wheel over the pane does not take the composer's keys", res["composer_after_wheel"]["key_ms"] is not None and res["composer_after_wheel"]["focus_pid"] == cp.pid, res["composer_after_wheel"])
            elif only == "stage2_middle":
                # Lead, Nov 15: a middle drag that starts on the pane while Backplane has the keys pans, 10/10 at holds of
                # 300 ms and of 1 s (the candidate panned 1 in 3). Judged by the canvas: a pan redraws most of it.
                rows = []
                for hold in (0.3, 1.0):
                    for n in range(1, 11):
                        deselect(); backplane_keys("composer")
                        a, b = h.p(f"mid_{int(hold*1000)}_{n}_0.png"), h.p(f"mid_{int(hold*1000)}_{n}_1.png")
                        E.grab_screen(a)
                        held = press(0x20, 0x40, cx_, cy_, hold, (cx_ + 150, cy_ + 100))
                        time.sleep(0.6)
                        E.grab_screen(b)
                        panned = E.img_diff_box(a, b, gl["root_x"], gl["root_y"], gl["w"], gl["h"])["changed_pixels"]
                        rows.append({"hold_s": hold, "n": n, "held_ms": held, "panned_px": panned, "ok": panned > 20000})
                gest = [l.strip()[9:] for l in open(h.p("client.log"), encoding="utf-8", errors="replace") if '"kicad_middle_gesture"' in l or '"kicad_middle_move"' in l]
                print("DIAG stage2_middle " + json.dumps({"rows": rows, "gesture_lines": gest[:12]})[:4000], flush=True)
                for hold in (0.3, 1.0):
                    rs = [r for r in rows if r["hold_s"] == hold]
                    h.check(f"Middle drag held {int(hold*1000)} ms from Backplane's keys pans, 10/10", all(r["ok"] for r in rs), [(r["n"], r["panned_px"]) for r in rs])
            elif only == "stage2_sweep":
                # Lead, Nov 15: a press on the shield released about when the replay fires must still reach KiCad as a
                # whole click (SB2: a 60 ms right click got up-before-down, no menu). (i) holds of 40/55/60/70/120 ms for
                # each button, (ii) 20 right clicks held 60 ms, (iii) a first press on a non-canvas part of the frame
                # (infobar, else the toolbar strip) and then a right click on the canvas. Every press starts with Backplane
                # holding the keys, so it goes through the shield.
                def hold_click(dn, up, x, y, ms):
                    E.screen_move(x, y)
                    E._send(E._mouse(dn))
                    t0 = time.perf_counter()
                    while (time.perf_counter() - t0) * 1000 < ms:
                        pass
                    E._send(E._mouse(up))
                    return round((time.perf_counter() - t0) * 1000, 1)
                def menus():
                    return len([w for w in kicad_toplevels() if w["visible"] and w["class"] == "#32768"])
                def right_once(x, y, ms):
                    deselect(); backplane_keys("composer")
                    n0 = menus()
                    held = hold_click(8, 0x10, x, y, ms)
                    time.sleep(1.0)
                    n = menus() - n0
                    E.key_tap("Escape"); time.sleep(0.4)
                    return {"held_ms": held, "menus": n}
                sweep = {"left": [], "right": [], "middle": []}
                for ms in (40, 55, 60, 70, 120):
                    deselect(); backplane_keys("composer")
                    box = message_panel_box(frame)
                    a, b = h.p(f"sw_l{ms}_0.png"), h.p(f"sw_l{ms}_1.png")
                    E.grab_screen(a)
                    held = hold_click(2, 4, cx_, cy_, ms)
                    time.sleep(0.8)
                    E.grab_screen(b)
                    sweep["left"].append({"hold": ms, "held_ms": held, "selected_px": E.img_diff_box(a, b, *box)["changed_pixels"]})
                    sweep["right"].append({"hold": ms, **right_once(cx_ + 60, cy_ + 40, ms)})
                    deselect(); backplane_keys("composer")
                    held = hold_click(0x20, 0x40, cx_, cy_, ms)
                    time.sleep(0.5)
                    a, b = h.p(f"sw_m{ms}_0.png"), h.p(f"sw_m{ms}_1.png")
                    E.grab_screen(a)
                    E.screen_move(cx_ + 120, cy_ + 60)
                    time.sleep(0.6)
                    E.grab_screen(b)
                    moved = E.img_diff_box(a, b, gl["root_x"], gl["root_y"], gl["w"], gl["h"])["changed_pixels"]
                    sweep["middle"].append({"hold": ms, "held_ms": held, "move_after_px": moved})
                twenty = [right_once(cx_ + 60, cy_ + 40, 60)["menus"] for _ in range(20)]
                # (iii) a first press off the canvas: the infobar's text if KiCad shows one, else the toolbar strip above
                # the canvas, then the canvas
                strip_y = gl["root_y"] + 12 if any(c.get("class") == "Static" and c.get("visible") and gl["root_y"] <= c.get("root_y", -1) <= gl["root_y"] + 40
                                                   for c in E.list_child_windows(frame)) else gl["root_y"] - 30
                deselect(); backplane_keys("composer")
                first = hold_click(2, 4, gl["root_x"] + gl["w"] // 2, strip_y, 80)
                time.sleep(0.6)
                n0 = menus()
                E.screen_click(cx_ + 60, cy_ + 40, 3)
                time.sleep(1.0)
                after_first = {"first_press_y": strip_y, "first_held_ms": first, "menus": menus() - n0}
                E.key_tap("Escape"); time.sleep(0.4)
                lost = sum(1 for l in open(h.p("client.log"), encoding="utf-8", errors="replace") if "kicad_capture_lost" in l)
                res = {"sweep": sweep, "right_60ms_x20": twenty, "first_press_off_canvas": after_first, "capture_lost": lost}
                print("DIAG stage2_sweep " + json.dumps(res)[:4000], flush=True)
                for ms_row in sweep["left"]:
                    h.check(f"Sweep: left click held {ms_row['hold']} ms selects the item", ms_row["selected_px"] > 200, ms_row)
                for ms_row in sweep["right"]:
                    h.check(f"Sweep: right click held {ms_row['hold']} ms opens one menu", ms_row["menus"] == 1, ms_row)
                for ms_row in sweep["middle"]:
                    h.check(f"Sweep: middle click held {ms_row['hold']} ms leaves no pan behind", ms_row["move_after_px"] < 20000, ms_row)
                h.check("20 right clicks held 60 ms each open one menu", twenty == [1] * 20, twenty)
                h.check("A first press off the canvas (infobar or toolbar), then a right click on the canvas opens one menu",
                        after_first["menus"] == 1, after_first)
            elif only == "stage2":
                rounds = max(1, int(os.environ.get("BP_STAGE2_ROUNDS", "5") or 5))
                for entry in ("composer", "other_app"):
                    for n in range(1, rounds + 1):
                        deselect()
                        backplane_keys(entry)
                        before = shield_clicks()
                        a, b = h.p(f"s2click_{entry}{n}_0.png"), h.p(f"s2click_{entry}{n}_1.png")
                        E.grab_screen(a)
                        around = traced_click(cx_, cy_)
                        fg = foreground_never_lost(500)
                        E.grab_screen(b)
                        panel = E.img_diff_box(a, b, *message_panel_box(frame))["changed_pixels"]
                        px, z = plain_key_acts(h, gl, f"s2_{entry}{n}", frame)
                        d = {"selected_panel_px": panel, "shield_clicks": shield_clicks() - before, "f2_acts": z, **fg, **around}
                        print(f"DIAG stage2_{entry}_{n} " + json.dumps(d)[:700], flush=True)
                        h.check(f"Stage 2 from {entry} {n}/{rounds}: one click on U1 both hands in and selects it, F2 then acts, foreground never lost",
                                panel > 200 and bool(z) and fg["no_foreground_samples"] == 0, d)
            else:
                res = {}
                # wheel over the pane while Backplane has the keys: does KiCad zoom?
                deselect(); backplane_keys("composer")
                E.screen_move(cx_, cy_)
                time.sleep(0.3)
                res["wheel_zoom_px"] = field_changed(h, zoom_field_box(frame), lambda: E._send(E._mouse(0x0800, -120)), "beh_wheel")
                res["wheel_keys_after"] = focus_snapshot(frame)["focus_class"]
                E._send(E._mouse(0x0800, 120))
                time.sleep(0.5)
                # hover: does KiCad follow the pointer (status-bar coordinates)?
                deselect(); backplane_keys("composer")
                E.screen_move(cx_ - 80, cy_ - 60)
                time.sleep(0.3)
                res["hover_coords_px"] = field_changed(h, coord_field_box(frame), lambda: E.screen_move(cx_ + 90, cy_ + 70), "beh_hover", 0.6)
                res["hover_keys_after"] = focus_snapshot(frame)["focus_class"]
                # a drag starting on the pane (a selection box over the whole board): does KiCad select?
                deselect(); backplane_keys("composer")
                a, b = h.p("beh_drag_0.png"), h.p("beh_drag_1.png")
                E.grab_screen(a)
                E.screen_drag(gl["root_x"] + 12, gl["root_y"] + 12, gl["root_x"] + gl["w"] - 12, gl["root_y"] + gl["h"] - 12)
                fgd = foreground_never_lost(400)
                E.grab_screen(b)
                res["drag_selects_px"] = E.img_diff_box(a, b, *message_panel_box(frame))["changed_pixels"]
                res["drag_no_foreground_samples"] = fgd["no_foreground_samples"]
                res["drag_keys_after"] = focus_snapshot(frame)["focus_class"]
                deselect()
                print("DIAG pane_behaviours " + json.dumps(res), flush=True)
                h.check("Pane behaviours recorded (diagnostic: wheel, hover, drag while Backplane has the keys)", True, res)
            paths = {}
            waits = []
            for l in open(h.p("client.log"), encoding="utf-8", errors="replace"):
                if '"kicad_replay"' in l:
                    try:
                        r = json.loads(l.strip()[9:])["kicad_replay"]
                        paths[r["path"]] = paths.get(r["path"], 0) + 1
                        waits.append(r["waited_ms"])
                    except (ValueError, KeyError):
                        pass
            print("DIAG replay_paths " + json.dumps({"paths": paths, "waited_ms": waits}), flush=True)
            try:
                np_proc.kill()
            except OSError:
                pass
            return h.finish()
        if only == "away_return":
            # The "focus elsewhere leaves the state alone" rule: away to another app (Notepad) and back, three ways.
            import subprocess
            gls = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
            gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
            top = E.find_main_window(cp.pid)
            cbox = composer_box(h, top)
            E.INPUT_EXES.add("notepad.exe")  # this mode's own Notepad may take input; the pre-run check makes sure no other is open
            np_proc = subprocess.Popen(["notepad.exe"])
            np = 0
            for _ in range(50):
                np = E.find_main_window(np_proc.pid)
                if np:
                    break
                time.sleep(0.2)
            E.u32.SetWindowPos(np, 0, 1480, 120, 420, 500, 0x0040)
            tx, ty, tw, th = window_box(top)
            def snap(tag):
                d = focus_snapshot(frame)
                d["foreground_title"] = window_title(d["foreground"]) if d["foreground"] else ""
                return d
            def shield_clicks():
                return sum(1 for l in open(h.p("client.log"), encoding="utf-8", errors="replace") if "kicad_shield_click" in l)
            def canvas_click(at):
                box = message_panel_box(frame)
                a, b = h.p(f"away_{at}_0.png"), h.p(f"away_{at}_1.png")
                before = shield_clicks()
                E.grab_screen(a)
                x, y = (gl["root_x"] + gl["w"] // 2, gl["root_y"] + gl["h"] // 2) if at.endswith("u1") else (gl["root_x"] + 30, gl["root_y"] + gl["h"] - 40)
                around = traced_click(x, y)
                fg = foreground_never_lost(500)
                E.grab_screen(b)
                return {"panel_px": E.img_diff_box(a, b, *box)["changed_pixels"], "consumed_by_shield": shield_clicks() - before, **fg, **around}
            def away_and_back(how):
                E.screen_click(1480 + 200, 120 + 250, 1)
                time.sleep(0.6)
                away = snap("away")
                if how == "title":
                    E.screen_click(tx + tw // 2, ty + 12, 1)
                    time.sleep(0.6)
                return away
            res = {"notepad": np}
            # 1. KiCad had the keys (Edit), away, back by Backplane's title bar
            r1 = {"kicad_first": snap("k")}
            r1["away"] = away_and_back("title")
            r1["back"] = snap("back")
            px, z = plain_key_acts(h, gl, "away1", frame)
            r1["f2_acts"] = z
            r1["canvas_click_u1"] = canvas_click("1u1")
            res["kicad_had_keys"] = r1
            # 2. Backplane had the keys (composer), away, back by the title bar, type in the composer, then one canvas click
            keys_back_to_backplane(h, cp, frame, top, "composer", cbox)
            time.sleep(0.3)
            r2 = {"backplane_first": snap("b")}
            r2["away"] = away_and_back("title")
            r2["back"] = snap("back")
            r2["composer_key_ms"] = key_latency_ms(h, cbox, "q", "away2")
            r2["canvas_click_empty"] = canvas_click("2empty")
            px, z = plain_key_acts(h, gl, "away2", frame)
            r2["f2_acts_after_click"] = z
            res["backplane_had_keys"] = r2
            # 3. Backplane had the keys, away, back by clicking straight on the canvas
            keys_back_to_backplane(h, cp, frame, top, "composer", cbox)
            time.sleep(0.3)
            r3 = {"backplane_first": snap("b3"), "away": away_and_back("none")}
            r3["canvas_click_u1_from_other_app"] = canvas_click("3u1")
            r3["after"] = snap("after3")
            px, z = plain_key_acts(h, gl, "away3", frame)
            r3["f2_acts"] = z
            res["back_by_canvas"] = r3
            # 4. KiCad had the keys, Alt+Tab away to Notepad and Alt+Tab back, no click: the keys should be where they were
            give_kicad_keys(h, cp, frame)
            time.sleep(0.4)
            r4 = {"kicad_first": snap("k4")}
            E.u32.SetForegroundWindow(np)  # Notepad next in the Alt+Tab order
            time.sleep(0.3)
            give_kicad_keys(h, cp, frame)
            r4["kicad_again"] = snap("k4b")
            E.key_chord("alt", "Tab")
            time.sleep(0.8)
            r4["away"] = snap("away4")
            E.key_chord("alt", "Tab")
            time.sleep(0.8)
            r4["back"] = snap("back4")
            px, z = plain_key_acts(h, gl, "away4", frame)
            r4["f2_acts_no_click"] = z
            res["alt_tab"] = r4
            try:
                np_proc.kill()
            except OSError:
                pass
            print("DIAG away_return " + json.dumps(res)[:8000], flush=True)
            # A title-bar click is a click in Backplane and may fairly give Backplane the keys; then the next canvas click must
            # bring them back to KiCad (with stage 2 it also acts). KiCad keeping them is fine too. Never stuck either way.
            back_kicad = r1["back"]["inside_frame"] and bool(r1["f2_acts"])
            back_backplane = r1["back"]["focus_pid"] == cp.pid and r1["canvas_click_u1"]["no_foreground_samples"] == 0
            h.check("1. KiCad had the keys, away and back by the title bar: KiCad still has them, or Backplane does and the next canvas click returns them",
                    back_kicad or back_backplane, r1)
            h.check("2. Backplane had the keys, away and back by the title bar: composer types, one canvas click hands in (shield), F2 then acts",
                    r2["composer_key_ms"] is not None and r2["canvas_click_empty"]["consumed_by_shield"] == 1 and bool(r2["f2_acts_after_click"]) and r2["canvas_click_empty"]["no_foreground_samples"] == 0, r2)
            h.check("3. Back from another app by clicking the canvas: KiCad gets the keys (F2 acts), foreground never empty",
                    bool(r3["f2_acts"]) and r3["canvas_click_u1_from_other_app"]["no_foreground_samples"] == 0, r3)
            # 4 is reported, not a blocker for stage 1 (the lead's follow-up if it fails)
            print("DIAG alt_tab_keys_back " + json.dumps({"f2_acts_no_click": r4["f2_acts_no_click"], "away_fg": r4["away"].get("foreground_title"), "back": {k: r4["back"].get(k) for k in ("foreground_title", "focus_class", "inside_frame")}}), flush=True)
            return h.finish()
        if only == "edit_first_click":
            # F3 shield, point 3: after Edit (KiCad has the keys), the first click on the canvas must reach KiCad, not be
            # consumed by the shield: a click on the board's centre (U1) changes the message panel to the item's details.
            gls = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
            gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
            time.sleep(float(os.environ.get("BP_FIRST_CLICK_AFTER_S", "3")))
            box = message_panel_box(frame)
            a, b = h.p("first_click_0.png"), h.p("first_click_1.png")
            E.grab_screen(a)
            around = traced_click(gl["root_x"] + gl["w"] // 2, gl["root_y"] + gl["h"] // 2) if gl else {}
            time.sleep(0.8)
            E.grab_screen(b)
            px = E.img_diff_box(a, b, *box)["changed_pixels"]
            consumed = [l.strip()[9:] for l in open(h.p("client.log"), encoding="utf-8", errors="replace") if "kicad_shield_click" in l]
            flips = [l.strip()[9:] for l in open(h.p("client.log"), encoding="utf-8", errors="replace") if "kicad_keys_flip" in l or '"kicad_shield"' in l]
            d = {"message_panel_px": px, "shield_consumed": consumed, "keys_flips": flips[:8], **around}
            print("DIAG edit_first_click " + json.dumps(d)[:2500], flush=True)
            h.check("After Edit, the first canvas click acts in KiCad (message panel shows the clicked item), not consumed by the shield",
                    px > 200 and not consumed, d)
            E.key_tap("Escape")
            return h.finish()
        if only == "composer_prewarm":
            # The reclaim's own job, kept: Backplane has the keys (composer), a pre-warm starts (a resize re-places the pane,
            # which starts it on a runtime without prewarm-keeps-focus); typing straight through it, every key must land in
            # the composer, none in the hidden schematic, and focus end in the composer.
            top = E.find_main_window(cp.pid)
            cbox = composer_box(h, top)
            time.sleep(3.0)
            keys_back_to_backplane(h, cp, frame, top, "composer", cbox)
            time.sleep(0.4)
            def prewarm_lines():
                try:
                    return open(h.p("client.log"), encoding="utf-8", errors="replace").read().count("editor.kicad.prewarm")
                except OSError:
                    return 0
            before_pw = prewarm_lines()
            wx, wy, ww, wh = window_box(top)
            E.u32.SetWindowPos(top, 0, wx, wy, ww - 40, wh, 0x0004 | 0x0010)  # SWP_NOZORDER | SWP_NOACTIVATE
            lat = [key_latency_ms(h, cbox, c, f"cp{i}") for i, c in enumerate("prewarmk")]
            time.sleep(1.0)
            E.u32.SetWindowPos(top, 0, wx, wy, ww, wh, 0x0004 | 0x0010)
            log = open(h.p("client.log"), encoding="utf-8", errors="replace").read()
            hidden = sorted(set(w["title"] for w in kicad_toplevels() + E.list_child_windows(top) if "Schematic Editor" in (w.get("title") or "")))
            end = focus_snapshot(frame)
            d = {"prewarm_started": prewarm_lines() > before_pw, "key_latency_ms": lat, "stray_events": log.count("kicad_stray_foreground"),
                 "hidden_schematic_titles": hidden, "focus_end": {k: end.get(k) for k in ("focus_class", "focus_pid", "inside_frame")}}
            print("DIAG composer_prewarm " + json.dumps(d), flush=True)
            h.check("Backplane had the keys, a pre-warm starts, typing through it: every key in the composer, schematic untouched, focus ends in the composer",
                    d["prewarm_started"] and all(v is not None for v in lat) and not any(t.lstrip().startswith("*") for t in hidden)
                    and end["focus_pid"] == cp.pid and not end["inside_frame"], d)
            return h.finish()
        if only == "steal_key":
            # A plain key pressed inside the ~0.4 s pre-warm steal (triggered on the client's pre-warm request): does it
            # reach the hidden Schematic Editor (a dialog opens, the "*" mark), the PCB editor, or nothing?
            key = os.environ.get("BP_STEAL_KEY", "a")
            top = E.find_main_window(cp.pid)
            zbox = zoom_field_box(frame)
            t0 = time.monotonic()
            seen = False
            while time.monotonic() - t0 < 30:
                try:
                    if "editor.kicad.prewarm" in open(h.p("client.log"), encoding="utf-8", errors="replace").read():
                        seen = True
                        break
                except OSError:
                    pass
                time.sleep(0.02)
            time.sleep(float(os.environ.get("BP_STEAL_DELAY_S", "0.2")))  # the steal lands ~100-160 ms after the request
            pre_titles = sorted(set(w["title"] for w in kicad_toplevels() + E.list_child_windows(top) if "Schematic Editor" in (w.get("title") or "")))
            a = h.p("steal_key_0.png")
            E.grab_screen(a)
            at = focus_snapshot(frame)
            at["foreground_title"] = window_title(at["foreground"]) if at["foreground"] else ""
            E.key_tap(key)
            time.sleep(2.0)
            b = h.p("steal_key_1.png")
            E.grab_screen(b)
            pops = [(w["class"], w["title"]) for w in kicad_toplevels() if w["visible"] and w["class"] in ("#32770", "wxWindowNR") and "Editor" not in w["title"] and "KiCad" not in w["title"]]
            post_titles = sorted(set(w["title"] for w in kicad_toplevels() + E.list_child_windows(top) if "Schematic Editor" in (w.get("title") or "")))
            d = {"key": key, "prewarm_seen": seen, "focus_when_pressed": {k: at.get(k) for k in ("focus_class", "inside_frame", "foreground_title")},
                 "pcb_zoom_readout_changed_px": E.img_diff_box(a, b, *zbox)["changed_pixels"], "kicad_dialogs_after": pops,
                 "schematic_titles_before": pre_titles, "schematic_titles_after": post_titles,
                 "schematic_modified": any(t.lstrip().startswith("*") for t in post_titles)}
            print("DIAG steal_key " + json.dumps(d), flush=True)
            for _ in pops:
                E.key_tap("Escape"); time.sleep(0.5)
            if os.environ.get("BP_IDLE_REQUIRE_PREWARM") == "1":
                pcb_dialog = any("Footprint Chooser" in t for _, t in pops)
                h.check("steal_key: the pre-warm was exercised (its request seen; else: not exercised)", seen, {"prewarm_seen": seen})
                h.check("steal_key: a plain key 0.2 s into the pre-warm acts in the shown PCB editor (its Footprint Chooser), the hidden schematic untouched",
                        seen and (pcb_dialog or d["pcb_zoom_readout_changed_px"] > 0) and not d["schematic_modified"]
                        and bool(d["focus_when_pressed"].get("inside_frame")), d)
            else:
                h.check("Steal-window key recorded (diagnostic)", True, d)
            return h.finish()
        if only == "idle_trace":
            # Edit, then no input: sample foreground/focus every 20 ms until 8 s after the Edit click, then plain F2.
            # Did focus ever leave the shown editor, when, to what, and did it come back?
            t_end = t_edit + 8.0
            t_cap = t_edit + 20.0
            prev = None
            # BP_IDLE_REPLACE_S: that long after Edit, the window 2 px wider (no activation, no input), a re-place: a runtime
            # without `prewarm-keeps-focus` gets its pre-warm only on a re-place (the client's gate), so this asks for it there
            replace_at = float(os.environ.get("BP_IDLE_REPLACE_S", "0") or 0)
            replaced = False
            while time.monotonic() < (min(max(t_end, t_prewarm + 3.0), t_cap) if t_prewarm else t_end) or (not t_prewarm and time.monotonic() < t_cap and os.environ.get("BP_IDLE_REQUIRE_PREWARM") == "1"):
                if replace_at and not replaced and time.monotonic() >= t_edit + replace_at and top:
                    r = E.W.RECT()
                    E.u32.GetWindowRect(top, E.ctypes.byref(r))
                    E.u32.SetWindowPos(top, 0, 0, 0, r.right - r.left + 2, r.bottom - r.top, 0x0002 | 0x0004 | 0x0010)  # NOMOVE|NOZORDER|NOACTIVATE
                    replaced = True
                    timeline.append({"stage": "replaced", "ms": round((time.monotonic() - started) * 1000)})
                if not t_prewarm and time.monotonic() - last_log_poll > 0.2:
                    last_log_poll = time.monotonic()
                    try:
                        if "editor.kicad.prewarm" in open(h.p("client.log"), encoding="utf-8", errors="replace").read():
                            t_prewarm = time.monotonic()
                            timeline.append({"stage": "prewarm_requested", "ms": round((t_prewarm - started) * 1000)})
                    except OSError:
                        pass
                sn = focus_snapshot(frame)
                if sn != prev:
                    timeline.append({"ms": round((time.monotonic() - started) * 1000), **sn})
                    prev = sn
                time.sleep(0.02)
            gls = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
            gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
            px, zoomed = plain_key_acts(h, gl, "idle_trace", frame) if gl else (0, False)
            edit_ms = round((t_edit - started) * 1000) if t_edit else None
            pw_ms = round((t_prewarm - started) * 1000) if t_prewarm else None
            settle_ms = next((x["ms"] for x in timeline if "ms" in x and "stage" not in x and x["ms"] >= (edit_ms or 0)
                              and x.get("focus") and (x["focus"] == frame or E.u32.IsChild(frame, x["focus"]))), None)
            changes = []
            for x in timeline:
                if "stage" in x or "ms" not in x or edit_ms is None or x["ms"] < edit_ms:
                    continue
                f = x.get("focus") or 0
                inside = bool(f and frame and (f == frame or E.u32.IsChild(frame, f)))
                changes.append({"ms_after_edit": x["ms"] - edit_ms, "ms_after_prewarm": (x["ms"] - pw_ms) if pw_ms else None,
                                "inside_shown": inside, "focus_class": x.get("focus_class"),
                                "foreground_title": window_title(x["foreground"]) if x.get("foreground") else ""})
            got = [c for c in changes if c["inside_shown"]]
            left = [c for c in changes if got and c["ms_after_edit"] > got[0]["ms_after_edit"] and not c["inside_shown"]]
            back = bool(left and any(c["inside_shown"] and c["ms_after_edit"] > left[0]["ms_after_edit"] for c in changes))
            d = {"prewarm_ms_after_edit": (pw_ms - edit_ms) if pw_ms and edit_ms is not None else None,
                 "first_in_shown_ms": got[0]["ms_after_edit"] if got else None,
                 "left_shown": bool(left), "first_left": left[0] if left else None, "came_back": back,
                 "f2_acts": bool(zoomed), "changes": changes[:40]}
            print("DIAG idle_trace " + json.dumps(d)[:5000], flush=True)
            if os.environ.get("BP_IDLE_REQUIRE_PREWARM") == "1":
                # the lead's bar for the veto (Nov 15): the pre-warm must really run (a runtime that does not advertise
                # prewarm-keeps-focus never starts it), and after it no focus sample may be outside the shown editor
                after_pw = [c for c in changes if c["ms_after_prewarm"] is not None and c["ms_after_prewarm"] >= 0 and not c["inside_shown"]]
                d["outside_after_prewarm"] = after_pw[:6]
                h.check("The idle pre-warm was exercised: requested within 20 s of Edit and sampled 3 s past it (else: not exercised)",
                        d["prewarm_ms_after_edit"] is not None, {"prewarm_ms_after_edit": d["prewarm_ms_after_edit"]})
                h.check("Edit, no input: zero focus samples outside the shown editor after the pre-warm, and F2 acts",
                        d["prewarm_ms_after_edit"] is not None and bool(zoomed) and not after_pw, d)
                return h.finish()
            h.check("Edit, no input for 8 s: the keys stay in the shown editor (F2 acts)", bool(zoomed) and not (left and not back), d)
            return h.finish()
        if only == "idle_hotkey":
            # The self-started pre-warm, slowed on purpose (BP_PREWARM_STALL_S: KiCad suspended that long from the client's
            # pre-warm request, so its frames are built and any steal lands well after the opening hand-in), then no input
            # until 15 s after the stall, then a plain hotkey with no click: the shown editor must act, the schematic untouched.
            gls = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
            gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
            top = E.find_main_window(cp.pid)
            stall = float(os.environ.get("BP_PREWARM_STALL_S", "0") or 0)
            # KiCad suspended from before the client's pre-warm request (it asks ~2 s after the first placement), so the
            # pre-warmed frame is built and shown only after the resume: the steal lands ~stall s after the opening hand-in,
            # outside verify_handin's 1.5 s window
            stalled = any(x.get("stage") == "stall_started" and x.get("ok") for x in timeline)
            seen = wait_prewarm(h, top, "during", 20)
            time.sleep(15)
            snap = focus_snapshot(frame)
            if snap["foreground"]:
                snap["foreground_title"] = window_title(snap["foreground"])
            px, zoomed = plain_key_acts(h, gl, "idle_hotkey", frame) if gl else (0, False)
            hidden = sorted(set(w["title"] for w in kicad_toplevels() + E.list_child_windows(top) if "Schematic Editor" in (w.get("title") or "")))
            stray = [l.strip()[9:] for l in open(h.p("client.log"), encoding="utf-8", errors="replace") if "kicad_stray_foreground" in l]
            d = {"stall_s": stall, "stalled": stalled, "prewarm": seen, "before_hotkey": snap, "zoom_readout_changed": zoomed,
                 "hidden_schematic_titles": hidden, "guard_fired": stray}
            print("DIAG idle_hotkey " + json.dumps(d)[:2000], flush=True)
            h.check(f"Slowed idle pre-warm ({stall:g} s), no input, then a plain hotkey: the shown editor acts, hidden schematic untouched",
                    snap["inside_frame"] and bool(zoomed) and not any(t.lstrip().startswith("*") for t in hidden), d)
            return h.finish()
        if only == "idle_prewarm_tab":
            # Lane B's idle pre-warm (46343fe7e): Edit, 10 s with no input (pre-warm starts by itself), then the first
            # editor-tab click and the first canvas hotkey: the keys must be the shown editor's, the hidden schematic untouched.
            gls = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
            gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
            top = E.find_main_window(cp.pid)
            time.sleep(10)
            before = focus_snapshot(frame)
            pw = prewarm_frames(top)
            h.click_control(top, "kicad-tab:PCB")
            time.sleep(0.6)
            after = focus_snapshot(frame)
            if after["foreground"]:
                after["foreground_title"] = window_title(after["foreground"])
                after["foreground_class"] = E.hwnd_class(after["foreground"])
            fg = foreground_never_lost(400)
            px, zoomed = plain_key_acts(h, gl, "idle_prewarm_tab", frame) if gl else (0, False)
            hidden = sorted(set(w["title"] for w in kicad_toplevels() + E.list_child_windows(top) if "Schematic Editor" in (w.get("title") or "")))
            stray = [l.strip() for l in open(h.p("client.log"), encoding="utf-8", errors="replace") if "kicad_stray_foreground" in l]
            d = {"before_click": before, "prewarmed_seen": [(w["hwnd"], w["title"]) for w in pw], "after_click": after, **fg,
                 "zoom_readout_changed": zoomed, "hidden_schematic_titles": hidden, "guard_fired": stray}
            print("DIAG idle_prewarm_tab " + json.dumps(d)[:2000], flush=True)
            if os.environ.get("BP_IDLE_REQUIRE_PREWARM") == "1":
                asked = "editor.kicad.prewarm" in open(h.p("client.log"), encoding="utf-8", errors="replace").read()
                h.check("first tab: the idle pre-warm was exercised before the click (its request seen; else: not exercised)", asked, {"prewarm_requested": asked})
            h.check("Idle pre-warm, then first tab click and first hotkey: the shown editor gets the keys (F2 by the readout), hidden schematic untouched",
                    after["inside_frame"] and bool(zoomed) and fg["no_foreground_samples"] == 0 and not any(t.lstrip().startswith("*") for t in hidden), d)
            return h.finish()
        if only == "notice_dialogs":
            # Lane B's checks for embed-notice / embed-dialog-events (fork ba5fe711f0, client c0b81fbf8). With
            # BP_EXPECT_NOTICE=1 (a runtime without its software opengl32.dll on a machine with no GPU) KiCad's
            # "Could not use OpenGL..." reaches the pane as a notice, not a box. Then a native modal in KiCad
            # (its Open file dialog, Ctrl+O) is named in the pane's dialogs while up, and gone after Escape. The pane's
            # "KiCad has ... open" wording is only for a dialog before a frame is on show (Lane B), so not looked for here.
            settled_rep = h.last_kicad_report("settled")
            rect_ids = set()
            for m in h.metrics():
                for r in (m.get("rects") if isinstance(m.get("rects"), list) else []):
                    if isinstance(r, dict) and r.get("id"):
                        rect_ids.add(r["id"])
            notice = settled_rep.get("notice")
            box = E.startup_box()
            hub_lines = [l.strip() for l in open(h.p("hub.log"), encoding="utf-8", errors="replace") if "kicad embed: KiCad notice" in l]
            # the capability the hub keys dialogs_reported on (Lane B: true on Windows exactly when it is there); the
            # embedded GUI's own GetCapabilities, as the hub reads it
            sock = settled_rep.get("sock") or ""
            try:
                caps = h.ipc_cmd("caps", sock).get("capabilities") if sock else None
            except Exception as e:  # noqa: BLE001
                caps = f"unreadable: {e}"[:200]
            res = {"notice": notice, "notice_rects": sorted(i for i in rect_ids if i.startswith("kicad:notice")), "box_now": box,
                   "hub_notice_lines": hub_lines[:3], "embed_caps": [c for c in caps if c.startswith("embed")] if isinstance(caps, list) else caps}
            h.check("The runtime reports dialogs (embed-dialog-events: the hub's dialogs_reported) and notices (embed-notice)",
                    isinstance(caps, list) and "embed-dialog-events" in caps and "embed-notice" in caps, res)
            if os.environ.get("BP_EXPECT_NOTICE") == "1":
                h.check("No GPU: KiCad's graphics fallback is a notice in the pane (\"KiCad says: ...\", the hub's \"KiCad notice\" line), and no box is up",
                        bool(notice) and "opengl" in str(notice).lower() and not box and bool(hub_lines)
                        and bool(rect_ids & {"kicad:notice", "kicad:notice-start"}), res)
            keys = give_kicad_keys(h, cp, frame)
            n0 = len(open(h.p("client.log"), encoding="utf-8", errors="replace").read())

            def pane_dialogs():
                """The pane's dialog lists since the press, in order (the client's kicad_info lines)."""
                out = []
                for l in open(h.p("client.log"), encoding="utf-8", errors="replace").read()[n0:].splitlines():
                    if l.startswith("GPUI_LOG ") and '"kicad_info"' in l:
                        try:
                            out.append(json.loads(l[9:]).get("dialogs") or [])
                        except ValueError:
                            pass
                return out

            E.key_chord("ctrl", "o")
            seen_open = None
            end = time.monotonic() + 8
            while time.monotonic() < end and seen_open is None:
                time.sleep(0.25)
                seen_open = next((d for d in pane_dialogs() if d), None)
            h.shot("notice_dialogs_open")
            named = [w["title"] for w in kicad_toplevels() if w["visible"] and w["class"] == "#32770"]
            E.key_tap("Escape")
            seen_closed = False
            end = time.monotonic() + 6
            while time.monotonic() < end and not seen_closed:
                time.sleep(0.25)
                lists = pane_dialogs()
                first = next((i for i, d in enumerate(lists) if d), None)
                seen_closed = first is not None and any(not d for d in lists[first + 1:])
            res.update({"keys": keys, "native_modals": named, "pane_dialogs_while_open": seen_open, "closed_after_escape": seen_closed})
            print("DIAG notice_dialogs " + json.dumps(res)[:3000], flush=True)
            h.check("A native modal in KiCad (Open, Ctrl+O) is named in the pane's dialogs while up, and gone after Escape",
                    bool(named) and bool(seen_open) and bool(seen_closed), res)
            return h.finish()
        if only == "took_race":
            # Lead, Nov 15: a click in the composer within ~300 ms of a canvas click, then typing: the text must land in
            # the composer. KiCad's focus notice from the canvas click used to pull the keyboard back (the X11 `took` rule
            # running on Windows), and the typing went to KiCad. Digits, so a stray key in KiCad opens nothing.
            gls = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
            gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
            top = E.find_main_window(cp.pid)
            cbox = composer_box(h, top)
            rows = []
            def focus_notices():
                return sum(1 for l in open(h.p("client.log"), encoding="utf-8", errors="replace") if '"kicad_ev":"focus' in l)
            for n in range(1, 6):
                before = focus_notices()
                E.screen_click(gl["root_x"] + 30, gl["root_y"] + gl["h"] - 40, 1)  # an empty spot of the board: KiCad's keys
                # the rule only acts on KiCad's focus notice (`took`, from _BACKPLANE_FOCUS) under 600 ms old: wait for it,
                # then the composer 100 ms later (took-race-base, Nov 15: a fixed 250 ms came before the notice, nothing armed)
                t0 = time.monotonic()
                while focus_notices() == before and time.monotonic() - t0 < 1.0:
                    time.sleep(0.02)
                armed = focus_notices() > before
                time.sleep(0.1)
                E.screen_click(cbox[0] + 120, cbox[1] + 30, 1)                  # the composer
                a, b = h.p(f"took_{n}_0.png"), h.p(f"took_{n}_1.png")
                E.grab_screen(a)
                for ch in "123":
                    E.key_tap(ch)
                time.sleep(0.5)
                E.grab_screen(b)
                typed = E.img_diff_box(a, b, *cbox)["changed_pixels"]
                snap = focus_snapshot(frame)
                rows.append({"round": n, "armed": armed, "notice_ms": round((time.monotonic() - t0) * 1000), "composer_px": typed,
                             "focus_pid": snap["focus_pid"], "focus_class": snap["focus_class"], "ok": typed > 150 and snap["focus_pid"] == cp.pid})
                E.key_chord("ctrl", "a"); E.key_tap("BackSpace"); time.sleep(0.2)
                if not rows[-1]["ok"]:
                    E.key_tap("Escape"); time.sleep(0.3)
            print("DIAG took_race " + json.dumps(rows), flush=True)
            armed_n = sum(1 for r in rows if r["armed"])
            h.check("The race was exercised: KiCad's focus notice arrived before the composer click in at least 3 of 5 rounds",
                    armed_n >= 3, {"armed_rounds": armed_n})
            h.check("Canvas click, then the composer under 600 ms after KiCad's focus notice, and typing: the text lands in the composer, 5/5",
                    all(r["ok"] for r in rows), rows)
            return h.finish()
        if only == "plan":
            plan_check(h, cp, frame, rep, timeline)
            return h.finish()
        if only == "f6":
            return f6_check(h, cp, frame, children, timeline, pid)
        if only == "mouse_return":
            return mouse_return_check(h, cp, frame, children, timeline, kpid=pid)
        if not only_click:
            # Keys must be on KiCad's drawing panel (EDA_DRAW_PANEL_GAL, which binds key events to
            # the tool dispatcher) or its wxGLCanvas child: focus in the frame's toolbar Edit box
            # leaves every canvas hotkey dead even though the frame keeps the foreground.
            gls = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
            gl = max(gls, key=lambda w: w["w"] * w["h"]) if gls else None
            panel = int(gl["parent"]) if gl else 0
            on_canvas = bool(snap["focus_pid"] == pid and (snap["focus_class"] == "wxGLCanvas" or (panel and snap["focus"] == panel)))
            # 1. A plain, unmodified canvas hotkey the tool dispatcher handles: F2 (zoom out),
            # seen as the pane's pixels changing; F1 zooms back. Pressed with no click.
            pane = rep.get("container") or {}
            px, py, pw, ph = int(pane.get("root_x") or 0), int(pane.get("root_y") or 0), int(pane.get("w") or 0), int(pane.get("h") or 0)
            if gl:
                px, py, pw, ph = int(gl["root_x"]), int(gl["root_y"]), int(gl["w"]), int(gl["h"])
            zshots = [h.p("zoom_0_before.png"), h.p("zoom_1_after_f2.png")]
            E.grab_screen(zshots[0])
            E.key_tap("F2")
            time.sleep(1.0)
            E.grab_screen(zshots[1])
            zoom = E.img_diff_box(zshots[0], zshots[1], px, py, pw, ph)
            plain_key = E.img_diff_box(zshots[0], zshots[1], *zoom_field_box(frame))["changed_pixels"] > 15
            if plain_key:
                zoom_back()
            print("DIAG plain_key_f2 " + json.dumps({"changed_px": zoom["changed_pixels"], "box": zoom["box"], "focus": focus_snapshot(frame)}), flush=True)
            # 2. A real KiCad shortcut (a frame accelerator), rather than trusting the client's
            # cached `keys` or a ShowFrame/RunAction API call.
            E.key_chord("ctrl", "f")
            dialog = h.wait_kicad_dialog(pid, "Find", timeout_s=6)
            h.shot("02_find_without_click")
            find_detail = {"dialog": dialog, "focus": focus_snapshot(frame)}
            if dialog:
                E.key_tap("Escape")
                time.sleep(0.6)
            h.check("Edit puts the keys on KiCad's drawing panel or its wxGLCanvas without a click: focus there, plain F2 zooms, Ctrl+F opens Find",
                    settled and rep.get("keys") is True and on_canvas and plain_key and bool(dialog),
                    {**timeline[-1], "panel": panel, "on_canvas": on_canvas, "f2_changed_px": zoom["changed_pixels"], "find": bool(dialog)})
            h.check("Ctrl+F immediately after Edit opens KiCad Find without a canvas click", bool(dialog), find_detail)
        E.key_chord("ctrl", "grave")
        deadline = time.monotonic() + 3
        handback = focus_snapshot(frame)
        while time.monotonic() < deadline:
            handback = focus_snapshot(frame)
            if handback["focus_pid"] == cp.pid:
                break
            time.sleep(0.05)
        timeline.append({"stage": "after_handback", **handback})
        top = int(rep.get("toplevel") or 0)
        if os.environ.get("BP_OPENING_FOCUS_HOST_FG") == "1" and top and int(E.u32.GetForegroundWindow() or 0) != top:
            E.u32.SetForegroundWindow(top)
            deadline = time.monotonic() + 2
            while time.monotonic() < deadline and int(E.u32.GetForegroundWindow() or 0) != top:
                time.sleep(0.02)
            timeline.append({"stage": "harness_host_foreground", **focus_snapshot(frame)})
            print("DIAG harness_host_foreground " + json.dumps(timeline[-1]), flush=True)
        if not only_click:
            h.check("Ctrl+` returns actual Win32 keyboard focus to Backplane", handback["focus_pid"] == cp.pid and not handback["inside_frame"], handback)
        h.shot("03_handback")
        # Positive control after the no-click checks: the same runtime can serve
        # the shortcut when the person explicitly clicks its canvas.
        canvases = [w for w in children if w["visible"] and w["class"] == "wxGLCanvas"]
        canvas = max(canvases, key=lambda w: w["w"] * w["h"]) if canvases else None
        control = None
        if canvas:
            around = traced_click(canvas["root_x"] + 20, canvas["root_y"] + canvas["h"] - 30)
            # KiCad's drawing panel (the wxGLCanvas's parent, where the tool dispatcher's key events are bound) or the canvas
            clicked, settle_ms = wait_focus_in(frame, 3)
            if settle_ms is not None and clicked["focus_class"] not in ("wxGLCanvas", "wxWindowNR"):
                settle_ms = None
            timeline.append({"stage": "after_real_canvas_click", "settle_ms": settle_ms, **clicked, **around})
            print("DIAG canvas_click " + json.dumps(timeline[-1]), flush=True)
            if settle_ms is not None:
                E.key_chord("ctrl", "f")
                control = h.wait_kicad_dialog(pid, "Find", timeout_s=3)
        h.check("Control: native canvas click then Ctrl+F opens KiCad Find", bool(control), {"dialog": control, "focus": focus_snapshot(frame)})
        if only_click:
            return h.finish()
        if control:
            E.key_tap("Escape")
            time.sleep(0.3)
        if canvas:
            E.screen_click(canvas["root_x"] + 20, canvas["root_y"] + canvas["h"] - 30, 1)
        before_freeze, _ = wait_focus_in(frame, 3) if canvas else (focus_snapshot(frame), None)
        if not before_freeze["inside_frame"]:
            # F3 (a canvas click from a Backplane foreground leaves no foreground window) would
            # make the freeze check vacuous; the editor's tab gives KiCad the keys instead.
            via = give_kicad_keys(h, cp, frame)
            before_freeze = focus_snapshot(frame)
            timeline.append({"stage": "before_freeze_via_tab", **via})
        suspended = E.suspend_pid(pid, True) if before_freeze["inside_frame"] else False
        rescue = before_freeze
        elapsed = None
        try:
            if suspended:
                start = time.monotonic()
                E.key_chord("ctrl", "grave")
                while time.monotonic() - start < 1.5:
                    rescue = focus_snapshot(frame)
                    if rescue["focus_pid"] == cp.pid:
                        elapsed = round((time.monotonic() - start) * 1000)
                        break
                    time.sleep(0.02)
            h.check("Frozen KiCad: Ctrl+` rescues actual focus within 1.5s and hub answers", suspended and elapsed is not None and h.hub_hello(), {"suspended": suspended, "elapsed_ms": elapsed, **rescue})
        finally:
            if suspended:
                E.suspend_pid(pid, False)

        return ctrl_k_check(h, cp, frame, rep, timeline)
    finally:
        with open(h.p("focus_timeline.json"), "w", encoding="utf-8") as f:
            json.dump(timeline, f, indent=2)
        h.cleanup()


START_FAILURES = ("did not open its IPC socket", "did not answer Ping")


def start_failed(out_dir):
    """KiCad never came up in this run (the hub's start window missed): a slow first scan of new files, not a focus result."""
    try:
        text = open(os.path.join(out_dir, "hub.log"), encoding="utf-8", errors="replace").read()
    except OSError:
        return None
    return next((m for m in START_FAILURES if m in text), None)


def defender_events(minutes=10):
    """Microsoft Defender's operational events of the last minutes (cloud lookups, scans), to set beside a failed start."""
    import subprocess
    ps = ("Get-WinEvent -LogName 'Microsoft-Windows-Windows Defender/Operational' -MaxEvents 40 -EA SilentlyContinue | "
          f"Where-Object {{ $_.TimeCreated -gt (Get-Date).AddMinutes(-{minutes}) }} | "
          "ForEach-Object { '{0:HH:mm:ss} {1}' -f $_.TimeCreated, $_.Id }")
    try:
        return subprocess.run(["powershell", "-NoProfile", "-Command", ps], capture_output=True, text=True, timeout=30).stdout.split()
    except (OSError, subprocess.SubprocessError):
        return []


if __name__ == "__main__":
    if len(sys.argv) not in (6, 7):
        sys.exit(__doc__)
    args = list(sys.argv[1:6])
    port = int(sys.argv[6]) if len(sys.argv) == 7 else 3937
    rc = E.guarded_run(lambda: run(E.SuiteHarness("opening_focus", *args, port)), args[0], args[3])
    why = rc != 3 and start_failed(args[4])
    if why:
        # one retry, in a fresh folder; a second failed start fails the run
        print("START-RETRY " + json.dumps({"why": why, "defender_events_last_10_min": defender_events()}), flush=True)
        args[4] = args[4].rstrip("\\/") + "-retry"
        rc = E.guarded_run(lambda: run(E.SuiteHarness("opening_focus", *args, port)), args[0], args[3])
        if start_failed(args[4]):
            print("START-FAILED-TWICE " + json.dumps({"why": start_failed(args[4])}), flush=True)
            rc = rc or 1
    sys.exit(rc)
