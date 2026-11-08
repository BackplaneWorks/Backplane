"""input.py click X Y | type TEXT | key NAME: real input in this desktop session (SendInput), screen coordinates. Run it in the session (sess.sh)."""
import ctypes, sys, time
from ctypes import wintypes as W
u32 = ctypes.WinDLL('user32'); u32.VkKeyScanW.restype = ctypes.c_short
try: ctypes.WinDLL('shcore').SetProcessDpiAwareness(2)
except Exception: pass
class MI(ctypes.Structure): _fields_ = [('dx', W.LONG), ('dy', W.LONG), ('mouseData', W.DWORD), ('dwFlags', W.DWORD), ('time', W.DWORD), ('dwExtraInfo', ctypes.c_size_t)]
class KI(ctypes.Structure): _fields_ = [('wVk', W.WORD), ('wScan', W.WORD), ('dwFlags', W.DWORD), ('time', W.DWORD), ('dwExtraInfo', ctypes.c_size_t)]
class HI(ctypes.Structure): _fields_ = [('uMsg', W.DWORD), ('wParamL', W.WORD), ('wParamH', W.WORD)]
class U(ctypes.Union): _fields_ = [('mi', MI), ('ki', KI), ('hi', HI)]
class IN(ctypes.Structure): _fields_ = [('type', W.DWORD), ('u', U)]
def send(*xs): a = (IN * len(xs))(*xs); u32.SendInput(len(xs), a, ctypes.sizeof(IN)); time.sleep(0.04)
def m(f): i = IN(type=0); i.u.mi = MI(0, 0, 0, f, 0, 0); return i
def k(vk=0, sc=0, f=0): i = IN(type=1); i.u.ki = KI(vk, sc, f, 0, 0); return i
VK = {'Return': 0x0D, 'Escape': 0x1B, 'Tab': 0x09, 'space': 0x20}
cmd, a = sys.argv[1], sys.argv[2:]
if cmd == 'click': u32.SetCursorPos(int(a[0]), int(a[1])); time.sleep(0.1); send(m(2)); time.sleep(0.06); send(m(4))
elif cmd == 'key': v = VK[a[0]]; send(k(v), k(v, 0, 2))
elif cmd == 'type':
    for ch in a[0]:
        r = u32.VkKeyScanW(ord(ch)) if ord(ch) < 0x80 else -1
        if r == -1: send(k(0, ord(ch), 4), k(0, ord(ch), 6)); continue
        v, sh = r & 0xff, (r >> 8) & 1
        if sh: send(k(0x10))
        send(k(v), k(v, 0, 2))
        if sh: send(k(0x10, 0, 2))
