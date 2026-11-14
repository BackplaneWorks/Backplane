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
Set BP_OPENING_FOCUS_HOST_FG=1 to make Backplane's window foreground from the
harness before that click when the handback left KiCad foreground, so clients
are compared from the same Backplane-foreground state.
"""
import json
import os
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


def fg_exe():
    """The foreground window's program name (lower case), "" when none or unreadable."""
    hwnd = int(E.u32.GetForegroundWindow() or 0)
    if not hwnd:
        return ""
    pid = E.W.DWORD()
    E.u32.GetWindowThreadProcessId(hwnd, E.ctypes.byref(pid))
    k32 = E.ctypes.windll.kernel32
    hp = k32.OpenProcess(0x1000, False, pid.value)  # PROCESS_QUERY_LIMITED_INFORMATION
    if not hp:
        return ""
    try:
        buf = E.ctypes.create_unicode_buffer(520)
        n = E.W.DWORD(520)
        ok = k32.QueryFullProcessImageNameW(hp, 0, buf, E.ctypes.byref(n))
        return os.path.basename(buf.value).lower() if ok else ""
    finally:
        k32.CloseHandle(hp)


def zoom_back():
    """F1 (KiCad's zoom in) only while Backplane or KiCad is in front: on Explorer or the desktop F1 opens
    Windows Help, which starts Edge, and Edge then holds the foreground for every later run (bp-win, 21:24)."""
    exe = fg_exe()
    if exe in ("backplane-gpui.exe", "kicad.exe", "pcbnew.exe", "eeschema.exe"):
        E.key_tap("F1")
        time.sleep(0.6)
    else:
        print("DIAG f1_skipped " + json.dumps({"foreground_exe": exe}), flush=True)


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
                if route == "tab":
                    clicked = h.click_control(top, "kicad-tab:PCB")
                    around = {"clicked": clicked}
                    fg = foreground_never_lost()
                else:
                    around = traced_click(gl["root_x"] + 20, gl["root_y"] + gl["h"] - 30)
                    shot = h.p(f"return_{entry}_{route}{n}_50ms.png")
                    E.grab_screen(shot)  # the title bar ~50 ms after the click: inactive = a visible flicker
                    fg = foreground_never_lost(550)
                    around["shot_50ms"] = shot
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


def run(h):
    only = os.environ.get("BP_OPENING_FOCUS_ONLY", "")
    only_click = only == "click_control"
    only_ctrl_k = only == "ctrl_k"
    h.init_dirs()
    timeline = []
    try:
        provenance = {}
        for name, path in (("client", h.client_exe), ("hub", h.hub_exe), ("kicad", h.kicad_bin)):
            provenance[name] = {"path": path, "sha256": E.sha256_file(path)}
        with open(h.p("binaries.json"), "w", encoding="utf-8") as f:
            json.dump(provenance, f, indent=2)
        h.start_hub(embed_mode="fork")
        cp = h.start_client([
            {"select": 0}, {"wait": 2800}, {"click": "rail:Board"},
            {"wait": 2800}, {"mark": "ready_for_edit"}, {"wait": 1800},
            {"kicad": {"wait_painted": 45000, "kind": "PCB", "tag": "first_paint"}},
            {"wait": 2000}, {"kicad": {"report": True, "tag": "settled"}},
            {"mark": "opening_settled"},
        ], hold=True, exit_after=120)
        started = time.monotonic()
        previous = None
        settled = False
        clicked_edit = False
        at_request = None
        trace_idle = os.environ.get("BP_OPENING_FOCUS_ONLY") == "idle_trace"
        t_edit, t_prewarm, last_log_poll = 0.0, 0.0, 0.0
        # idle_hotkey: KiCad suspended from 0.5 s after its first paint (before the client's pre-warm request at ~2 s)
        stall_s = float(os.environ.get("BP_PREWARM_STALL_S", "0") or 0) if os.environ.get("BP_OPENING_FOCUS_ONLY") == "idle_hotkey" else 0
        stall_started, stall_pid = None, 0
        while cp.poll() is None and time.monotonic() - started < 65:
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
            marks = {m.get("name") for m in h.metrics() if m.get("event") == "mark"}
            if trace_idle and t_edit and not t_prewarm and time.monotonic() - last_log_poll > 0.2:
                last_log_poll = time.monotonic()
                try:
                    if "editor.kicad.prewarm" in open(h.p("client.log"), encoding="utf-8", errors="replace").read():
                        t_prewarm = time.monotonic()
                        timeline.append({"stage": "prewarm_requested", "ms": round((t_prewarm - started) * 1000)})
                except OSError:
                    pass
            if "ready_for_edit" in marks and not clicked_edit:
                clicked_edit = h.click_control(E.find_main_window(cp.pid), "ui:Edit")
                timeline.append({"stage": "native_edit_click", "clicked": clicked_edit, **focus_snapshot()})
                t_edit = time.monotonic()
            if "opening_settled" in marks:
                settled = True
                break
            time.sleep(0.02 if trace_idle else 0.05)
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
        if only in ("stage2", "stage2_buttons", "pane_behaviours"):
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
                held = press(2, 4, gl["root_x"] + 12, gl["root_y"] + 12, 0.3, (gl["root_x"] + gl["w"] - 12, gl["root_y"] + gl["h"] - 12))
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
            h.check("Steal-window key recorded (diagnostic)", True, d)
            return h.finish()
        if only == "idle_trace":
            # Edit, then no input: sample foreground/focus every 20 ms until 8 s after the Edit click, then plain F2.
            # Did focus ever leave the shown editor, when, to what, and did it come back?
            t_end = t_edit + 8.0
            prev = None
            while time.monotonic() < t_end:
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
            h.check("Idle pre-warm, then first tab click and first hotkey: the shown editor gets the keys (F2 by the readout), hidden schematic untouched",
                    after["inside_frame"] and bool(zoomed) and fg["no_foreground_samples"] == 0 and not any(t.lstrip().startswith("*") for t in hidden), d)
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
    rc = run(E.SuiteHarness("opening_focus", *args, port))
    why = start_failed(args[4])
    if why:
        # one retry, in a fresh folder; a second failed start fails the run
        print("START-RETRY " + json.dumps({"why": why, "defender_events_last_10_min": defender_events()}), flush=True)
        args[4] = args[4].rstrip("\\/") + "-retry"
        rc = run(E.SuiteHarness("opening_focus", *args, port))
        if start_failed(args[4]):
            print("START-FAILED-TWICE " + json.dumps({"why": start_failed(args[4])}), flush=True)
            rc = rc or 1
    sys.exit(rc)
