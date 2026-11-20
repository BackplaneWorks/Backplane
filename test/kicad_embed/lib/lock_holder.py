#!/usr/bin/env python3
"""#1344's stand-in for the viewer's kicad-cli: holds KiCad's lock of a document (its `~<file>.lck`, KiCad's JSON, an exclusive flock as
KiCad and kicad-cli take it) from one moment to another, then lets go the way kicad-cli does (unlock, close, remove the file).

  lock_holder.py HUB_LOG PRESS_FILE DOC TAKE RELEASE [DOC TAKE RELEASE ...]

A moment is `now`, `press+S` (S seconds after PRESS_FILE appears: the shell touches it when Edit is pressed) or `phase:NAME+S` (S seconds
after the hub logs `kicad embed: start +<ms> NAME`). Each event is printed `HOLDER <epoch_ms> took|released <doc>`."""
import fcntl, json, os, socket, sys, time


def lock_path(doc):
    d, f = os.path.split(doc)
    return os.path.join(d, "~" + f + ".lck")


def at(spec, hub_log, press_file):
    """The epoch time of a moment, or None while it has not come."""
    if spec == "now":
        return 0.0
    base, _, plus = spec.partition("+")
    plus = float(plus or 0)
    if base == "press":
        return os.path.getmtime(press_file) + plus if os.path.exists(press_file) else None
    if base.startswith("phase:"):
        name = base[6:]
        try:
            for line in open(hub_log, errors="replace"):
                if "kicad embed: start +" in line and line.rstrip().endswith(" " + name):
                    return phase_seen.setdefault(name, time.time()) + plus
        except FileNotFoundError:
            pass
        return None
    raise SystemExit(f"bad moment {spec}")


phase_seen = {}


def main():
    hub_log, press_file, rest = sys.argv[1], sys.argv[2], sys.argv[3:]
    plans = [{"doc": rest[i], "take": rest[i + 1], "release": rest[i + 2], "fd": None, "done": False} for i in range(0, len(rest), 3)]
    me = {"hostname": socket.gethostname(), "username": os.environ.get("USER", "h"), "token": "issue-1344-stand-in"}
    while not all(p["done"] for p in plans):
        now = time.time()
        for p in plans:
            if p["done"]:
                continue
            if p["fd"] is None:
                t = at(p["take"], hub_log, press_file)
                if t is not None and now >= t:
                    lp = lock_path(p["doc"])
                    fd = os.open(lp, os.O_RDWR | os.O_CREAT | os.O_TRUNC, 0o644)
                    os.write(fd, json.dumps(me).encode())
                    fcntl.flock(fd, fcntl.LOCK_EX)
                    p["fd"] = fd
                    print(f"HOLDER {int(time.time() * 1000)} took {p['doc']}", flush=True)
            else:
                t = at(p["release"], hub_log, press_file)
                if t is not None and now >= t:
                    fcntl.flock(p["fd"], fcntl.LOCK_UN)
                    os.close(p["fd"])
                    try:
                        os.unlink(lock_path(p["doc"]))
                    except FileNotFoundError:
                        pass
                    p["done"] = True
                    print(f"HOLDER {int(time.time() * 1000)} released {p['doc']}", flush=True)
        time.sleep(0.05)


if __name__ == "__main__":
    main()
