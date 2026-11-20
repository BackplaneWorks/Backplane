"""The X focus over time, for embed_pointer_keys_e2e's race.
  focus_trace.py watch SECONDS          a line `<ms> <focus window>` at each change of the X input focus (polled every 2 ms), on $DISPLAY
  focus_trace.py judge FOCUS KEYS TOP   per key line `<k> <click ms> <key start ms> <key end ms>`: the focus the key met, and how long after
                                        the click began the focus left the window it had then (the click's hand-back)"""
import ctypes, ctypes.util, os, sys, time


def watch(seconds):
    x = ctypes.CDLL(ctypes.util.find_library("X11"))
    x.XOpenDisplay.restype = ctypes.c_void_p
    d = ctypes.c_void_p(x.XOpenDisplay(os.environ["DISPLAY"].encode()))
    w, r, last, end = ctypes.c_ulong(), ctypes.c_int(), None, time.time() + seconds
    while time.time() < end:
        x.XGetInputFocus(d, ctypes.byref(w), ctypes.byref(r))
        if w.value != last:
            last = w.value
            print(time.time_ns() // 1000000, hex(last), flush=True)
        time.sleep(0.002)


def judge(focus_path, keys_path, top):
    f = [(int(a), int(b, 16)) for a, b in (l.split() for l in open(focus_path) if len(l.split()) == 2)]
    top = int(top, 0)

    def at(t):
        v = None
        for ts, w in f:
            if ts > t:
                break
            v = w
        return v

    for l in open(keys_path):
        k, c0, k0, k1 = (int(v) for v in l.split())
        was = at(c0)
        left = next((ts for ts, w in f if ts >= c0 and w != was), None)
        where = lambda w: "the top-level itself" if w == top else hex(w or 0)
        print(f"{k}: the click's hand-back {left - c0 if left else '-'} ms after the click began; the key {k0 - c0}..{k1 - c0} ms;"
              f" focus at the key {where(at(k0))} .. {where(at(k1))}")


if __name__ == "__main__":
    if sys.argv[1] == "watch":
        watch(float(sys.argv[2]))
    else:
        judge(*sys.argv[2:5])
