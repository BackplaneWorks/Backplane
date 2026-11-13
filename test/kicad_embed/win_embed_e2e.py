#!/usr/bin/env python3
"""Windows E2E suites for first-party embedded Backplane KiCad (bp.8+):
  - window:     HWND parenting (WS_CLIPCHILDREN / WS_CHILD), <=2px geometry, GAL paint,
                focus handoff (WM_APP+0x101 + Ctrl+`), sidebar fold reflow, top-level
                resize & pure-move popup menu sync (#32768), SetWindowRgn overlay holes,
                non-CAD rail hide/restore, Board <-> Schematic <-> 3D warm switching,
                ui:View warm keepalive + close, and Windows stock-mode guidance.
  - editors:    Live Named Pipe GUI IPC, Schematic interactive wire + Ctrl+S + ERC/SVG,
                Symbol Editor, PCB copper zone unfill/refill + track add + SaveDocument,
                Footprint Editor, DRC dialog (GWLP_HWNDPARENT) + CLI DRC, 3D Viewer
                OpenGL render + mouse-drag orbit, Plot dialog + Gerbers/Drill/STEP +
                companion metadata preservation.
  - resilience: Live agent IPC track commit + GAL repaint + human Ctrl+Z undo,
                NtSuspendProcess freeze isolation + RegisterHotKey Ctrl+` focus rescue,
                dirty-close refusal, GPUI client hard-kill behavior & recovery,
                KiCad hard-kill survival + editor.kicad.restart + zero-orphan teardown.

Run inside an interactive Windows desktop session (via test/windows/rig/sess.sh):
  python win_embed_e2e.py <window|editors|resilience|all> <CLIENT_EXE> <HUB_EXE> <ASSETS_DIR> <KICAD_DIR> <OUT_DIR> [PORT]
"""

import ctypes
from ctypes import wintypes as W
import glob
import hashlib
import json
import os
import shutil
import subprocess
import sys
import threading
import time
import urllib.request

if os.name != "nt":
    sys.exit("win_embed_e2e.py must run on Windows")

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
IPC_PY = os.path.join(HERE, "lib", "gui_ipc_probe.py")
SEED_PY = os.path.join(HERE, "lib", "seed_project.py")
HUB_RPC_MJS = os.path.join(HERE, "lib", "hub_rpc.mjs")

# ---- Win32 ctypes bindings (64-bit safe) ----
u32 = ctypes.WinDLL("user32", use_last_error=True)
k32 = ctypes.WinDLL("kernel32", use_last_error=True)
ntdll = ctypes.WinDLL("ntdll")

try:
    ctypes.WinDLL("shcore").SetProcessDpiAwareness(2)
except Exception:
    u32.SetProcessDPIAware()

WNDENUMPROC = ctypes.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)

u32.EnumWindows.argtypes = [WNDENUMPROC, W.LPARAM]
u32.EnumWindows.restype = W.BOOL
u32.EnumChildWindows.argtypes = [W.HWND, WNDENUMPROC, W.LPARAM]
u32.EnumChildWindows.restype = W.BOOL
u32.IsWindow.argtypes = [W.HWND]
u32.IsWindow.restype = W.BOOL
u32.IsWindowVisible.argtypes = [W.HWND]
u32.IsWindowVisible.restype = W.BOOL
u32.IsChild.argtypes = [W.HWND, W.HWND]
u32.IsChild.restype = W.BOOL
u32.GetParent.argtypes = [W.HWND]
u32.GetParent.restype = W.HWND
u32.GetAncestor.argtypes = [W.HWND, W.UINT]
u32.GetAncestor.restype = W.HWND
u32.GetWindow.argtypes = [W.HWND, W.UINT]
u32.GetWindow.restype = W.HWND
u32.GetWindowLongPtrW.argtypes = [W.HWND, ctypes.c_int]
u32.GetWindowLongPtrW.restype = ctypes.c_ssize_t
u32.GetWindowThreadProcessId.argtypes = [W.HWND, ctypes.POINTER(W.DWORD)]
u32.GetWindowThreadProcessId.restype = W.DWORD
u32.GetWindowRect.argtypes = [W.HWND, ctypes.POINTER(W.RECT)]
u32.GetWindowRect.restype = W.BOOL
u32.GetClientRect.argtypes = [W.HWND, ctypes.POINTER(W.RECT)]
u32.GetClientRect.restype = W.BOOL
u32.ClientToScreen.argtypes = [W.HWND, ctypes.POINTER(W.POINT)]
u32.ClientToScreen.restype = W.BOOL
u32.AdjustWindowRectEx.argtypes = [ctypes.POINTER(W.RECT), W.DWORD, W.BOOL, W.DWORD]
u32.AdjustWindowRectEx.restype = W.BOOL
u32.SetWindowPos.argtypes = [W.HWND, W.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, W.UINT]
u32.SetWindowPos.restype = W.BOOL
u32.ShowWindow.argtypes = [W.HWND, ctypes.c_int]
u32.ShowWindow.restype = W.BOOL
u32.SetForegroundWindow.argtypes = [W.HWND]
u32.SetForegroundWindow.restype = W.BOOL
u32.GetWindowTextLengthW.argtypes = [W.HWND]
u32.GetWindowTextLengthW.restype = ctypes.c_int
u32.GetWindowTextW.argtypes = [W.HWND, W.LPWSTR, ctypes.c_int]
u32.GetWindowTextW.restype = ctypes.c_int
u32.GetClassNameW.argtypes = [W.HWND, W.LPWSTR, ctypes.c_int]
u32.GetClassNameW.restype = ctypes.c_int
u32.VkKeyScanW.argtypes = [W.WCHAR]
u32.VkKeyScanW.restype = ctypes.c_short
u32.GetSystemMetrics.argtypes = [ctypes.c_int]
u32.GetSystemMetrics.restype = ctypes.c_int
u32.PostMessageW.argtypes = [W.HWND, W.UINT, W.WPARAM, W.LPARAM]
u32.PostMessageW.restype = W.BOOL

k32.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
k32.OpenProcess.restype = W.HANDLE
k32.CloseHandle.argtypes = [W.HANDLE]
k32.CloseHandle.restype = W.BOOL
k32.TerminateProcess.argtypes = [W.HANDLE, W.UINT]
k32.TerminateProcess.restype = W.BOOL
k32.GetExitCodeProcess.argtypes = [W.HANDLE, ctypes.POINTER(W.DWORD)]
k32.GetExitCodeProcess.restype = W.BOOL
ntdll.NtSuspendProcess.argtypes = [W.HANDLE]
ntdll.NtSuspendProcess.restype = ctypes.c_long
ntdll.NtResumeProcess.argtypes = [W.HANDLE]
ntdll.NtResumeProcess.restype = ctypes.c_long

GWL_STYLE = -16
GWL_EXSTYLE = -20
GWLP_HWNDPARENT = -8
GA_PARENT = 1
GW_OWNER = 4
WS_CHILD = 0x40000000
WS_POPUP = 0x80000000
WS_CLIPCHILDREN = 0x02000000
SWP_SHOWWINDOW = 0x0040
SWP_NOSIZE = 0x0001
SWP_NOZORDER = 0x0004
PROCESS_TERMINATE = 0x0001
PROCESS_SUSPEND_RESUME = 0x0800
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
STILL_ACTIVE = 259


class GUITHREADINFO(ctypes.Structure):
    _fields_ = [
        ("cbSize", W.DWORD),
        ("flags", W.DWORD),
        ("hwndActive", W.HWND),
        ("hwndFocus", W.HWND),
        ("hwndCapture", W.HWND),
        ("hwndMenuOwner", W.HWND),
        ("hwndMoveSize", W.HWND),
        ("hwndCaret", W.HWND),
        ("rcCaret", W.RECT),
    ]


u32.GetGUIThreadInfo.argtypes = [W.DWORD, ctypes.POINTER(GUITHREADINFO)]
u32.GetGUIThreadInfo.restype = W.BOOL


class MI(ctypes.Structure):
    _fields_ = [
        ("dx", W.LONG),
        ("dy", W.LONG),
        ("mouseData", W.DWORD),
        ("dwFlags", W.DWORD),
        ("time", W.DWORD),
        ("dwExtraInfo", ctypes.c_size_t),
    ]


class KI(ctypes.Structure):
    _fields_ = [
        ("wVk", W.WORD),
        ("wScan", W.WORD),
        ("dwFlags", W.DWORD),
        ("time", W.DWORD),
        ("dwExtraInfo", ctypes.c_size_t),
    ]


class HI(ctypes.Structure):
    _fields_ = [("uMsg", W.DWORD), ("wParamL", W.WORD), ("wParamH", W.WORD)]


class _U(ctypes.Union):
    _fields_ = [("mi", MI), ("ki", KI), ("hi", HI)]


class INPUT(ctypes.Structure):
    _fields_ = [("type", W.DWORD), ("u", _U)]


u32.SendInput.argtypes = [W.UINT, ctypes.POINTER(INPUT), ctypes.c_int]
u32.SendInput.restype = W.UINT

VK = {
    "Return": 0x0D,
    "Escape": 0x1B,
    "Tab": 0x09,
    "BackSpace": 0x08,
    "Delete": 0x2E,
    "space": 0x20,
    "Up": 0x26,
    "Down": 0x28,
    "Left": 0x25,
    "Right": 0x27,
    "Home": 0x24,
    "End": 0x23,
    "Page_Up": 0x21,
    "Page_Down": 0x22,
    "Insert": 0x2D,
    "grave": 0xC0,
    "ctrl": 0x11,
    "shift": 0x10,
    "alt": 0x12,
}
VK.update({f"F{n}": 0x6F + n for n in range(1, 13)})
EXT_VK = {0x26, 0x28, 0x25, 0x27, 0x24, 0x23, 0x21, 0x22, 0x2D, 0x2E}


def vk_of(name: str) -> int:
    if name in VK:
        return VK[name]
    if len(name) == 1 and name.isalnum():
        return ord(name.upper())
    raise ValueError(f"unknown key: {name}")


def _send(*ins):
    arr = (INPUT * len(ins))(*ins)
    u32.SendInput(len(ins), arr, ctypes.sizeof(INPUT))
    time.sleep(0.03)


def _mouse(flags: int, data: int = 0) -> INPUT:
    i = INPUT(type=0)
    i.u.mi = MI(0, 0, data & 0xFFFFFFFF, flags, 0, 0)
    return i


def _kbd(vk: int = 0, scan: int = 0, flags: int = 0) -> INPUT:
    i = INPUT(type=1)
    i.u.ki = KI(vk, scan, flags, 0, 0)
    return i


def screen_move(x: int, y: int):
    u32.SetCursorPos(int(x), int(y))
    time.sleep(0.05)


def screen_click(x: int, y: int, button: int = 1):
    screen_move(x, y)
    dn, up = {1: (2, 4), 2: (0x20, 0x40), 3: (8, 0x10)}[button]
    _send(_mouse(dn))
    time.sleep(0.06)
    _send(_mouse(up))
    time.sleep(0.05)


def screen_drag(x0: int, y0: int, x1: int, y1: int):
    screen_move(x0, y0)
    _send(_mouse(2))
    time.sleep(0.08)
    for step in range(1, 13):
        screen_move(
            int(x0 + (x1 - x0) * step / 12),
            int(y0 + (y1 - y0) * step / 12),
        )
    _send(_mouse(4))
    time.sleep(0.06)


def key_tap(name: str):
    v = vk_of(name)
    ext = 1 if v in EXT_VK else 0
    _send(_kbd(v, 0, ext))
    _send(_kbd(v, 0, ext | 2))
    time.sleep(0.05)


def key_chord(*names: str):
    mods = [vk_of(m) for m in names[:-1]]
    last = vk_of(names[-1])
    for m in mods:
        _send(_kbd(m, 0, 1 if m in EXT_VK else 0))
    ext = 1 if last in EXT_VK else 0
    _send(_kbd(last, 0, ext))
    _send(_kbd(last, 0, ext | 2))
    for m in reversed(mods):
        _send(_kbd(m, 0, (1 if m in EXT_VK else 0) | 2))
    time.sleep(0.06)


def pid_alive(pid: int) -> bool:
    if not pid or pid <= 0:
        return False
    h = k32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, int(pid))
    if not h:
        return False
    try:
        code = W.DWORD(0)
        if not k32.GetExitCodeProcess(h, ctypes.byref(code)):
            return False
        return code.value == STILL_ACTIVE
    finally:
        k32.CloseHandle(h)


def kill_pid_tree(pid: int):
    if pid and pid > 0:
        subprocess.run(
            ["taskkill", "/T", "/F", "/PID", str(int(pid))],
            capture_output=True,
        )


def hard_kill_pid(pid: int):
    if not pid or pid <= 0:
        return
    h = k32.OpenProcess(PROCESS_TERMINATE, False, int(pid))
    if h:
        try:
            k32.TerminateProcess(h, 137)
        finally:
            k32.CloseHandle(h)


def suspend_pid(pid: int, suspend: bool) -> bool:
    h = k32.OpenProcess(PROCESS_SUSPEND_RESUME, False, int(pid))
    if not h:
        return False
    try:
        rc = ntdll.NtSuspendProcess(h) if suspend else ntdll.NtResumeProcess(h)
        return rc == 0
    finally:
        k32.CloseHandle(h)


def hwnd_title(h: int) -> str:
    n = u32.GetWindowTextLengthW(h)
    if n <= 0:
        return ""
    buf = ctypes.create_unicode_buffer(n + 2)
    u32.GetWindowTextW(h, buf, n + 2)
    return buf.value


def hwnd_class(h: int) -> str:
    buf = ctypes.create_unicode_buffer(256)
    u32.GetClassNameW(h, buf, 256)
    return buf.value


def hwnd_pid(h: int) -> int:
    p = W.DWORD(0)
    u32.GetWindowThreadProcessId(h, ctypes.byref(p))
    return int(p.value)


def hwnd_rect(h: int):
    r = W.RECT()
    if not u32.GetWindowRect(h, ctypes.byref(r)):
        return (0, 0, 0, 0)
    return (int(r.left), int(r.top), int(r.right - r.left), int(r.bottom - r.top))


def hwnd_style(h: int) -> int:
    return int(u32.GetWindowLongPtrW(h, GWL_STYLE)) & 0xFFFFFFFF


def hwnd_owner(h: int) -> int:
    o = u32.GetWindow(h, GW_OWNER)
    if o:
        return int(o)
    if (hwnd_style(h) & WS_CHILD) == 0:
        p = u32.GetWindowLongPtrW(h, GWLP_HWNDPARENT)
        return int(p) & 0xFFFFFFFFFFFFFFFF
    return 0


def find_main_window(pid: int):
    found = []

    def cb(h, _):
        if hwnd_pid(h) == pid and u32.IsWindowVisible(h) and u32.GetWindowTextLengthW(h) > 0:
            x, y, w, ht = hwnd_rect(h)
            if w >= 200 and ht >= 150:
                found.append((w * ht, int(h)))
        return True

    u32.EnumWindows(WNDENUMPROC(cb), 0)
    return max(found)[1] if found else 0


def place_client_window(h: int, x: int, y: int, client_w: int, client_h: int):
    u32.ShowWindow(h, 9)  # SW_RESTORE
    r = W.RECT(0, 0, int(client_w), int(client_h))
    style = u32.GetWindowLongPtrW(h, GWL_STYLE) & 0xFFFFFFFF
    ex = u32.GetWindowLongPtrW(h, GWL_EXSTYLE) & 0xFFFFFFFF
    u32.AdjustWindowRectEx(ctypes.byref(r), style, False, ex)
    win_w = int(r.right - r.left)
    win_h = int(r.bottom - r.top)
    u32.SetWindowPos(h, 0, int(x), int(y), win_w, win_h, SWP_SHOWWINDOW)
    u32.SetForegroundWindow(h)


def client_origin(h: int):
    pt = W.POINT(0, 0)
    u32.ClientToScreen(h, ctypes.byref(pt))
    return int(pt.x), int(pt.y)


def list_top_windows(pid: int = 0):
    out = []

    def cb(h, _):
        if not u32.IsWindowVisible(h):
            return True
        wpid = hwnd_pid(h)
        if pid and wpid != pid:
            return True
        x, y, w, ht = hwnd_rect(h)
        out.append(
            {
                "hwnd": int(h),
                "pid": wpid,
                "title": hwnd_title(h),
                "class": hwnd_class(h),
                "root_x": x,
                "root_y": y,
                "w": w,
                "h": ht,
                "style": hwnd_style(h),
                "owner": hwnd_owner(h),
            }
        )
        return True

    u32.EnumWindows(WNDENUMPROC(cb), 0)
    return out


def list_child_windows(parent_hwnd: int, pid: int = 0):
    out = []
    if not parent_hwnd or not u32.IsWindow(parent_hwnd):
        return out

    def cb(h, _):
        wpid = hwnd_pid(h)
        if pid and wpid != pid:
            return True
        x, y, w, ht = hwnd_rect(h)
        out.append(
            {
                "hwnd": int(h),
                "pid": wpid,
                "title": hwnd_title(h),
                "class": hwnd_class(h),
                "visible": bool(u32.IsWindowVisible(h)),
                "root_x": x,
                "root_y": y,
                "w": w,
                "h": ht,
                "style": hwnd_style(h),
                "parent": int(u32.GetAncestor(h, GA_PARENT) or 0),
            }
        )
        return True

    u32.EnumChildWindows(parent_hwnd, WNDENUMPROC(cb), 0)
    return out


def is_frame_parented_in(frm_h: int, cont_h: int) -> bool:
    if not frm_h or not cont_h or (hwnd_style(frm_h) & WS_CHILD) == 0:
        return False
    p = int(u32.GetAncestor(frm_h, GA_PARENT) or 0)
    if p == cont_h:
        return True
    return bool(
        p
        and hwnd_class(p) == "BackplaneKiCadEmbedHolder"
        and (hwnd_style(p) & WS_CHILD) != 0
        and int(u32.GetAncestor(p, GA_PARENT) or 0) == cont_h
    )


def list_popup_menus(kicad_pid: int):
    out = []
    if kicad_pid <= 0:
        return out
    for w in list_top_windows(kicad_pid):
        if w["w"] < 30 or w["h"] < 20:
            continue
        is_menu = w["class"] == "#32768"
        is_kicad_popup = (
            (w["style"] & WS_POPUP) != 0
            and "pcb editor" not in w["title"].lower()
            and "schematic editor" not in w["title"].lower()
        )
        if is_menu or is_kicad_popup:
            out.append(w)
    return out


def foreground_focus_hwnd() -> int:
    gti = GUITHREADINFO()
    gti.cbSize = ctypes.sizeof(GUITHREADINFO)
    if u32.GetGUIThreadInfo(0, ctypes.byref(gti)) and gti.hwndFocus:
        return int(gti.hwndFocus)
    return 0


def grab_screen(path: str, bbox=None):
    from PIL import ImageGrab

    img = ImageGrab.grab(bbox=bbox, all_screens=True)
    img.save(path)
    return path


def img_diff_box(path_a: str, path_b: str, x: int, y: int, w: int, h: int):
    from PIL import Image, ImageChops

    ia = Image.open(path_a).convert("RGB")
    ib = Image.open(path_b).convert("RGB")
    iw, ih = ia.size
    x0 = max(0, min(iw - 1, int(x)))
    y0 = max(0, min(ih - 1, int(y)))
    x1 = max(x0 + 1, min(iw, int(x + w)))
    y1 = max(y0 + 1, min(ih, int(y + h)))
    ca = ia.crop((x0, y0, x1, y1))
    cb = ib.crop((x0, y0, x1, y1))
    diff = ImageChops.difference(ca, cb)
    raw = diff.tobytes()
    changed = 0
    for i in range(0, len(raw), 3):
        if raw[i] + raw[i + 1] + raw[i + 2] > 24:
            changed += 1
    return {"changed_pixels": changed, "box": [x0, y0, x1 - x0, y1 - y0]}


def sha256_file(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def find_bun() -> str:
    cand = shutil.which("bun")
    if cand:
        return cand
    up = os.environ.get("USERPROFILE", r"C:\Users\bpadmin")
    p = os.path.join(up, ".bun", "bin", "bun.exe")
    if os.path.isfile(p):
        return p
    raise RuntimeError("bun.exe not found")


class SuiteHarness:
    def __init__(self, suite: str, client_exe: str, hub_exe: str, assets_dir: str, kicad_dir: str, out_dir: str, port: int):
        self.suite = suite
        self.client_exe = os.path.abspath(client_exe)
        self.hub_exe = os.path.abspath(hub_exe)
        self.assets_dir = os.path.abspath(assets_dir)
        self.kicad_dir = os.path.abspath(kicad_dir)
        self.kicad_bin = os.path.join(self.kicad_dir, "bin", "kicad.exe")
        self.kicad_cli = os.path.join(self.kicad_dir, "bin", "kicad-cli.exe")
        self.out_dir = os.path.abspath(out_dir)
        self.port = port
        self.rows = []
        self.tracked_pids = set()
        self.procs = []
        self.bun = find_bun()

        scr_w = u32.GetSystemMetrics(0)
        scr_h = u32.GetSystemMetrics(1)
        self.init_w = min(1440, max(1180, scr_w - 80))
        self.init_h = min(860, max(720, scr_h - 90))

    def p(self, *parts: str) -> str:
        return os.path.join(self.out_dir, *parts)

    def check(self, what: str, cond: bool, detail="") -> bool:
        ok = bool(cond)
        ev = str(detail)[:360] if detail != "" else ""
        self.rows.append({"id": what, "ok": ok, "ev": ev})
        if ok:
            print(f"PASS {what}", flush=True)
        else:
            suffix = f" ({ev})" if ev else ""
            print(f"FAIL {what}{suffix}", flush=True)
        return ok

    def track_pid(self, pid: int):
        if pid and int(pid) > 0:
            self.tracked_pids.add(int(pid))

    def init_dirs(self, seed_msg: str = ""):
        shutil.rmtree(self.out_dir, ignore_errors=True)
        for sub in (
            "home",
            "proj",
            "bin",
            "shots",
            "tmp",
            "fh/.config",
            "fh/.cache",
            "fh/.local/share",
            "fh/AppData/Roaming",
            "fh/AppData/Local",
            "fh/AppData/Roaming/kicad/10.99",
        ):
            os.makedirs(self.p(sub), exist_ok=True)
        seed_env = dict(os.environ, KICAD_CLI=self.kicad_cli)
        if seed_msg:
            seed_env["KEMBED_SEED_MESSAGE"] = seed_msg
        subprocess.run(
            [sys.executable, SEED_PY, self.out_dir, REPO],
            env=seed_env,
            check=True,
        )
        # BP_SEED_KICAD_PROFILE=<dir>: start KiCad from a profile an earlier session left (e.g. a saved AUI layout with panes open)
        seed_prof = os.environ.get("BP_SEED_KICAD_PROFILE", "")
        if seed_prof and os.path.isdir(seed_prof):
            shutil.copytree(seed_prof, self.p("fh/AppData/Roaming/kicad/10.99"), dirs_exist_ok=True)

    def base_env(self, embed_mode: str = "fork") -> dict:
        env = dict(os.environ)
        kicad_bin_dir = os.path.join(self.kicad_dir, "bin")
        py_dir = os.path.dirname(sys.executable)
        bun_dir = os.path.dirname(self.bun)
        env.update(
            HOME=self.p("fh"),
            USERPROFILE=self.p("fh"),
            APPDATA=self.p("fh/AppData/Roaming"),
            LOCALAPPDATA=self.p("fh/AppData/Local"),
            XDG_CONFIG_HOME=self.p("fh/.config"),
            XDG_CACHE_HOME=self.p("fh/.cache"),
            XDG_DATA_HOME=self.p("fh/.local/share"),
            TMP=self.p("tmp"),
            TEMP=self.p("tmp"),
            TMPDIR=self.p("tmp"),
            BACKPLANE_AGENTS=self.p("bin"),
            BACKPLANE_NO_UPDATE="1",
            BACKPLANE_KICAD_BIN=self.kicad_bin,
            BACKPLANE_KICAD_CLI=self.kicad_cli,
            BACKPLANE_KICAD_EMBED_MODE=embed_mode,
            KICAD_CONFIG_HOME=self.p("fh/AppData/Roaming/kicad"),
            FAKE_LOG=self.p("standin.log"),
            BP_STANDIN_MODE="stream",
            PATH=os.pathsep.join([self.p("bin"), kicad_bin_dir, py_dir, bun_dir, env.get("PATH", "")]),
        )
        env.pop("SHELL", None)
        return env

    def start_hub(self, embed_mode: str = "fork", port: int = 0, sub_home: str = "home", log_name: str = "hub.log"):
        hp = port or self.port
        env = self.base_env(embed_mode=embed_mode)
        lf = open(self.p(log_name), "ab")
        proc = subprocess.Popen(
            [
                self.hub_exe,
                "--home",
                self.p(sub_home),
                "--port",
                str(hp),
                "--host",
                "127.0.0.1",
                "--no-tailscale",
                "--foreground",
            ],
            env=env,
            cwd=self.p("proj"),
            stdout=lf,
            stderr=subprocess.STDOUT,
            creationflags=subprocess.CREATE_NO_WINDOW,
        )
        self.procs.append(proc)
        self.track_pid(proc.pid)
        for _ in range(100):
            if self.hub_hello(hp):
                return proc
            time.sleep(0.2)
        raise RuntimeError(f"hub failed to start on port {hp}")

    def hub_hello(self, port: int = 0) -> bool:
        hp = port or self.port
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{hp}/hello", timeout=1.2) as r:
                return r.status == 200
        except Exception:
            return False

    def start_client(
        self,
        script_obj,
        log_name: str = "client.log",
        port: int = 0,
        token_rel: str = "home/token",
        hold: bool = False,
        exit_after: int = 240,
        place_xy=(16, 16),
    ):
        hp = port or self.port
        script_path = self.p(f"{log_name}.script.json")
        with open(script_path, "w", encoding="utf-8") as f:
            json.dump(script_obj, f)
        env = self.base_env()
        env["BACKPLANE_GPUI_ASSETS"] = self.assets_dir
        env["BACKPLANE_GPUI_RECTS"] = "1"
        if hold:
            env["BACKPLANE_GPUI_HOLD"] = "1"
        lf = open(self.p(log_name), "ab")
        proc = subprocess.Popen(
            [
                self.client_exe,
                "--url",
                f"http://127.0.0.1:{hp}",
                "--token-file",
                self.p(token_rel),
                "--script",
                script_path,
                "--exit-after",
                str(exit_after),
            ],
            env=env,
            cwd=self.p("proj"),
            stdout=lf,
            stderr=subprocess.STDOUT,
            creationflags=subprocess.CREATE_NO_WINDOW,
        )
        self.procs.append(proc)
        self.track_pid(proc.pid)

        def _placer():
            for _ in range(50):
                if proc.poll() is not None:
                    return
                h = find_main_window(proc.pid)
                if h:
                    place_client_window(h, place_xy[0], place_xy[1], self.init_w, self.init_h)
                    time.sleep(1.5)
                    if u32.IsWindow(h):
                        place_client_window(h, place_xy[0], place_xy[1], self.init_w, self.init_h)
                    return
                time.sleep(0.2)

        threading.Thread(target=_placer, daemon=True).start()
        return proc

    def wait_client_hwnd(self, proc, timeout_s: float = 20.0) -> int:
        end = time.time() + timeout_s
        while time.time() < end:
            h = find_main_window(proc.pid)
            if h:
                return h
            time.sleep(0.2)
        return 0

    def metrics(self, log_name: str = "client.log"):
        path = self.p(log_name)
        if not os.path.isfile(path):
            return []
        out = []
        with open(path, encoding="utf-8", errors="replace") as f:
            for line in f:
                if "GPUI_METRIC " in line:
                    try:
                        out.append(json.loads(line.split("GPUI_METRIC ", 1)[1]))
                    except Exception:
                        pass
        return out

    def kicad_reports(self, log_name: str = "client.log") -> dict:
        reps = {}
        for m in self.metrics(log_name):
            if m.get("event") == "kicad" and m.get("tag"):
                reps[m["tag"]] = m
        return reps

    def last_kicad_report(self, tag: str, log_name: str = "client.log") -> dict:
        return self.kicad_reports(log_name).get(tag, {})

    def wait_for_mark(self, mark_name: str, log_name: str = "client.log", timeout_s: float = 75.0) -> bool:
        end = time.time() + timeout_s
        while time.time() < end:
            for m in self.metrics(log_name):
                if m.get("event") == "mark" and m.get("name") == mark_name:
                    return True
            time.sleep(0.2)
        return False

    def controls(self, log_name: str = "client.log") -> dict:
        ctl = {}
        for m in self.metrics(log_name):
            if m.get("event") == "controls" and m.get("controls"):
                ctl = {c["id"]: c for c in m["controls"]}
        return ctl

    def probes(self, log_name: str = "client.log") -> dict:
        pr = {}
        for m in self.metrics(log_name):
            if m.get("event") == "probes" and m.get("rects"):
                pr = {r["id"]: r for r in m["rects"]}
        return pr

    def log_line_count(self, log_name: str = "client.log") -> int:
        path = self.p(log_name)
        if not os.path.isfile(path):
            return 0
        with open(path, encoding="utf-8", errors="replace") as f:
            return sum(1 for _ in f)

    def last_gpui_log(self, event_name: str, after_line: int = 0, log_name: str = "client.log") -> dict:
        path = self.p(log_name)
        if not os.path.isfile(path):
            return {}
        found = {}
        with open(path, encoding="utf-8", errors="replace") as f:
            for idx, line in enumerate(f):
                if idx < after_line or "GPUI_LOG " not in line:
                    continue
                try:
                    obj = json.loads(line.split("GPUI_LOG ", 1)[1])
                except Exception:
                    continue
                if obj.get("event") == event_name:
                    found = obj
        return found

    def click_link(self, gpui_hwnd: int, href: str, log_name: str = "client.log", timeout_s: float = 12.0) -> bool:
        end = time.time() + timeout_s
        while time.time() < end:
            rect = None
            for m in self.metrics(log_name):
                if m.get("event") == "links":
                    rect = next((l["rect"] for l in (m.get("links") or []) if l.get("href") == href), None)
            if isinstance(rect, dict) and "x" in rect and "y" in rect:
                ox, oy = client_origin(gpui_hwnd)
                screen_click(int(ox + rect["x"] + min(rect["w"] / 2, 8)), int(oy + rect["y"] + rect["h"] / 2))
                return True
            time.sleep(0.25)
        return False

    def click_control(self, gpui_hwnd: int, control_id: str, log_name: str = "client.log") -> bool:
        for _ in range(25):
            c = self.controls(log_name).get(control_id)
            r = (c.get("rect") if c else None) or self.probes(log_name).get(control_id)
            if isinstance(r, dict) and "x" in r and "y" in r:
                ox, oy = client_origin(gpui_hwnd)
                screen_click(int(ox + r["x"] + r["w"] / 2), int(oy + r["y"] + r["h"] / 2))
                return True
            time.sleep(0.2)
        return False

    def hub_rpc(self, steps, port: int = 0, token_rel: str = "home/token") -> str:
        hp = port or self.port
        cp = subprocess.run(
            [self.bun, HUB_RPC_MJS, str(hp), self.p(token_rel), json.dumps(steps)],
            capture_output=True,
            text=True,
            timeout=65,
        )
        if cp.returncode != 0:
            print(f"WARN hub_rpc exit={cp.returncode}: {cp.stderr or cp.stdout}", flush=True)
        return cp.stdout

    def ipc_cmd(self, *args: str, timeout: float = 15.0) -> dict:
        cp = subprocess.run(
            [sys.executable, IPC_PY, *args],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        if cp.returncode != 0:
            raise RuntimeError(f"ipc {args} failed ({cp.returncode}): {cp.stderr or cp.stdout}")
        return json.loads(cp.stdout.strip() or "{}")

    def wait_for_ipc(self, sock: str, timeout_s: float = 20.0) -> bool:
        if not sock:
            return False
        end = time.time() + timeout_s
        while time.time() < end:
            try:
                res = self.ipc_cmd("ping", sock, timeout=4.0)
                if res.get("ok"):
                    return True
            except Exception:
                pass
            time.sleep(0.25)
        return False

    def shot(self, name: str) -> str:
        return grab_screen(self.p("shots", f"{name}.png"))

    def wait_embedded_frame(self, cont_hwnd: int, kicad_pid: int, title_sub: str, timeout_s: float = 35.0):
        sub = title_sub.lower()
        end = time.time() + timeout_s
        while time.time() < end:
            _, _, cw, ch_h = hwnd_rect(cont_hwnd)
            for ch in list_child_windows(cont_hwnd):
                if (
                    ch["visible"]
                    and is_frame_parented_in(ch["hwnd"], cont_hwnd)
                    and (not kicad_pid or ch["pid"] == kicad_pid)
                    and sub in ch["title"].lower()
                    and ch["w"] >= 150
                    and ch["h"] >= 100
                    and (cw < 150 or (abs(ch["w"] - cw) <= 8 and abs(ch["h"] - ch_h) <= 8))
                ):
                    return ch
            time.sleep(0.25)
        return None

    def wait_kicad_dialog(self, kicad_pid: int, title_sub: str, timeout_s: float = 20.0):
        sub = title_sub.lower()
        end = time.time() + timeout_s
        while time.time() < end:
            for w in list_top_windows(kicad_pid):
                if sub in w["title"].lower() and w["w"] >= 160 and w["h"] >= 90:
                    return w
            time.sleep(0.2)
        return None

    def cleanup(self):
        for p in reversed(self.procs):
            try:
                kill_pid_tree(p.pid)
            except Exception:
                pass
        for pid in list(self.tracked_pids):
            try:
                kill_pid_tree(pid)
            except Exception:
                pass
        with open(self.p("rows.json"), "w", encoding="utf-8") as f:
            json.dump({"suite": self.suite, "rows": self.rows}, f, indent=2)

    def finish(self) -> int:
        passed = sum(1 for r in self.rows if r["ok"])
        failed = sum(1 for r in self.rows if not r["ok"])
        print(
            f"win_embed_{self.suite}_e2e (fork): passed={passed} failed={failed} artifacts={self.out_dir}",
            flush=True,
        )
        return 1 if failed else 0


def shown_frame_of(rep: dict) -> dict:
    sx = rep.get("shown")
    for fr in rep.get("frames") or []:
        if fr.get("xid") == sx:
            return fr
    return {}


# ==============================================================================
# Suite 1: window
# ==============================================================================
def run_window_suite(h: SuiteHarness) -> int:
    h.init_dirs()
    try:
        h.start_hub(embed_mode="fork")

        script = [
            {"select": 0},
            {"wait": 2800},
            {"click": "rail:Board"},
            {"wait": 2800},
            {"click": "ui:Edit"},
            {"kicad": {"wait_painted": 45000, "kind": "PCB", "tag": "shown"}},
            {"mark": "01_shown"},
            {"wait": 3000},
            {"kicad": {"report": True, "tag": "clicked_focus"}},
            {"mark": "handback_focus"},
            {"wait": 2000},
            {"kicad": {"report": True, "tag": "focus_returned"}},
            {"keys": "ctrl-b"},
            {"wait": 1200},
            {"kicad": {"report": True, "tag": "sidebar_folded"}},
            {"mark": "02_sidebar_folded"},
            {"wait": 900},
            {"keys": "ctrl-b"},
            {"wait": 1000},
            {"mark": "pre_resize"},
            {"wait": 2400},
            {"kicad": {"report": True, "tag": "resized"}},
            {"mark": "03_resized"},
            {"wait": 900},
            {"mark": "pre_move"},
            {"wait": 2400},
            {"kicad": {"report": True, "tag": "moved_pre_popup"}},
            {"mark": "pure_moved"},
            {"wait": 4200},
            {"keys": "f1"},
            {"wait": 900},
            {"kicad": {"report": True, "tag": "overlay_cutout"}},
            {"mark": "05_overlay_cutout"},
            {"wait": 900},
            {"keys": "f1"},
            {"wait": 900},
            {"kicad": {"report": True, "tag": "overlay_closed"}},
            {"mark": "06_overlay_closed"},
            {"wait": 900},
            {"click": "rail:Files"},
            {"wait": 1100},
            {"kicad": {"report": True, "tag": "other_view"}},
            {"mark": "07_other_view"},
            {"wait": 900},
            {"click": "rail:Board"},
            {"wait": 1100},
            {"kicad": {"report": True, "tag": "back_board"}},
            {"mark": "08_back_board"},
            {"wait": 900},
            {"click": "rail:Schematic"},
            {"kicad": {"wait_painted": 45000, "kind": "SCH", "tag": "cold_sch"}},
            {"mark": "09_switch_sch"},
            {"wait": 900},
            {"click": "rail:3D"},
            {"kicad": {"wait_painted": 45000, "kind": "PCB_3D", "tag": "cold_3d"}},
            {"wait": 2500},
            {"mark": "10_switch_3d"},
            {"wait": 900},
            {"click": "rail:Board"},
            {"kicad": {"wait_painted": 6000, "kind": "PCB", "tag": "warm_pcb"}},
            {"click": "rail:Schematic"},
            {"kicad": {"wait_painted": 6000, "kind": "SCH", "tag": "warm_sch"}},
            {"click": "rail:3D"},
            {"kicad": {"wait_painted": 6000, "kind": "PCB_3D", "tag": "warm_3d"}},
            {"click": "rail:Board"},
            {"kicad": {"wait_painted": 6000, "kind": "PCB", "tag": "warm_pcb_2"}},
            {"mark": "11_switch_pcb"},
            {"wait": 900},
            {"click": "ui:View"},
            {"wait": 1300},
            {"kicad": {"report": True, "tag": "view_mode"}},
            {"mark": "12_view_mode"},
        ]

        cp = h.start_client(script, log_name="client.log", hold=True, exit_after=240)
        gpui_hwnd = h.wait_client_hwnd(cp)
        seen = set()
        popups_captured = []
        move_click_pt = (0, 0)
        win32_styles_obs = {}

        while cp.poll() is None and "12_view_mode" not in seen:
            for m in h.metrics("client.log"):
                if m.get("event") != "mark":
                    continue
                name = m.get("name")
                if not name or name in seen:
                    continue
                seen.add(name)
                time.sleep(0.2)

                if name == "01_shown":
                    h.shot("01_shown")
                    r_shown = h.last_kicad_report("shown")
                    top_h = int(r_shown.get("toplevel") or gpui_hwnd or 0)
                    cont_h = int((r_shown.get("container") or {}).get("xid") or 0)
                    frm_h = int(r_shown.get("shown") or 0)
                    kpid = int(r_shown.get("pid") or 0)
                    h.track_pid(kpid)
                    # the PCB frame's window tree as shown (AUI panes: canvas, toolbars, Appearance, Properties, Agents...) and its
                    # client size, for diagnosing a squeezed canvas
                    try:
                        fh = frm_h or int((shown_frame_of(r_shown) or {}).get("xid") or 0)
                        rc = W.RECT()
                        u32.GetClientRect(fh, ctypes.byref(rc))
                        kids = [c for c in list_child_windows(fh) if c["visible"] and c["w"] >= 8 and c["h"] >= 8]
                        with open(h.p("panes_01_shown.json"), "w", encoding="utf-8") as pf:
                            json.dump({"frame": fh, "client": [rc.right - rc.left, rc.bottom - rc.top], "children": kids}, pf, indent=1)
                    except Exception as e:
                        print(f"DIAG panes dump failed: {e}")
                    win32_styles_obs = {
                        "top_clipchildren": bool(top_h and (hwnd_style(top_h) & WS_CLIPCHILDREN) != 0),
                        "cont_ws_child": bool(cont_h and (hwnd_style(cont_h) & WS_CHILD) != 0 and int(u32.GetAncestor(cont_h, GA_PARENT) or 0) == top_h),
                        "frm_ws_child": is_frame_parented_in(frm_h, cont_h),
                    }
                    sr = r_shown.get("shown_root") or {}
                    pane = r_shown.get("pane") or {}
                    cx = int(sr.get("x", 600) + int(pane.get("w", 600) * 0.35))
                    cy = int(sr.get("y", 300) + int(pane.get("h", 500) * 0.50))
                    screen_click(cx, cy, 1)
                    time.sleep(0.35)

                elif name == "handback_focus":
                    key_chord("ctrl", "grave")
                    time.sleep(0.25)

                elif name == "pre_resize":
                    if gpui_hwnd and u32.IsWindow(gpui_hwnd):
                        place_client_window(gpui_hwnd, 24, 24, h.init_w - 180, h.init_h - 110)

                elif name == "pre_move":
                    if gpui_hwnd and u32.IsWindow(gpui_hwnd):
                        u32.SetWindowPos(gpui_hwnd, 0, 260, 95, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_SHOWWINDOW)

                elif name == "pure_moved":
                    r_mv = h.last_kicad_report("moved_pre_popup")
                    kpid = int(r_mv.get("pid") or 0)
                    h.track_pid(kpid)
                    sr = r_mv.get("shown_root") or {}
                    pane = r_mv.get("pane") or {}
                    gx = int(sr.get("x", 700) + int(pane.get("w", 600) * 0.45))
                    gy = int(sr.get("y", 350) + int(pane.get("h", 500) * 0.50))
                    move_click_pt = (gx, gy)
                    screen_click(gx, gy, 1)
                    time.sleep(0.25)
                    screen_click(gx, gy, 3)
                    time.sleep(0.65)
                    h.shot("04_pure_moved_popup")
                    popups_captured = list_popup_menus(kpid)
                    if popups_captured:
                        key_tap("Escape")
                        time.sleep(0.2)
                    key_chord("ctrl", "grave")
                    time.sleep(0.2)

                else:
                    h.shot(name)
            time.sleep(0.12)

        reps = h.kicad_reports("client.log")
        r_shown = reps.get("shown", {})
        kpid = int(r_shown.get("pid") or 0)
        h.track_pid(kpid)

        warm_alive = pid_alive(kpid)
        pro_path = h.p("proj", "ecc83-pp.kicad_pro")
        if not os.path.isfile(pro_path):
            pro_path = h.p("proj", "boards", "ecc83-pp", "ecc83-pp.kicad_pro")
        h.hub_rpc([{"m": "editor.kicad.close", "p": {"project": pro_path, "save": "discard"}}])
        closed_clean = False
        for _ in range(40):
            if not pid_alive(kpid):
                closed_clean = True
                break
            time.sleep(0.2)
        kill_pid_tree(cp.pid)

        f_shown = shown_frame_of(r_shown)
        c_shown = r_shown.get("container") or {}
        p_shown = r_shown.get("pane") or {}

        h.check(
            "Hub spawned KiCad in fork mode and reached ready with valid pid",
            r_shown.get("mode") == "fork" and kpid > 0,
            {"mode": r_shown.get("mode"), "pid": kpid, "state": r_shown.get("state")},
        )
        h.check(
            "GPUI HWND carries WS_CLIPCHILDREN and container + PCB frame are Win32 WS_CHILD windows",
            win32_styles_obs.get("top_clipchildren") is True
            and win32_styles_obs.get("cont_ws_child") is True
            and win32_styles_obs.get("frm_ws_child") is True,
            win32_styles_obs,
        )
        h.check(
            "PCB Editor window is reparented inside Backplane GPUI Win32 tree",
            r_shown.get("in_gpui_tree") is True and f_shown.get("shown") is True and f_shown.get("kind") == "PCB",
            {"in_gpui_tree": r_shown.get("in_gpui_tree"), "frame": f_shown},
        )
        h.check(
            "Client-placed PCB Editor matches viewer pane geometry within <=2px",
            abs(c_shown.get("x", -99) - p_shown.get("x", 0)) <= 2
            and abs(c_shown.get("y", -99) - p_shown.get("y", 0)) <= 2
            and abs(f_shown.get("w", -99) - p_shown.get("w", 0)) <= 2
            and abs(f_shown.get("h", -99) - p_shown.get("h", 0)) <= 2,
            {"container": c_shown, "frame": f_shown, "pane": p_shown},
        )
        h.check(
            "PCB Editor OpenGL GAL canvas painted non-blank content inside Backplane pane",
            (f_shown.get("painted") or 0) > 20 and (c_shown.get("painted") or 0) > 20,
            {"frame_painted": f_shown.get("painted"), "container_painted": c_shown.get("painted")},
        )
        h.check(
            "Opening embedded KiCad did not steal keyboard focus before user click",
            r_shown.get("keys") is False,
            {"keys": r_shown.get("keys")},
        )

        r_cf = reps.get("clicked_focus", {})
        r_fr = reps.get("focus_returned", {})
        h.check(
            "Clicking inside embedded PCB canvas gives KiCad focus (WM_APP+0x101) and Ctrl+` returns focus to GPUI",
            r_cf.get("keys") is True and r_fr.get("keys") is False,
            {"after_click": r_cf.get("keys"), "after_ctrl_backtick": r_fr.get("keys")},
        )

        r_fold = reps.get("sidebar_folded", {})
        c_fold = r_fold.get("container") or {}
        f_fold = shown_frame_of(r_fold)
        p_fold = r_fold.get("pane") or {}
        h.check(
            "Folding GPUI sidebar (Ctrl+B) reflows and widens client-placed KiCad pane",
            f_fold.get("shown") is True
            and c_fold.get("x", 9999) < c_shown.get("x", 0)
            and f_fold.get("w", 0) > f_shown.get("w", 0)
            and abs(f_fold.get("w", -99) - p_fold.get("w", 0)) <= 2,
            {"before_x": c_shown.get("x"), "after_x": c_fold.get("x"), "before_w": f_shown.get("w"), "after_w": f_fold.get("w")},
        )

        r_res = reps.get("resized", {})
        f_res = shown_frame_of(r_res)
        p_res = r_res.get("pane") or {}
        h.check(
            "Top-level window resize resizes client-placed KiCad cleanly within <=2px",
            f_res.get("shown") is True
            and f_res.get("w", 9999) < f_shown.get("w", 0)
            and abs(f_res.get("w", -99) - p_res.get("w", 0)) <= 2
            and abs(f_res.get("h", -99) - p_res.get("h", 0)) <= 2,
            {"before_w": f_shown.get("w"), "after_w": f_res.get("w"), "pane": p_res},
        )

        r_mv = reps.get("moved_pre_popup", {})
        sr_mv = r_mv.get("shown_root") or {}
        p_mv = r_mv.get("pane") or {}
        mcx, mcy = move_click_pt
        # Account for both left-aligned and right-aligned Win32 TrackPopupMenu (SPI_GETMENUDROPALIGNMENT) and vertical work-area clamping.
        popup_ok = any(
            min(abs(p.get("root_x", -9999) - mcx), abs(p.get("root_x", -9999) + p.get("w", 0) - mcx)) <= 120
            and (p.get("root_y", 9999) - 24) <= mcy <= (p.get("root_y", -9999) + p.get("h", 0) + 24)
            for p in popups_captured
        )
        h.check(
            "Pure top-level window move anchors right-click Win32 popup menu (#32768) at moved screen coords",
            len(popups_captured) >= 1 and popup_ok,
            {"shown_root": sr_mv, "pane": p_mv, "click": move_click_pt, "popups": popups_captured},
        )

        r_cut = reps.get("overlay_cutout", {})
        r_cut_closed = reps.get("overlay_closed", {})
        cut_rects = (r_cut.get("container") or {}).get("shape_rects", 0)
        closed_rects = (r_cut_closed.get("container") or {}).get("shape_rects", 0)
        h.check(
            "GPUI modal overlay (Keysheet F1) punches SetWindowRgn hole in container and restores on close",
            cut_rects >= 2 and closed_rects == 1,
            {"cut_rects": cut_rects, "closed_rects": closed_rects},
        )

        r_other = reps.get("other_view", {})
        r_back = reps.get("back_board", {})
        h.check(
            "Switching to non-CAD rail view (Files) hides container and switching back restores it",
            (r_other.get("container") or {}).get("shown") is False
            and (r_back.get("container") or {}).get("shown") is True
            and shown_frame_of(r_back).get("shown") is True,
            {"other_container": r_other.get("container"), "back_container": r_back.get("container")},
        )

        r_wsch = reps.get("warm_sch", {})
        r_w3d = reps.get("warm_3d", {})
        r_wpcb = reps.get("warm_pcb_2", {})
        f_wsch = shown_frame_of(r_wsch)
        f_w3d = shown_frame_of(r_w3d)
        f_wpcb = shown_frame_of(r_wpcb)
        h.check(
            "Rail multi-frame switching (Board <-> Schematic <-> 3D) paints all 3 editors with fast warm switch",
            (f_wsch.get("painted") or 0) > 15
            and (f_w3d.get("painted") or 0) > 15
            and (f_wpcb.get("painted") or 0) > 15
            and (r_wsch.get("wait_ms") or 9999) < 350
            and (r_w3d.get("wait_ms") or 9999) < 350
            and (r_wpcb.get("wait_ms") or 9999) < 350,
            {
                "warm_sch_ms": r_wsch.get("wait_ms"),
                "warm_3d_ms": r_w3d.get("wait_ms"),
                "warm_pcb_ms": r_wpcb.get("wait_ms"),
                "sch_painted": f_wsch.get("painted"),
                "v3d_painted": f_w3d.get("painted"),
                "pcb_painted": f_wpcb.get("painted"),
            },
        )

        r_view = reps.get("view_mode", {})
        h.check(
            "ui:View detaches editor cleanly while keeping warm KiCad alive, and editor.kicad.close exits it",
            r_view.get("ok") is False
            and r_view.get("error") == "not editing"
            and warm_alive
            and closed_clean,
            {"view_report": r_view, "warm_alive": warm_alive, "closed_clean": closed_clean},
        )

        # Check 15: Stock mode on Windows (never adopt cross-process top-levels; pane says needs Backplane KiCad)
        stock_dir = h.p("stock_run")
        for sub in ("home", "proj", "bin", "shots", "tmp"):
            os.makedirs(os.path.join(stock_dir, sub), exist_ok=True)
        subprocess.run(
            [sys.executable, SEED_PY, stock_dir, REPO],
            env=dict(os.environ, KICAD_CLI=h.kicad_cli),
            check=True,
        )
        stock_port = h.port + 10
        stock_hub = h.start_hub(embed_mode="stock", port=stock_port, sub_home="stock_run/home", log_name="hub_stock.log")
        stock_script = [
            {"select": 0},
            {"wait": 2600},
            {"click": "rail:Board"},
            {"wait": 2600},
            {"click": "ui:Edit"},
            {"wait": 2500},
            {"kicad": {"report": True, "tag": "stock_win"}},
            {"mark": "stock_ready"},
            {"wait": 800},
        ]
        stock_cp = h.start_client(
            stock_script,
            log_name="client_stock.log",
            port=stock_port,
            token_rel="stock_run/home/token",
            exit_after=60,
        )
        h.wait_for_mark("stock_ready", log_name="client_stock.log", timeout_s=35.0)
        h.shot("13_stock_needs_bp_kicad")
        r_stock = h.last_kicad_report("stock_win", log_name="client_stock.log")
        floating_kicad = [
            w for w in list_top_windows()
            if "pcb editor" in w["title"].lower() or "kicad 10" in w["title"].lower()
        ]
        kill_pid_tree(stock_cp.pid)
        kill_pid_tree(stock_hub.pid)

        h.check(
            "Stock mode on Windows spawns zero floating KiCad windows and reports stock mode cleanly",
            r_stock.get("mode") == "stock"
            and int(r_stock.get("pid") or 0) == 0
            and r_stock.get("state") == "ready"
            and len(floating_kicad) == 0,
            {"report": r_stock, "floating": floating_kicad},
        )

        return h.finish()
    finally:
        h.cleanup()


# ==============================================================================
# Suite 2: editors
# ==============================================================================
def run_editors_suite(h: SuiteHarness) -> int:
    h.init_dirs()
    try:
        proj_dir = h.p("proj")
        pro_path = os.path.join(proj_dir, "ecc83-pp.kicad_pro")
        sch_path = os.path.join(proj_dir, "ecc83-pp.kicad_sch")
        pcb_path = os.path.join(proj_dir, "ecc83-pp.kicad_pcb")
        bp_json = os.path.join(proj_dir, ".backplane.json")
        comp_json = os.path.join(proj_dir, "ecc83-pp.kicad_pcb.backplane.json")

        sha_bp_before = sha256_file(bp_json)
        sha_comp_before = sha256_file(comp_json)
        with open(sch_path, encoding="utf-8", errors="replace") as sf:
            sch_wires_before = sf.read().count("(wire")

        h.start_hub(embed_mode="fork")

        script = [
            {"select": 0},
            {"wait": 2800},
            {"click": "rail:Board"},
            {"wait": 2800},
            {"click": "ui:Edit"},
            {"kicad": {"wait_painted": 45000, "kind": "PCB", "tag": "pcb_init"}},
            {"mark": "host_ready"},
        ]
        cp = h.start_client(script, log_name="client.log", hold=True, exit_after=380)
        gpui_hwnd = h.wait_client_hwnd(cp)
        h.wait_for_mark("host_ready", timeout_s=75.0)

        r_init = h.last_kicad_report("pcb_init")
        kpid = int(r_init.get("pid") or 0)
        api_sock = r_init.get("sock") or ""
        cont_hwnd = int((r_init.get("container") or {}).get("xid") or 0)
        h.track_pid(kpid)

        ipc_up = h.wait_for_ipc(api_sock, timeout_s=20.0)
        ver_json = h.ipc_cmd("version", api_sock) if ipc_up else {}
        h.check(
            f"Backplane ui:Edit started KiCad (fork) + live Named Pipe GUI IPC ({ver_json})",
            kpid > 0 and ipc_up and int(ver_json.get("major") or 0) >= 10,
            ver_json,
        )

        for _ in range(50):
            try:
                if h.ipc_cmd("open-docs", api_sock, "pcb").get("count", 0) >= 1:
                    break
            except Exception:
                pass
            time.sleep(0.25)

        # 2. Switch to Schematic via Backplane's rail, draw wire interactively, Ctrl+S save, ERC + SVG export
        h.click_control(gpui_hwnd, "rail:Schematic")
        sch_win = h.wait_embedded_frame(cont_hwnd, kpid, "Schematic Editor", timeout_s=40.0)
        time.sleep(0.8)
        h.check(
            "Schematic Editor (eeschema) placed by Backplane client inside Win32 container",
            sch_win is not None and (sch_win["style"] & WS_CHILD) != 0,
            sch_win,
        )

        if sch_win:
            sx, sy, sw, sh = sch_win["root_x"], sch_win["root_y"], sch_win["w"], sch_win["h"]
            scx = int(sx + sw * 0.65)
            scy = int(sy + sh * 0.72)
            screen_click(scx, scy, 1)
            t_focus = time.time() + 3.0
            while time.time() < t_focus:
                fg_f = foreground_focus_hwnd()
                if fg_f and (fg_f == cont_hwnd or u32.IsChild(cont_hwnd, fg_f)):
                    break
                time.sleep(0.05)
            time.sleep(0.35)
            key_tap("Home")
            time.sleep(0.45)
            key_tap("w")
            time.sleep(0.30)
            screen_click(scx - 45, scy, 1)
            time.sleep(0.35)
            screen_click(scx + 95, scy, 1)
            time.sleep(0.35)
            key_tap("End")
            time.sleep(0.25)
            key_tap("Escape")
            time.sleep(0.30)
            key_chord("ctrl", "s")
            time.sleep(1.0)
        h.shot("01_schematic_edited")

        with open(sch_path, encoding="utf-8", errors="replace") as sf:
            sch_wires_after = sf.read().count("(wire")
        h.check(
            f"Embedded Schematic Editor interactive wire edit + Ctrl+S saved to disk ({sch_wires_before} -> {sch_wires_after} wires)",
            sch_wires_after > sch_wires_before,
            f"wires {sch_wires_before} -> {sch_wires_after}",
        )

        erc_json = h.p("erc.json")
        sch_svg_dir = h.p("sch_svg")
        os.makedirs(sch_svg_dir, exist_ok=True)
        subprocess.run([h.kicad_cli, "sch", "erc", "--format", "json", "-o", erc_json, sch_path], capture_output=True)
        subprocess.run([h.kicad_cli, "sch", "export", "svg", "-o", sch_svg_dir, sch_path], capture_output=True)
        erc_ok = os.path.isfile(erc_json) and "sheets" in json.load(open(erc_json, encoding="utf-8"))
        svgs = [p for p in glob.glob(os.path.join(sch_svg_dir, "*.svg")) if os.path.getsize(p) > 500]
        h.check(
            "Saved schematic passes kicad-cli sch erc JSON and exports non-empty SVG",
            erc_ok and len(svgs) >= 1,
            {"erc_ok": erc_ok, "svgs": len(svgs)},
        )

        # 3. Symbol Editor via editor.kicad.show
        key_chord("ctrl", "grave")
        time.sleep(0.15)
        h.hub_rpc([{"method": "editor.kicad.show", "params": {"project": pro_path, "frame": "SYMBOL_EDITOR", "item": "proj_syms:TestResistor"}, "timeout_ms": 45000}])
        sym_win = h.wait_embedded_frame(cont_hwnd, kpid, "Symbol Editor", timeout_s=40.0)
        time.sleep(0.7)
        h.shot("02_symbol_editor")
        h.check(
            "Symbol Editor opened and placed by Backplane client inside viewer pane",
            sym_win is not None and (sym_win["style"] & WS_CHILD) != 0,
            sym_win,
        )

        # 4. PCB Editor: switch back via Board rail, unfill/refill copper zones, add routed F.Cu track, SaveDocument
        h.click_control(gpui_hwnd, "rail:Board")
        pcb_win = h.wait_embedded_frame(cont_hwnd, kpid, "PCB Editor", timeout_s=30.0)
        time.sleep(0.6)
        h.check(
            "PCB Editor (pcbnew) placed by Backplane client on rail switch back",
            pcb_win is not None and (pcb_win["style"] & WS_CHILD) != 0,
            pcb_win,
        )

        traces_0 = h.ipc_cmd("items-count", api_sock, "pcb", "11")["count"]
        zones_0 = h.ipc_cmd("items-count", api_sock, "pcb", "16")["count"]
        bx, by, bw, bh = pcb_win["root_x"], pcb_win["root_y"], pcb_win["w"], pcb_win["h"]
        screen_click(int(bx + bw * 0.14), int(by + bh * 0.86), 1)
        time.sleep(0.35)
        if list_popup_menus(kpid):
            key_tap("Escape")
            time.sleep(0.2)
        key_tap("Home")
        time.sleep(0.5)

        h.ipc_cmd("run-action", api_sock, "pcbnew.ZoneFiller.zoneUnfillAll")
        time.sleep(0.8)
        shot_unfilled = h.shot("03a_zones_unfilled")

        h.ipc_cmd("run-action", api_sock, "pcbnew.ZoneFiller.zoneFillAll")
        time.sleep(1.0)
        shot_filled = h.shot("03b_zones_filled")

        zone_diff = img_diff_box(
            shot_unfilled,
            shot_filled,
            int(bx + 45),
            int(by + 65),
            int(max(120, bw - 260)),
            int(bh - 110),
        )
        h.check(
            f"Copper zone unfill + refill updated GAL pour geometry ({zone_diff})",
            zones_0 >= 1 and zone_diff["changed_pixels"] > 150,
            zone_diff,
        )

        h.ipc_cmd("agent-add-track", api_sock, "32.0", "96.0", "148.0", "128.0", "1.2", "3")
        time.sleep(0.4)
        h.ipc_cmd("save-doc", api_sock, "pcb")
        time.sleep(0.4)
        h.shot("03c_pcb_saved")

        traces_1 = h.ipc_cmd("items-count", api_sock, "pcb", "11")["count"]
        mod_res = h.ipc_cmd("modified", api_sock, "pcb")
        mod_after_save = bool(mod_res.get("modified")) if mod_res.get("supported") else ("*" in hwnd_title(pcb_win["hwnd"]))
        h.check(
            f"PCB track added ({traces_0} -> {traces_1}) and SaveDocument cleared modified state",
            traces_1 == traces_0 + 1 and not mod_after_save,
            {"traces": (traces_0, traces_1), "mod": mod_after_save},
        )

        # 5. Footprint Editor via editor.kicad.show
        key_chord("ctrl", "grave")
        time.sleep(0.15)
        h.hub_rpc([{"method": "editor.kicad.show", "params": {"project": pro_path, "frame": "FOOTPRINT_EDITOR"}, "timeout_ms": 45000}])
        fp_win = h.wait_embedded_frame(cont_hwnd, kpid, "Footprint Editor", timeout_s=40.0)
        time.sleep(0.7)
        h.shot("04_footprint_editor")
        h.check(
            "Footprint Editor opened and placed by Backplane client inside viewer pane",
            fp_win is not None and (fp_win["style"] & WS_CHILD) != 0,
            fp_win,
        )

        h.click_control(gpui_hwnd, "rail:Board")
        pcb_win = h.wait_embedded_frame(cont_hwnd, kpid, "PCB Editor", timeout_s=30.0)
        time.sleep(0.5)
        if pcb_win:
            screen_click(int(pcb_win["root_x"] + pcb_win["w"] * 0.14), int(pcb_win["root_y"] + pcb_win["h"] * 0.86), 1)
            time.sleep(0.3)

        # 6. DRC Dialog (GWLP_HWNDPARENT == gpui_hwnd, inside screen bounds) + CLI DRC
        drc_proc = subprocess.Popen(
            [sys.executable, IPC_PY, "run-action", api_sock, "pcbnew.DRCTool.runDRC"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            creationflags=subprocess.CREATE_NO_WINDOW,
        )
        drc_dlg = h.wait_kicad_dialog(kpid, "Design Rules Checker", timeout_s=20.0)
        time.sleep(0.55)
        if drc_dlg:
            drc_dlg["owner"] = hwnd_owner(drc_dlg["hwnd"])
        h.shot("05_drc_dialog")

        scr_w, scr_h = u32.GetSystemMetrics(0), u32.GetSystemMetrics(1)
        drc_ok = bool(
            drc_dlg
            and drc_dlg["owner"] == gpui_hwnd
            and -20 <= drc_dlg["root_x"] <= scr_w
            and -20 <= drc_dlg["root_y"] <= scr_h
        )
        h.check(
            "DRC dialog floats above GPUI with Backplane-managed GWLP_HWNDPARENT == gpui_hwnd",
            drc_ok,
            {"dialog": drc_dlg, "gpui_hwnd": gpui_hwnd},
        )
        if drc_dlg:
            u32.PostMessageW(drc_dlg["hwnd"], 0x0010, 0, 0)
            time.sleep(0.35)
            if u32.IsWindowVisible(drc_dlg["hwnd"]):
                screen_click(drc_dlg["root_x"] + 120, max(12, drc_dlg["root_y"] + 18), 1)
                time.sleep(0.15)
                key_tap("Escape")
                time.sleep(0.25)
        try:
            drc_proc.wait(timeout=5.0)
        except Exception:
            drc_proc.kill()

        drc_json = h.p("drc.json")
        subprocess.run([h.kicad_cli, "pcb", "drc", "--format", "json", "-o", drc_json, pcb_path], capture_output=True)
        cli_drc_ok = os.path.isfile(drc_json) and "violations" in json.load(open(drc_json, encoding="utf-8"))
        h.check("Saved PCB produces valid kicad-cli pcb drc JSON report", cli_drc_ok)

        # 7. 3D Viewer: switch via 3D rail, let OpenGL settle, and orbit by mouse drag
        h.click_control(gpui_hwnd, "rail:3D")
        v3d_win = h.wait_embedded_frame(cont_hwnd, kpid, "3D Viewer", timeout_s=40.0)
        time.sleep(1.5)
        shot_3d_before = h.shot("06a_3d_before_orbit")

        orbit_diff = {"changed_pixels": 0}
        if v3d_win:
            x3, y3, w3, h3 = v3d_win["root_x"], v3d_win["root_y"], v3d_win["w"], v3d_win["h"]
            cx3, cy3 = int(x3 + w3 / 2), int(y3 + h3 / 2)
            screen_click(cx3, cy3, 1)
            time.sleep(0.25)
            screen_drag(cx3 - 90, cy3 - 50, cx3 + 90, cy3 + 60)
            time.sleep(1.0)
            shot_3d_after = h.shot("06b_3d_after_orbit")
            orbit_diff = img_diff_box(shot_3d_before, shot_3d_after, x3 + 40, y3 + 60, w3 - 80, h3 - 100)

        h.check(
            f"Embedded 3D Viewer rendered OpenGL board and orbited on mouse drag ({orbit_diff})",
            v3d_win is not None and orbit_diff["changed_pixels"] > 400,
            orbit_diff,
        )

        # 8. Plot dialog + Gerbers / Drill / STEP manufacturing export + companion metadata preservation
        h.click_control(gpui_hwnd, "rail:Board")
        pcb_win = h.wait_embedded_frame(cont_hwnd, kpid, "PCB Editor", timeout_s=30.0)
        time.sleep(0.4)
        if pcb_win:
            screen_click(int(pcb_win["root_x"] + pcb_win["w"] * 0.14), int(pcb_win["root_y"] + pcb_win["h"] * 0.86), 1)
            time.sleep(0.3)

        plot_proc = subprocess.Popen(
            [sys.executable, IPC_PY, "run-action", api_sock, "common.Control.plot"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            creationflags=subprocess.CREATE_NO_WINDOW,
        )
        plot_dlg = h.wait_kicad_dialog(kpid, "Plot", timeout_s=20.0)
        plot_dlg_ok = False
        if plot_dlg:
            time.sleep(0.55)
            plot_dlg["owner"] = hwnd_owner(plot_dlg["hwnd"])
            h.shot("07_plot_dialog")
            u32.PostMessageW(plot_dlg["hwnd"], 0x0010, 0, 0)
            time.sleep(0.35)
            if u32.IsWindowVisible(plot_dlg["hwnd"]):
                screen_click(plot_dlg["root_x"] + plot_dlg["w"] // 2, max(12, plot_dlg["root_y"] + 18), 1)
                time.sleep(0.15)
                key_tap("Escape")
            try:
                plot_proc.wait(timeout=8.0)
            except Exception:
                plot_proc.kill()
            time.sleep(0.35)
            plot_dlg_ok = (
                plot_dlg["owner"] == gpui_hwnd
                and not u32.IsWindowVisible(plot_dlg["hwnd"])
                and h.wait_for_ipc(api_sock, timeout_s=8.0)
            )
        else:
            plot_proc.kill()

        h.check(
            "Plot dialog opened cleanly over embedded PCB Editor (GWLP_HWNDPARENT == gpui_hwnd) and dismissed without hang",
            plot_dlg_ok,
            plot_dlg,
        )

        gdir = os.path.join(proj_dir, "mfg", "gerbers")
        step_f = os.path.join(proj_dir, "mfg", "ecc83-pp.step")
        os.makedirs(gdir, exist_ok=True)
        subprocess.run([h.kicad_cli, "pcb", "export", "gerbers", "-o", gdir + os.sep, pcb_path], capture_output=True)
        subprocess.run([h.kicad_cli, "pcb", "export", "drill", "-o", gdir + os.sep, pcb_path], capture_output=True)
        subprocess.run(
            [h.kicad_cli, "pcb", "export", "step", "--board-only", "--subst-models", "-o", step_f, pcb_path],
            capture_output=True,
        )
        gbrs = [p for p in glob.glob(os.path.join(gdir, "*.g*")) if os.path.getsize(p) > 100]
        drls = [p for p in glob.glob(os.path.join(gdir, "*.drl")) if os.path.getsize(p) > 50]
        step_ok = os.path.isfile(step_f) and os.path.getsize(step_f) > 1000
        h.check(
            "Manufacturing export produced valid Gerbers, Excellon drill, and STEP model",
            len(gbrs) >= 4 and len(drls) >= 1 and step_ok,
            {"gbrs": len(gbrs), "drls": len(drls), "step_ok": step_ok},
        )

        sha_bp_after = sha256_file(bp_json)
        comp_ok = False
        if os.path.isfile(comp_json):
            comp_ok = sha256_file(comp_json) == sha_comp_before
        else:
            with open(pcb_path, encoding="utf-8", errors="replace") as pf:
                comp_ok = '(custom_property "backplane_note" "preserve-across-gui-save-and-plot")' in pf.read()
        h.check(
            "Companion metadata (.backplane.json & custom_properties) preserved across GUI save and plot",
            sha_bp_after == sha_bp_before and comp_ok,
            {"sha_bp": sha_bp_after, "comp_ok": comp_ok},
        )

        h.hub_rpc([{"method": "editor.kicad.close", "params": {"project": pro_path, "save": "discard"}}])
        return h.finish()
    finally:
        h.cleanup()


# ==============================================================================
# Suite 3: resilience
# ==============================================================================
def _extract_hub_field(rpc_out: str, field: str) -> str:
    val = ""
    for line in rpc_out.splitlines():
        if line.startswith("INFO "):
            obj = (json.loads(line[5:]).get("value") or {})
            if obj.get(field):
                val = str(obj[field])
        elif line.startswith("{"):
            obj = (json.loads(line).get("reply") or json.loads(line).get("result") or {})
            if isinstance(obj, dict) and obj.get(field):
                val = str(obj[field])
    return val


def run_resilience_suite(h: SuiteHarness) -> int:
    h.init_dirs()
    try:
        proj_dir = h.p("proj")
        pro_path = os.path.join(proj_dir, "ecc83-pp.kicad_pro")

        h.start_hub(embed_mode="fork")

        script1 = [
            {"select": 0},
            {"wait": 2800},
            {"click": "rail:Board"},
            {"wait": 2800},
            {"click": "ui:Edit"},
            {"kicad": {"wait_painted": 45000, "kind": "PCB", "tag": "pcb_init"}},
            {"mark": "host_ready"},
        ]
        cp = h.start_client(script1, log_name="client.log", hold=True, exit_after=360)
        gpui_hwnd = h.wait_client_hwnd(cp)
        h.wait_for_mark("host_ready", timeout_s=75.0)

        r_init = h.last_kicad_report("pcb_init")
        kpid = int(r_init.get("pid") or 0)
        api_sock = r_init.get("sock") or ""
        cont1_hwnd = int((r_init.get("container") or {}).get("xid") or 0)
        h.track_pid(kpid)

        h.wait_for_ipc(api_sock, timeout_s=20.0)
        open_docs = {"count": 0, "documents": []}
        for _ in range(50):
            try:
                open_docs = h.ipc_cmd("open-docs", api_sock, "pcb")
                if open_docs.get("count", 0) >= 1:
                    break
            except Exception:
                pass
            time.sleep(0.25)

        ipc_ok = (
            open_docs.get("count") == 1
            and "ecc83-pp.kicad_pcb" in open_docs["documents"][0].get("board_filename", "")
        )
        h.check(
            "Backplane-launched PCB Editor (fork) exposes live GUI IPC Named Pipe for ecc83-pp.kicad_pcb",
            ipc_ok,
            open_docs,
        )

        # 2. Live agent BeginCommit -> CreateItems(Track) -> EndCommit -> visual GAL repaint -> human Ctrl+Z
        pcb_win = h.wait_embedded_frame(cont1_hwnd, kpid, "PCB Editor", timeout_s=20.0)
        bx, by, bw, bh = pcb_win["root_x"], pcb_win["root_y"], pcb_win["w"], pcb_win["h"]
        gal_x = int(bx + bw * 0.14)
        gal_y = int(by + bh * 0.86)

        screen_click(gal_x, gal_y, 1)
        time.sleep(0.35)
        if list_popup_menus(kpid):
            key_tap("Escape")
            time.sleep(0.2)
        key_tap("Home")
        time.sleep(0.6)

        n0 = h.ipc_cmd("items-count", api_sock, "pcb", "11")["count"]
        shot_before = h.shot("01a_before_agent_commit")

        h.ipc_cmd("agent-add-track", api_sock, "28.0", "94.0", "155.0", "132.0", "2.8", "3")
        time.sleep(0.8)
        shot_after = h.shot("01b_after_agent_commit")

        n1 = h.ipc_cmd("items-count", api_sock, "pcb", "11")["count"]
        mod1 = bool(h.ipc_cmd("modified", api_sock, "pcb").get("modified"))
        agent_diff = img_diff_box(
            shot_before,
            shot_after,
            int(bx + 45),
            int(by + 65),
            int(max(120, bw - 260)),
            int(bh - 110),
        )
        h.check(
            f"Live agent IPC commit incremented PCB traces ({n0} -> {n1}) and visibly repainted embedded GAL canvas ({agent_diff})",
            n1 == n0 + 1 and mod1 and agent_diff["changed_pixels"] > 40,
            agent_diff,
        )

        screen_click(gal_x, gal_y, 1)
        t_focus = time.time() + 3.0
        while time.time() < t_focus:
            fg_f = foreground_focus_hwnd()
            if fg_f and (fg_f == cont1_hwnd or u32.IsChild(cont1_hwnd, fg_f)):
                break
            time.sleep(0.05)
        time.sleep(0.25)
        if list_popup_menus(kpid):
            key_tap("Escape")
            time.sleep(0.2)
        key_chord("ctrl", "z")
        time.sleep(0.7)
        h.shot("02_after_human_undo")
        n_undo = h.ipc_cmd("items-count", api_sock, "pcb", "11")["count"]
        kdesc = os.path.join(h.kicad_dir, "share", "backplane", "api.desc")
        per_actor = False
        try:
            with open(kdesc, "rb") as df:
                per_actor = b"UndoAgentChange" in df.read()
        except OSError:
            pass
        if per_actor:
            # bp.11+ co-edit, per-actor undo: the person's Ctrl+Z leaves the agent's edit alone; the Agents pane's "Undo this change"
            # (UndoAgentChange over IPC) takes it back (as test/kicad_embed/embed_agent_resilience_e2e.sh)
            h.check(
                f"Human Ctrl+Z leaves the agent's IPC commit alone (per-actor undo: {n1} -> {n_undo})",
                n_undo == n1,
                f"expected {n1} got {n_undo}",
            )
            try:
                ua = h.ipc_cmd("undo-agent-latest", api_sock)
            except Exception as e:
                ua = {"error": str(e)}
            time.sleep(0.6)
            n_undo = h.ipc_cmd("items-count", api_sock, "pcb", "11")["count"]
            h.check(
                f"The agent's change taken back (UndoAgentChange, as the Agents pane does: {n1} -> {n_undo})",
                n_undo == n0,
                f"expected {n0} got {n_undo}: {ua}",
            )
        else:
            h.check(
                f"Human Ctrl+Z inside embedded PCB Editor atomically undid agent's IPC commit ({n1} -> {n_undo})",
                n_undo == n0,
                f"expected {n0} got {n_undo}",
            )

        # 3. Child freeze (NtSuspendProcess) isolation + RegisterHotKey grab_escape (Ctrl+`) host rescue
        h.ipc_cmd("agent-add-track", api_sock, "35.0", "98.0", "140.0", "126.0", "2.0", "3")
        time.sleep(0.4)
        screen_click(gal_x, gal_y, 1)
        time.sleep(0.4)

        foc_before = foreground_focus_hwnd()
        foc_in_child_before = bool(foc_before and (foc_before == cont1_hwnd or u32.IsChild(cont1_hwnd, foc_before)))

        suspended = suspend_pid(kpid, True)
        try:
            t0 = time.time()
            key_chord("ctrl", "grave")
            time.sleep(0.3)
            foc_after = foreground_focus_hwnd()
            elapsed_ms = int((time.time() - t0) * 1000)
            foc_in_child_after = bool(foc_after and u32.IsChild(cont1_hwnd, foc_after) and hwnd_pid(foc_after) == kpid)
            hub_alive = h.hub_hello()
        finally:
            suspend_pid(kpid, False)

        ping_after_resume = h.wait_for_ipc(api_sock, timeout_s=8.0)
        h.check(
            f"While KiCad is NtSuspendProcess-frozen, RegisterHotKey Ctrl+` returns focus to GPUI in {elapsed_ms}ms and hub stays responsive",
            suspended
            and foc_in_child_before
            and not foc_in_child_after
            and elapsed_ms < 1500
            and hub_alive
            and ping_after_resume,
            {
                "suspended": suspended,
                "foc_before": foc_in_child_before,
                "foc_after": foc_in_child_after,
                "elapsed_ms": elapsed_ms,
                "hub_alive": hub_alive,
                "ipc_after_resume": ping_after_resume,
            },
        )

        # 4. Dirty-close guard (editor.kicad.close save=ask)
        time.sleep(0.25)
        close_ask = h.hub_rpc([{"method": "editor.kicad.close", "params": {"project": pro_path, "save": "ask"}}])
        refused_dirty = False
        for line in close_ask.splitlines():
            if line.startswith("{"):
                d = json.loads(line)
                rep = d.get("reply") or d.get("result") or {}
                if d.get("ok") is True and isinstance(rep, dict) and rep.get("closed") is False:
                    refused_dirty = True
        h.check(
            "editor.kicad.close(save=ask) refuses to close while PCB has unsaved edits",
            refused_dirty,
            close_ask.strip(),
        )

        # 5. Same-client ui:View -> ui:Edit container reuse preserving unsaved edits + hard GPUI client kill observation
        h.click_control(gpui_hwnd, "ui:View")
        time.sleep(1.0)
        h.click_control(gpui_hwnd, "ui:Edit")
        # bp.11: the client makes a fresh container on re-entry and moves KiCad's holder into it (as on X11), so look under any.
        pcb_warm = None
        end_warm = time.time() + 15.0
        while pcb_warm is None and time.time() < end_warm:
            for ch in list_child_windows(gpui_hwnd):
                p = int(u32.GetAncestor(ch["hwnd"], GA_PARENT) or 0)
                cont = int(u32.GetAncestor(p, GA_PARENT) or 0) if p and hwnd_class(p) == "BackplaneKiCadEmbedHolder" else p
                if cont and is_frame_parented_in(ch["hwnd"], cont):
                    pcb_warm = h.wait_embedded_frame(cont, kpid, "PCB Editor", timeout_s=0.3)
                    if pcb_warm is not None:
                        break
            if pcb_warm is None:
                time.sleep(0.25)
        n_warm = h.ipc_cmd("items-count", api_sock, "pcb", "11")["count"]
        mod_warm = bool(h.ipc_cmd("modified", api_sock, "pcb").get("modified"))
        h.shot("03_after_view_edit_reattach")
        h.check(
            f"Detaching to ui:View and re-entering ui:Edit re-embeds warm KiCad preserving unsaved edits (traces={n_warm})",
            pcb_warm is not None and mod_warm and n_warm == n0 + 1,
            {"pcb_warm": pcb_warm, "mod_warm": mod_warm, "n_warm": n_warm},
        )

        # Also probe hard TerminateProcess of GPUI client #1 -> fresh GPUI client #2 + hub survival
        hard_kill_pid(cp.pid)
        time.sleep(0.8)
        hub_survived_client_kill = h.hub_hello()
        kp_survived_client_kill = pid_alive(kpid)
        print(
            f"DIAG after hard TerminateProcess(client1): hub_alive={hub_survived_client_kill} kicad_alive={kp_survived_client_kill}",
            flush=True,
        )

        # Start fresh GPUI client #2 (if bp.8's destroyed container_1 tore down child HWND on Windows, restart recovers it)
        script2 = [
            {"select": 0},
            {"wait": 2800},
            {"click": "rail:Board"},
            {"wait": 2800},
            {"click": "ui:Edit"},
            {"wait": 3000},
            {"mark": "client2_up"},
        ]
        cp2 = h.start_client(script2, log_name="client2.log", hold=True, exit_after=320)
        gpui2_hwnd = h.wait_client_hwnd(cp2)
        h.wait_for_mark("client2_up", log_name="client2.log", timeout_s=45.0)
        # Wait for client #2's attach (including any Windows dead-container auto-restart + LocalHistory dialog) to settle
        for _ in range(120):
            info_c2 = h.hub_rpc([])
            st_c2 = _extract_hub_field(info_c2, "state")
            pid_c2 = int(_extract_hub_field(info_c2, "pid") or 0)
            if pid_c2 > 0:
                h.track_pid(pid_c2)
                for w in list_top_windows(pid_c2):
                    tl = w["title"].lower()
                    if (
                        w["owner"] == gpui2_hwnd
                        or "recover" in tl
                        or "unsaved" in tl
                        or "already open" in tl
                    ) and "pcb editor" not in tl and "schematic editor" not in tl:
                        screen_click(w["root_x"] + w["w"] // 2, w["root_y"] + 20, 1)
                        time.sleep(0.15)
                        key_tap("Return")
                        time.sleep(0.35)
                if st_c2 == "ready":
                    c2_children = [
                        ch for ch in list_child_windows(gpui2_hwnd)
                        if ch["visible"] and ch["pid"] == pid_c2 and "pcb editor" in ch["title"].lower() and ch["w"] >= 150
                    ]
                    if c2_children:
                        break
            time.sleep(0.25)
        h.check(
            "Hard TerminateProcess of GPUI client #1 leaves hub alive and fresh client #2 reconnects cleanly",
            hub_survived_client_kill and gpui2_hwnd != 0,
            {"hub_alive": hub_survived_client_kill, "gpui2_hwnd": gpui2_hwnd, "kp_survived": kp_survived_client_kill},
        )

        # 6. Hard KiCad crash (TerminateProcess) recovery via editor.kicad.restart & zero-orphan teardown
        cur_kpid = int(_extract_hub_field(h.hub_rpc([]), "pid") or kpid or 0)
        h.track_pid(cur_kpid)
        if pid_alive(kpid):
            hard_kill_pid(kpid)
        if cur_kpid != kpid and pid_alive(cur_kpid):
            hard_kill_pid(cur_kpid)
        kpid = cur_kpid
        time.sleep(0.9)

        gpui2_still_up = bool(gpui2_hwnd and u32.IsWindow(gpui2_hwnd))
        hub_after_kill = h.hub_hello()
        h.check(
            "Hard TerminateProcess of embedded KiCad leaves Backplane GPUI window and hub alive without cascading crash",
            gpui2_still_up and hub_after_kill,
            {"gpui2_still_up": gpui2_still_up, "hub_after_kill": hub_after_kill},
        )

        restart_res_holder = {"out": ""}

        def _do_restart():
            restart_res_holder["out"] = h.hub_rpc(
                [{"method": "editor.kicad.restart", "timeout_ms": 50000, "params": {"project": pro_path, "frame": "PCB"}}]
            )

        rt = threading.Thread(target=_do_restart, daemon=True)
        rt.start()

        kp2 = 0
        for _ in range(80):
            info_out = h.hub_rpc([])
            cand = int(_extract_hub_field(info_out, "pid") or 0)
            if cand > 0 and cand != kpid:
                kp2 = cand
                break
            time.sleep(0.25)
        h.track_pid(kp2)

        # Dismiss any post-crash LocalHistory "Recover Unsaved Changes" / lock dialog while waiting for PCB Editor to embed
        pcb2_win = None
        for _ in range(160):
            if kp2 > 0:
                for w in list_top_windows(kp2):
                    tl = w["title"].lower()
                    if (
                        w["owner"] == gpui2_hwnd
                        or "recover" in tl
                        or "unsaved" in tl
                        or "already open" in tl
                    ) and "pcb editor" not in tl and "schematic editor" not in tl:
                        screen_click(w["root_x"] + w["w"] // 2, w["root_y"] + 20, 1)
                        time.sleep(0.15)
                        key_tap("Return")
                        time.sleep(0.35)
                children = [
                    ch for ch in list_child_windows(gpui2_hwnd)
                    if ch["visible"] and ch["pid"] == kp2 and "pcb editor" in ch["title"].lower() and ch["w"] >= 150
                ]
                if children:
                    pcb2_win = children[0]
                    break
            time.sleep(0.25)

        rt.join(timeout=15.0)
        api2_sock = _extract_hub_field(restart_res_holder["out"], "sock") or _extract_hub_field(h.hub_rpc([]), "sock")
        reopen_ipc = h.wait_for_ipc(api2_sock, timeout_s=15.0)
        time.sleep(0.7)
        h.shot("04_reopened_after_crash")

        h.check(
            f"editor.kicad.restart spawned fresh KiCad (pid={kp2} != {kpid}) and client re-embedded it cleanly",
            kp2 > 0 and kp2 != kpid and reopen_ipc and pcb2_win is not None and (pcb2_win["style"] & WS_CHILD) != 0,
            {"kp1": kpid, "kp2": kp2, "reopen_ipc": reopen_ipc, "pcb2_win": pcb2_win},
        )

        h.hub_rpc([{"method": "editor.kicad.close", "params": {"project": pro_path, "save": "discard"}}])
        kill_pid_tree(cp2.pid)
        for _ in range(35):
            if not pid_alive(kp2):
                break
            time.sleep(0.2)

        clean_ok = not pid_alive(kpid) and not pid_alive(kp2)
        h.check("editor.kicad.close left zero running KiCad processes", clean_ok, {"kp1": pid_alive(kpid), "kp2": pid_alive(kp2)})

        return h.finish()
    finally:
        h.cleanup()


def _screen_box_luma(x: int, y: int, w: int, h: int) -> float:
    from PIL import ImageGrab, ImageStat

    im = ImageGrab.grab(
        bbox=(int(x), int(y), int(x + max(1, w)), int(y + max(1, h))),
        all_screens=True,
    ).convert("L")
    return float(ImageStat.Stat(im).mean[0])


def _extract_hub_infos(raw: str, prefix: str) -> list:
    out = []
    for line in (raw or "").splitlines():
        if not line.startswith("INFO "):
            continue
        try:
            d = json.loads(line[5:])
            if (d.get("key") or "").startswith(prefix):
                out.append(d)
        except Exception:
            pass
    return out


def _atomic_append_pcb_track(pcb_path: str, y_mm: float):
    import uuid

    with open(pcb_path, encoding="utf-8") as f:
        s = f.read().rstrip()
    if not s.endswith(")"):
        raise RuntimeError(f"unexpected pcb tail in {pcb_path}")
    seg = f'\n\t(segment (start 120 {y_mm}) (end 140 {y_mm}) (width 0.25) (layer "F.Cu") (net 0) (uuid "{uuid.uuid4()}"))\n)'
    tmp = pcb_path + ".tmp"
    with open(tmp, "w", encoding="utf-8", newline="\n") as f:
        f.write(s[:-1] + seg + "\n")
    os.replace(tmp, pcb_path)


def _box_centre(box: dict) -> tuple[float, float]:
    return ((box["x_min"] + box["x_max"]) / 2.0, (box["y_min"] + box["y_max"]) / 2.0)


# ==============================================================================
# Suite 4: bp9 (Tabs, Full size, Pop out/in, Live theme, Reload & conflict,
#               Cross-probe both ways, Comment to Backplane, Alt+V / Alt+H)
# ==============================================================================
def run_bp9_suite(h: SuiteHarness) -> int:
    desc_path = os.path.join(h.kicad_dir, "share", "backplane", "api.desc")
    has_bp9 = False
    if os.path.isfile(desc_path):
        with open(desc_path, "rb") as df:
            raw_desc = df.read()
            has_bp9 = b"SetTheme" in raw_desc and b"ReloadDocument" in raw_desc
    if not has_bp9:
        print(f"SKIP win_embed_bp9_e2e: {desc_path} has no SetTheme/ReloadDocument (needs bp.9+)", flush=True)
        return 0

    h.init_dirs(seed_msg="The tube is U1; its plate resistor is R1 and C1 couples the output.")
    try:
        h.start_hub(embed_mode="fork")
        pro_path = h.p("proj", "ecc83-pp.kicad_pro")
        pcb_path = h.p("proj", "ecc83-pp.kicad_pcb")

        # 1. Tabs (PCB -> SCH -> FOOTPRINT_EDITOR -> PCB), Full size on/off, Pop out / Pop back in
        script = [
            {"select": 0},
            {"wait": 2600},
            {"click": "rail:Board"},
            {"wait": 2600},
            {"click": "ui:Edit"},
            {"kicad": {"wait_painted": 45000, "kind": "PCB", "tag": "ux_pcb"}},
            {"click": "kicad-tab:SCH"},
            {"kicad": {"wait_painted": 35000, "kind": "SCH", "tag": "ux_sch"}},
            {"click": "kicad-tab:FOOTPRINT_EDITOR"},
            {"kicad": {"wait_painted": 45000, "kind": "FOOTPRINT_EDITOR", "tag": "ux_fp"}},
            {"click": "kicad-tab:PCB"},
            {"wait": 2200},
            {"kicad": {"report": True, "tag": "ux_pcb2"}},
            {"click": "kicad:full"},
            {"wait": 2200},
            {"kicad": {"report": True, "tag": "ux_full"}},
            {"mark": "shot_full"},
            {"wait": 1200},
            {"click": "kicad:full"},
            {"wait": 2000},
            {"kicad": {"report": True, "tag": "ux_unfull"}},
            {"click": "kicad:popout"},
            {"wait": 4500},
            {"kicad": {"report": True, "tag": "ux_popped"}},
            {"mark": "shot_popped"},
            {"wait": 1500},
            {"click": "kicad:popin"},
            {"wait": 3800},
            {"kicad": {"report": True, "tag": "ux_back"}},
            {"mark": "ux_done"},
        ]
        cp = h.start_client(script, hold=True, exit_after=360)
        gpui_hwnd = h.wait_client_hwnd(cp)

        seen_shots = set()

        def _shot_watcher():
            for mk, fn in (("shot_full", "01_full_size"), ("shot_popped", "02_popped_out")):
                if h.wait_for_mark(mk, timeout_s=150.0) and mk not in seen_shots:
                    seen_shots.add(mk)
                    time.sleep(0.35)
                    h.shot(fn)

        watcher = threading.Thread(target=_shot_watcher, daemon=True)
        watcher.start()

        ux_ok = h.wait_for_mark("ux_done", timeout_s=175.0)
        watcher.join(timeout=2.0)
        h.shot("02b_popped_back_in")

        p1 = h.last_kicad_report("ux_pcb")
        s1 = h.last_kicad_report("ux_sch")
        fp1 = h.last_kicad_report("ux_fp")
        p2 = h.last_kicad_report("ux_pcb2")
        fu = h.last_kicad_report("ux_full")
        un = h.last_kicad_report("ux_unfull")
        po = h.last_kicad_report("ux_popped")
        bk = h.last_kicad_report("ux_back")

        kpid = int(p1.get("pid") or bk.get("pid") or 0)
        api_sock = bk.get("sock") or p1.get("sock") or ""
        cont_hwnd = int((bk.get("container") or p1.get("container") or {}).get("xid") or 0)
        h.track_pid(kpid)

        h.check(
            "Tabs: Board, Schematic, and Footprints tabs each show and paint in the pane",
            ux_ok
            and shown_frame_of(p1).get("kind") == "PCB"
            and shown_frame_of(p1).get("painted", 0) > 0
            and shown_frame_of(s1).get("kind") == "SCH"
            and shown_frame_of(s1).get("painted", 0) > 0
            and shown_frame_of(fp1).get("kind") == "FOOTPRINT_EDITOR"
            and shown_frame_of(fp1).get("painted", 0) > 0,
            {"pcb": shown_frame_of(p1), "sch": shown_frame_of(s1), "fp": shown_frame_of(fp1)},
        )
        h.check(
            "Tabs: returning to Board tab keeps the same PCB frame HWND and same KiCad PID",
            bool(p1.get("pid")) and p2.get("pid") == p1.get("pid") and p2.get("shown") == p1.get("shown"),
            {"p1": (p1.get("pid"), p1.get("shown")), "p2": (p2.get("pid"), p2.get("shown"))},
        )
        cw = lambda r: int((r.get("container") or {}).get("w") or 0)
        h.check(
            "Full size: pane expands over sidebar + thread room and restores cleanly when toggled off",
            cw(fu) > cw(p2) + 250
            and shown_frame_of(fu).get("w", 0) >= cw(fu) - 6
            and abs(cw(un) - cw(p2)) <= 6,
            {"pcb2_w": cw(p2), "full_w": cw(fu), "unfull_w": cw(un)},
        )
        h.check(
            "Pop out: same KiCad PID moves into its own top-level window and paints",
            bool(po.get("toplevel"))
            and po.get("toplevel") != p1.get("toplevel")
            and po.get("pid") == p1.get("pid")
            and shown_frame_of(po).get("painted", 0) > 0,
            {"main_top": p1.get("toplevel"), "pop_top": po.get("toplevel"), "po_shown": shown_frame_of(po)},
        )
        h.check(
            "Pop back in: returns to main window's pane with exact same PCB frame HWND and KiCad PID painted",
            bk.get("toplevel") == p1.get("toplevel")
            and bk.get("pid") == p1.get("pid")
            and bk.get("shown") == p1.get("shown")
            and shown_frame_of(bk).get("painted", 0) > 0,
            {"bk_top": bk.get("toplevel"), "bk_shown": shown_frame_of(bk)},
        )

        h.wait_for_ipc(api_sock, timeout_s=20.0)

        # 2. Live theme (editor.kicad.theme: light -> dark on running editor)
        key_chord("ctrl", "grave")
        time.sleep(0.15)
        h.click_control(gpui_hwnd, "rail:Schematic")
        sch_win = h.wait_embedded_frame(cont_hwnd, kpid, "Schematic Editor", timeout_s=25.0)
        time.sleep(0.8)

        th_light_raw = h.hub_rpc([{"m": "editor.kicad.theme", "p": {"project": pcb_path, "mode": "light"}}])
        time.sleep(1.5)
        h.shot("03_theme_light")
        luma_light = -1.0
        if sch_win:
            luma_light = _screen_box_luma(
                sch_win["root_x"] + 90,
                sch_win["root_y"] + 110,
                max(120, sch_win["w"] - 180),
                max(120, sch_win["h"] - 190),
            )

        th_dark_raw = h.hub_rpc([{"m": "editor.kicad.theme", "p": {"project": pcb_path, "mode": "dark"}}])
        time.sleep(1.6)
        h.shot("04_theme_dark")
        luma_dark = 999.0
        if sch_win:
            luma_dark = _screen_box_luma(
                sch_win["root_x"] + 90,
                sch_win["root_y"] + 110,
                max(120, sch_win["w"] - 180),
                max(120, sch_win["h"] - 190),
            )

        h.check(
            f"Live theme (editor.kicad.theme): light ({luma_light:.1f}) -> dark ({luma_dark:.1f}) updates running KiCad live",
            "_builtin_default" in th_light_raw
            and "_builtin_backplane_dark" in th_dark_raw
            and luma_light > 150.0
            and luma_dark < 90.0,
            {"luma_light": luma_light, "luma_dark": luma_dark, "light_rpc": th_light_raw[:180], "dark_rpc": th_dark_raw[:180]},
        )

        # Switch back to PCB editor for reload, cross-probe, comment, and alignment checks
        key_chord("ctrl", "grave")
        time.sleep(0.15)
        h.click_control(gpui_hwnd, "rail:Board")
        pcb_win = h.wait_embedded_frame(cont_hwnd, kpid, "PCB Editor", timeout_s=25.0)
        time.sleep(0.8)

        # 3. Reload-on-disk (atomic rename) & conflict + Keep yours
        h.ipc_cmd("save-doc", api_sock, "pcb")
        time.sleep(1.4)
        n0 = h.ipc_cmd("items-count", api_sock, "pcb", "11")["count"]
        _atomic_append_pcb_track(pcb_path, 60.0)

        n1 = n0
        for _ in range(35):
            time.sleep(0.3)
            try:
                n1 = h.ipc_cmd("items-count", api_sock, "pcb", "11")["count"]
                if n1 == n0 + 1:
                    break
            except Exception:
                pass
        h.shot("05_reloaded_on_disk")
        h.check(
            f"Reload-on-disk: atomic rename write reloaded automatically in clean PCB editor ({n0} -> {n1} tracks)",
            n1 == n0 + 1,
            {"n0": n0, "n1": n1},
        )

        h.ipc_cmd("agent-add-track", api_sock, "120.0", "70.0", "140.0", "70.0", "0.25", "3")
        time.sleep(0.6)
        n2 = h.ipc_cmd("items-count", api_sock, "pcb", "11")["count"]
        _atomic_append_pcb_track(pcb_path, 80.0)

        conflicts_seen = []
        for _ in range(35):
            time.sleep(0.3)
            rel_infos = _extract_hub_infos(h.hub_rpc([]), "kicad.reload.")
            if rel_infos:
                conflicts_seen = (rel_infos[-1].get("value") or {}).get("conflicts") or []
                if len(conflicts_seen) >= 1:
                    break
        n3 = h.ipc_cmd("items-count", api_sock, "pcb", "11")["count"]
        h.shot("06_reload_conflict")
        h.check(
            f"Conflict over unsaved edits: reload held back ({n2} -> {n3}) and kicad.reload published conflicts={conflicts_seen}",
            n3 == n2 and len(conflicts_seen) >= 1,
            {"n2": n2, "n3": n3, "conflicts": conflicts_seen},
        )

        conflict_file = conflicts_seen[0] if conflicts_seen else pcb_path
        res_keep = h.hub_rpc(
            [{"m": "editor.kicad.resolve", "p": {"project": pcb_path, "file": conflict_file, "choice": "keep"}}]
        )
        time.sleep(0.8)
        n4 = h.ipc_cmd("items-count", api_sock, "pcb", "11")["count"]
        rel_after = _extract_hub_infos(h.hub_rpc([]), "kicad.reload.")
        conflicts_after = (rel_after[-1].get("value") or {}).get("conflicts") or [] if rel_after else []
        h.check(
            "Keep yours (editor.kicad.resolve): unsaved edits stay and conflicts clear to 0",
            '"ok":true' in res_keep.replace(" ", "") and n4 == n2 and len(conflicts_after) == 0,
            {"n4": n4, "n2": n2, "conflicts_after": conflicts_after, "res_keep": res_keep[:200]},
        )
        h.ipc_cmd("save-doc", api_sock, "pcb")
        time.sleep(1.0)

        # 4. Cross-probe both ways (Chat -> KiCad, RPC -> KiCad, KiCad click -> kicad.sel & View mode, View chat click -> zoom)
        for _ in range(3):
            try:
                h.ipc_cmd("run-action", api_sock, "common.Control.zoomOutCenter")
            except Exception:
                pass
            time.sleep(0.2)
        time.sleep(0.6)
        pcb_win = h.wait_embedded_frame(cont_hwnd, kpid, "PCB Editor", timeout_s=15.0) or pcb_win or {"root_x": 800, "root_y": 150, "w": 600, "h": 700}
        bx, by, bw, bh = pcb_win["root_x"], pcb_win["root_y"], pcb_win["w"], pcb_win["h"]

        shot_xp_before = h.shot("07a_before_xprobe_u1")
        l_xp0 = h.log_line_count()
        key_chord("ctrl", "grave")
        time.sleep(0.15)
        clicked_u1_link = h.click_link(gpui_hwnd, "kicad:U1", timeout_s=12.0)
        for _ in range(40):
            if h.last_gpui_log("xprobe_select", l_xp0):
                break
            time.sleep(0.25)
        time.sleep(1.1)
        shot_xp_after = h.shot("07b_after_xprobe_u1")
        sel_u1 = h.ipc_cmd("selection", api_sock, "pcb")
        xp_diff = img_diff_box(shot_xp_before, shot_xp_after, bx + 30, by + 60, max(80, bw - 60), max(80, bh - 120))
        h.check(
            f"Cross-probe (chat U1 -> KiCad): clicking U1 link selected U1 in PCB editor and zoomed view ({xp_diff})",
            clicked_u1_link and "U1" in (sel_u1.get("refs") or []) and xp_diff["changed_pixels"] > 400,
            {"clicked_u1_link": clicked_u1_link, "sel": sel_u1, "diff": xp_diff},
        )

        r2_raw = h.hub_rpc(
            [{"m": "editor.kicad.select", "p": {"project": pro_path, "references": ["C1", "Q99"], "zoom": True}}]
        )
        sel_c1 = h.ipc_cmd("selection", api_sock, "pcb")
        x2_ok = False
        for line in r2_raw.splitlines():
            if not line or line.startswith("INFO "):
                continue
            d = json.loads(line)
            rep_d = d.get("reply") or {}
            if (
                d.get("ok") is True
                and rep_d.get("kicad") is True
                and "C1" in (rep_d.get("found") or {}).get("references", [])
                and (rep_d.get("probe") or "").startswith("footprint C1")
            ):
                x2_ok = True
                break
        h.check(
            "Cross-probe (editor.kicad.select RPC): found C1 in KiCad and set viewer probe on Windows",
            x2_ok and "C1" in (sel_c1.get("refs") or []),
            {"sel_c1": sel_c1, "rpc": r2_raw[:240]},
        )

        # Person click in KiCad -> ET_SELECTION_CHANGED -> kicad.sel.<digest>
        h.hub_rpc([{"m": "editor.kicad.select", "p": {"project": pro_path, "references": ["U1"], "zoom": True}}])
        time.sleep(1.0)
        cx_mid, cy_mid = int(bx + bw // 2), int(by + bh // 2)
        screen_click(cx_mid, cy_mid, 1)
        time.sleep(0.5)
        key_tap("Escape")
        time.sleep(0.35)
        screen_click(cx_mid, cy_mid, 1)
        time.sleep(0.35)
        screen_click(cx_mid, cy_mid, 1)

        sel_info_val = {}
        for _ in range(30):
            time.sleep(0.3)
            sinfos = _extract_hub_infos(h.hub_rpc([]), "kicad.sel.")
            if sinfos:
                cand = sinfos[-1].get("value") or {}
                cs = cand.get("selection") or {}
                if cand.get("frame") == "PCB" and (
                    "U1" in (cs.get("references") or [])
                    or any(p.startswith("U1.") for p in (cs.get("pins") or []))
                ):
                    sel_info_val = cand
                    break
        h.check(
            "Cross-probe (KiCad click -> kicad.sel): person's click on U1 emitted kicad.sel and set viewer probe",
            sel_info_val.get("frame") == "PCB" and "ref=U1" in (sel_info_val.get("probe") or ""),
            sel_info_val,
        )

        # Switch to View mode to confirm Board viewer lights U1 AND clicking R1 zooms Board viewer
        key_chord("ctrl", "grave")
        time.sleep(0.2)
        l_view = h.log_line_count()
        h.click_control(gpui_hwnd, "ui:View")
        for _ in range(50):
            if h.last_gpui_log("probe_lit", l_view).get("n", 0) > 0:
                break
            time.sleep(0.25)
        p3_sync = h.last_gpui_log("probe_sync", l_view)
        p3_lit = h.last_gpui_log("probe_lit", l_view)
        h.shot("08a_view_lit_u1")
        h.check(
            "Cross-probe (KiCad -> Board viewer): View mode lights U1 selected in KiCad",
            "ref=U1" in (p3_sync.get("probe") or "") and p3_lit.get("n", 0) > 0,
            {"probe_sync": p3_sync, "probe_lit": p3_lit},
        )

        l_x4 = h.log_line_count()
        clicked_r1_link = h.click_link(gpui_hwnd, "kicad:R1", timeout_s=10.0)
        for _ in range(50):
            if h.last_gpui_log("xprobe_zoom", l_x4):
                break
            time.sleep(0.25)
        time.sleep(0.8)
        z4 = h.last_gpui_log("xprobe_zoom", l_x4)
        p4_sync = h.last_gpui_log("probe_sync", l_x4)
        p4_lit = h.last_gpui_log("probe_lit", l_x4)
        h.shot("08b_view_zoom_r1")
        h.check(
            "Cross-probe (View mode chat R1 -> Board viewer): clicking R1 lit and zoomed R1",
            clicked_r1_link
            and "ref=R1" in (z4.get("probe") or "")
            and "ref=R1" in (p4_sync.get("probe") or "")
            and p4_lit.get("n", 0) > 0,
            {"z4": z4, "sync": p4_sync, "lit": p4_lit},
        )

        # Return to Edit mode on PCB for Comment to Backplane + Alt+V / Alt+H
        h.click_control(gpui_hwnd, "ui:Edit")
        pcb_win = h.wait_embedded_frame(cont_hwnd, kpid, "PCB Editor", timeout_s=25.0)
        time.sleep(1.0)

        # 5. Comment to Backplane (CommentSelection over IPC -> ET_COMMENT -> copied PNG + user message in thread)
        h.ipc_cmd("select-items", api_sock, "pcb", "U1,C1", "1")
        time.sleep(0.8)
        note_text = "Check decoupling near U1 on Windows"
        cmt_res = h.ipc_cmd("comment-selection", api_sock, "pcb", note_text)
        cmt_obj = cmt_res.get("comment") or {}

        posted_msg = None
        copied_imgs = []
        comments_dir = h.p("home", "kicad-comments")
        events_jsonl = h.p("home", "events.jsonl")
        for _ in range(40):
            time.sleep(0.3)
            if os.path.isdir(comments_dir):
                copied_imgs = [
                    os.path.join(comments_dir, fn)
                    for fn in os.listdir(comments_dir)
                    if fn.endswith(".png") and os.path.getsize(os.path.join(comments_dir, fn)) > 300
                ]
            if os.path.isfile(events_jsonl):
                with open(events_jsonl, encoding="utf-8", errors="replace") as ef:
                    for line in ef:
                        try:
                            ev = json.loads(line)
                        except Exception:
                            continue
                        if (
                            ev.get("$") == "MessagePosted"
                            and ev.get("thread") == "t1"
                            and ev.get("role") == "user"
                            and note_text in (ev.get("text") or "")
                        ):
                            posted_msg = ev
            if posted_msg and copied_imgs:
                break

        time.sleep(0.8)
        h.shot("09_comment_to_backplane")
        msg_txt = (posted_msg or {}).get("text") or ""
        h.check(
            "Comment to Backplane (CommentSelection over IPC -> copied screenshot + user message with refs & attachment in thread)",
            bool(cmt_res.get("seq"))
            and cmt_obj.get("from_api") is True
            and sorted((cmt_obj.get("names") or {}).get("references") or []) == ["C1", "U1"]
            and len(copied_imgs) >= 1
            and posted_msg is not None
            and "Selected in the KiCad board:" in msg_txt
            and "U1" in msg_txt
            and "[attachment: " in msg_txt,
            {"seq": cmt_res.get("seq"), "copied_imgs": copied_imgs, "msg_txt": msg_txt[:260]},
        )

        # 6. Alt+V / Alt+H alignment hotkeys in embedded PCB editor
        pcb_win = h.wait_embedded_frame(cont_hwnd, kpid, "PCB Editor", timeout_s=15.0) or pcb_win or {"root_x": 800, "root_y": 150, "w": 600, "h": 700}
        bx, by, bw, bh = pcb_win["root_x"], pcb_win["root_y"], pcb_win["w"], pcb_win["h"]
        safe_x, safe_y = int(bx + 90), int(by + 95)
        screen_click(safe_x, safe_y, 1)
        time.sleep(0.4)
        if list_popup_menus(kpid):
            key_tap("Escape")
            time.sleep(0.2)

        def _pcb_boxes(refs_csv: str, zoom_flag: str = "0") -> dict:
            h.ipc_cmd("select-items", api_sock, "pcb", refs_csv, zoom_flag)
            time.sleep(0.5)
            cr = h.ipc_cmd("comment-selection", api_sock, "pcb", "(probe)")
            return {
                it["reference"]: it["bbox"]
                for it in ((cr.get("comment") or {}).get("items") or [])
                if it.get("kind") == "FOOTPRINT" and it.get("bbox")
            }

        refs = ["R2", "R1", "R3"]
        before_boxes = _pcb_boxes("R2,R1,R3", "1")
        have_all_3 = set(before_boxes.keys()) == set(refs)
        anchor_x, anchor_y = _box_centre(before_boxes["R2"]) if have_all_3 else (0.0, 0.0)

        # Ensure focus is inside PCB canvas with cursor off footprints, then select R2,R1,R3 and press Alt+V
        screen_click(safe_x, safe_y, 1)
        time.sleep(0.35)
        h.ipc_cmd("select-items", api_sock, "pcb", "R2,R1,R3", "0")
        time.sleep(0.45)
        screen_move(safe_x, safe_y)
        key_chord("alt", "v")
        time.sleep(1.1)
        h.shot("10a_after_alt_v")
        after_v = _pcb_boxes("R2,R1,R3", "0") if have_all_3 else {}
        xs_v = [_box_centre(after_v[r])[0] for r in refs] if set(after_v) == set(refs) else [0.0, 99.0]
        ys_v_ok = (
            all(abs(_box_centre(after_v[r])[1] - _box_centre(before_boxes[r])[1]) < 0.02 for r in refs)
            if set(after_v) == set(refs)
            else False
        )
        h.check(
            f"Alt+V aligns selected footprints (R2, R1, R3) onto R2's X centre ({anchor_x:.3f} mm -> {[round(x, 3) for x in xs_v]})",
            have_all_3 and all(abs(x - anchor_x) <= 0.05 for x in xs_v) and ys_v_ok,
            {"anchor_x": anchor_x, "xs_v": xs_v, "ys_v_ok": ys_v_ok},
        )

        # Undo Alt+V as the person did it: their own Ctrl+Z. An IPC run-action undo comes from the harness's API client, and with per-actor
        # undo (bp.11+) that client has nothing to undo, so Alt+V's X moves stayed and Alt+H's "X unchanged" check failed on every bp.12 run.
        screen_click(safe_x, safe_y, 1)
        time.sleep(0.35)
        key_chord("ctrl", "z")
        time.sleep(0.6)
        # Alt+H is judged against the positions it starts from (whatever the undo left), not the pre-Alt+V ones
        base_h = _pcb_boxes("R2,R1,R3", "0") if have_all_3 else {}
        if set(base_h) == set(refs):
            before_boxes = base_h
            anchor_y = _box_centre(base_h["R2"])[1]

        screen_click(safe_x, safe_y, 1)
        time.sleep(0.35)
        h.ipc_cmd("select-items", api_sock, "pcb", "R2,R1,R3", "0")
        time.sleep(0.45)
        screen_move(safe_x, safe_y)
        key_chord("alt", "h")
        time.sleep(1.1)
        h.shot("10b_after_alt_h")
        after_h = _pcb_boxes("R2,R1,R3", "0") if have_all_3 else {}
        ys_h = [_box_centre(after_h[r])[1] for r in refs] if set(after_h) == set(refs) else [0.0, 99.0]
        xs_h_ok = (
            all(abs(_box_centre(after_h[r])[0] - _box_centre(before_boxes[r])[0]) < 0.02 for r in refs)
            if set(after_h) == set(refs)
            else False
        )
        h.check(
            f"Alt+H aligns selected footprints (R2, R1, R3) onto R2's Y centre ({anchor_y:.3f} mm -> {[round(y, 3) for y in ys_h]})",
            have_all_3 and all(abs(y - anchor_y) <= 0.05 for y in ys_h) and xs_h_ok,
            {"anchor_y": anchor_y, "ys_h": ys_h, "xs_h_ok": xs_h_ok},
        )
        screen_click(safe_x, safe_y, 1)
        time.sleep(0.35)
        key_chord("ctrl", "z")
        time.sleep(0.4)

        # Clean teardown
        h.hub_rpc([{"method": "editor.kicad.close", "params": {"project": pro_path, "save": "discard"}}])
        kill_pid_tree(cp.pid)
        for _ in range(35):
            if not pid_alive(kpid):
                break
            time.sleep(0.2)
        h.check("bp9 suite teardown left zero running KiCad processes", not pid_alive(kpid), {"kpid_alive": pid_alive(kpid)})

        return h.finish()
    finally:
        h.cleanup()


def main():
    if len(sys.argv) < 7:
        sys.exit(
            "usage: win_embed_e2e.py <window|editors|resilience|bp9|all> <CLIENT_EXE> <HUB_EXE> <ASSETS_DIR> <KICAD_DIR> <OUT_DIR> [PORT]"
        )
    suite = sys.argv[1]
    client_exe, hub_exe, assets_dir, kicad_dir, out_dir = sys.argv[2:7]
    port = int(sys.argv[7]) if len(sys.argv) > 7 else 3851

    runners = {
        "window": run_window_suite,
        "editors": run_editors_suite,
        "resilience": run_resilience_suite,
        "bp9": run_bp9_suite,
    }
    if suite == "all":
        rc = 0
        for idx, name in enumerate(("window", "editors", "resilience", "bp9")):
            sub_out = os.path.join(out_dir, name)
            h = SuiteHarness(name, client_exe, hub_exe, assets_dir, kicad_dir, sub_out, port + idx * 20)
            rc |= runners[name](h)
        sys.exit(rc)
    elif suite in runners:
        h = SuiteHarness(suite, client_exe, hub_exe, assets_dir, kicad_dir, out_dir, port)
        rc = runners[suite](h)
        sys.exit(rc)
    else:
        sys.exit(f"unknown suite: {suite}")


if __name__ == "__main__":
    main()
