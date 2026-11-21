"""The X focus over time, for embed_pointer_keys_e2e's race.
  focus_trace.py watch SECONDS          a line `<ms> <focus window>` at each change of the X input focus (polled every 2 ms), on $DISPLAY
  focus_trace.py judge FOCUS KEYS TOP   per key line `<k> <click ms> <key start ms> <key end ms>`: the focus the key met, and how long after
                                        the click began the focus left the window it had then (the click's hand-back)
  focus_trace.py gap TOP N              the window manager's part played N times: the X focus put on top-level TOP itself, and how long until
                                        the client moved it on (its focus proxy redirect), polled every 0.2 ms; a line per try and a summary"""
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


def gap(top, n):
    x = ctypes.CDLL(ctypes.util.find_library("X11"))
    x.XOpenDisplay.restype = ctypes.c_void_p
    d = ctypes.c_void_p(x.XOpenDisplay(os.environ["DISPLAY"].encode()))
    top, w, r, out = int(top, 0), ctypes.c_ulong(), ctypes.c_int(), []
    for i in range(int(n)):
        x.XSetInputFocus(d, ctypes.c_ulong(top), 2, 0)  # RevertToParent, CurrentTime: as a window manager's click-to-focus does
        x.XSync(d, 0)
        t0, moved = time.perf_counter(), None
        while time.perf_counter() - t0 < 0.5:
            x.XGetInputFocus(d, ctypes.byref(w), ctypes.byref(r))
            if w.value != top:
                moved = (time.perf_counter() - t0) * 1000, w.value
                break
            time.sleep(0.0002)
        out.append(moved[0] if moved else None)
        print(f"try {i + 1}: " + (f"moved to {hex(moved[1])} after {moved[0]:.1f} ms" if moved else "left on the top-level (500 ms)"), flush=True)
        time.sleep(0.3)  # (over the redirect cap's 100 ms span: each try is a burst of its own)
    got = sorted(v for v in out if v is not None)
    print(f"summary: moved {len(got)}/{len(out)}" + (f", median {got[len(got) // 2]:.1f} ms, max {got[-1]:.1f} ms" if got else ""), flush=True)


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
    if sys.argv[1] == "gap":
        gap(sys.argv[2], sys.argv[3])
    elif sys.argv[1] == "watch":
        watch(float(sys.argv[2]))
    else:
        judge(*sys.argv[2:5])
