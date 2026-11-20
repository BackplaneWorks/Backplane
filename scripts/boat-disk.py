#!/usr/bin/env python3
"""boat-disk.py guard|report [--home H] [--high PCT] [--low PCT] [--cap-gb N] [--lanes DIR --keep LANE]: keep a build machine's disk from filling (runs ON it).
Boat build boxes (scripts/lib-boat.sh): ~/target/<slug>. h@backplane (scripts/bp-slot.sh): --lanes ~/bp-gpui-build, one tree per lane with its target in .target;
there a lane unused for two days goes whole first (after stale incremental state), the job's own lane (--keep) and any lane a running process is in never.

The box keeps ~/target/<slug> (cargo target dirs) and the sccache cache between jobs on purpose, so they grow without bound. Before each job (`guard`):
  always       job dirs in ~/.bpjobs older than a day (a job that died before bo_finish removed its own);
  over --high  (default 80 %) remove, oldest first, until under --low (default 70 %):
               1. cargo `incremental/` entries not touched for a day, 2. every other `incremental/` entry outside the target dirs used in the last hour,
               3. dependency artifacts (`deps/`, `build/`, `.fingerprint/` entries) not touched for 3 days, 4. whole target dirs not used in the last hour, least recently used first,
               5. sccache files beyond SCCACHE_CACHE_SIZE (the box's cap, 20G by default), oldest first.
  Cargo sees a removed artifact as stale and rebuilds it; nothing else on the box is touched (no snapshot, no image, no login).
`report` prints one JSON line (pct, used_gb, size_gb, target_gb, sccache_gb, at): bo_finish saves it so `scripts/boat-pool.sh status` shows the disk while the box is stopped.

Used space: the box's home is a lazily filled FUSE mount (ascii-lazyfs) whose `df` counts only what was read back, so used = the root disk's use, less what the lazy home
has hydrated onto it (/var/lib/ascii-lazy/real), plus du of the home: what the disk holds once everything is read. Ages come from files, never a directory's own mtime
(the lazy home sets that to when it hydrated it). --cap-gb N measures the home alone against N GB (tests, or a fixed budget).
Every path removed is a literal child of ~/target, ~/.bpjobs or the sccache dir, checked after resolving; a symlink is unlinked, never followed."""
import json, os, shutil, subprocess, sys, time

GB = 1 << 30
DAY = 86400


def du(path):
    """bytes under path (disk blocks, one filesystem), 0 when missing"""
    if not os.path.lexists(path):
        return 0
    try:
        out = subprocess.run(["du", "-sxk", path], capture_output=True, text=True, timeout=600).stdout.split()
        return int(out[0]) * 1024 if out else 0
    except (OSError, ValueError, subprocess.SubprocessError):
        return 0


LAZY_REAL = "/var/lib/ascii-lazy/real"   # where the lazy home keeps what it has hydrated, on the root disk


def usage(home, cap_gb):
    """(used, size) in bytes for the box's disk: what it would hold with the home fully hydrated"""
    if cap_gb:
        return du(home), int(float(cap_gb) * GB)
    r = os.statvfs("/")
    used = (r.f_blocks - r.f_bfree) * r.f_frsize
    size = used + r.f_bavail * r.f_frsize   # as df counts it: the blocks kept for root are not ours
    if os.stat(home).st_dev != os.stat("/").st_dev:   # the lazy home: its df counts only what was read back, du counts it all
        real = 0
        try:
            out = subprocess.run(["sudo", "-n", "du", "-sxk", LAZY_REAL], capture_output=True, text=True, timeout=600).stdout.split()
            real = int(out[0]) * 1024 if out else 0
        except (OSError, ValueError, subprocess.SubprocessError):
            pass
        used = max(used - real, 0) + du(home)
    return used, size


def size_of(cap):
    """SCCACHE_CACHE_SIZE syntax (20G, 512M, bytes) to bytes"""
    s = (cap or "20G").strip().upper()
    mult = {"K": 1 << 10, "M": 1 << 20, "G": GB, "T": 1 << 40}.get(s[-1:], 1)
    try:
        return int(float(s.rstrip("KMGT") or 0) * mult)
    except ValueError:
        return 20 * GB


def last_use(path, limit=20000):
    """when a directory was last used: its newest file (mtime or atime; a lazy home sets every directory's own mtime to when it was hydrated), looking at no more than `limit` files"""
    best, n = 0.0, 0
    for cur, ds, fs in os.walk(path):
        for f in fs:
            try:
                st = os.lstat(os.path.join(cur, f))
            except OSError:
                continue
            best = max(best, st.st_mtime, st.st_atime)
            n += 1
            if n >= limit:
                return best
    return best


def target_use(tdir):
    """when a target dir was last built: its profiles' newest dependency artifact or fingerprint"""
    best = 0.0
    for p in profiles(tdir):
        for sub in ("deps", ".fingerprint"):
            for e in children(os.path.join(p, sub)):
                best = max(best, mtime(e) if not os.path.isdir(e) else last_use(e, 50))
    return best


class Pruner:
    def __init__(self, home, used, size, low, now, dry):
        self.home, self.used, self.size, self.low, self.now, self.dry = home, used, size, low, now, dry
        self.roots = [os.path.realpath(os.path.join(home, x)) for x in ("target", ".bpjobs")]
        self.removed = []   # (path, bytes)
        self.gone = set()

    def pct(self):
        return 100.0 * self.used / self.size if self.size else 0.0

    def under(self):
        return self.pct() < self.low

    def allowed(self, path, extra=()):
        if not path or not os.path.isabs(path):
            return False
        real = os.path.realpath(os.path.dirname(path))   # the parent, resolved: the entry itself may be a symlink, which is unlinked
        return any(real == r or real.startswith(r + os.sep) for r in list(self.roots) + list(extra)) and os.path.basename(path) not in ("", ".", "..")

    def rm(self, path, extra=()):
        if path in self.gone:   # a dry run leaves it there for the next step to find again
            return
        if not self.allowed(path, extra):
            print("boat-disk: refused to remove %r (outside the build dirs)" % path, file=sys.stderr)
            return
        n = du(path) if os.path.isdir(path) and not os.path.islink(path) else (os.lstat(path).st_blocks * 512 if os.path.lexists(path) else 0)
        if not self.dry:
            if os.path.islink(path) or not os.path.isdir(path):
                os.unlink(path)
            else:
                shutil.rmtree(path, ignore_errors=True)
        self.used -= n
        self.removed.append((path, n))
        self.gone.add(path)


def children(d):
    try:
        return [os.path.join(d, e) for e in os.listdir(d)]
    except OSError:
        return []


def mtime(p):
    """when p was last used: a file's newest of mtime and atime (cargo reads a dependency it links), a directory's newest file"""
    if os.path.isdir(p) and not os.path.islink(p):
        return last_use(p)
    try:
        st = os.lstat(p)
        return max(st.st_mtime, st.st_atime)
    except OSError:
        return 0


def profiles(tdir):
    """the profile dirs of one target dir: debug, fast, release and <triple>/<profile>"""
    out = []
    for p in children(tdir):
        if not os.path.isdir(p) or os.path.islink(p):
            continue
        if os.path.isdir(os.path.join(p, "deps")) or os.path.isdir(os.path.join(p, "incremental")) or os.path.isdir(os.path.join(p, ".fingerprint")):
            out.append(p)
        else:
            out.extend(q for q in children(p) if os.path.isdir(q) and not os.path.islink(q) and os.path.isdir(os.path.join(q, "deps")))
    return out


def busy(units):
    """the units a running process of ours is inside (its working directory): a job in a slot may write nothing for a long while"""
    out = set()
    for pid in os.listdir("/proc") if os.path.isdir("/proc") else []:
        if not pid.isdigit():
            continue
        try:
            cwd = os.readlink("/proc/%s/cwd" % pid)
        except OSError:
            continue
        out.update(u for u in units if cwd == u or cwd.startswith(u + os.sep))
    return out


def guard(home, high, low, cap_gb, dry=False, now=None, lanes=None, keep=()):
    """lanes: a dir of lane trees, each with its own target dir in .target (h@backplane's ~/bp-gpui-build, scripts/bp-slot.sh); else the box's ~/target/<slug>"""
    now = time.time() if now is None else now
    used, size = usage(lanes or home, cap_gb)
    pr = Pruner(home, used, size, low, now, dry)
    if lanes:
        pr.roots.append(os.path.realpath(lanes))
    start = pr.pct()
    for j in sorted(children(os.path.join(home, ".bpjobs")), key=mtime):
        if now - mtime(j) > DAY:
            pr.rm(j)
    if start >= high:
        base = lanes or os.path.join(home, "target")
        units = [os.path.realpath(p) for p in children(base) if os.path.isdir(p) and not os.path.islink(p) and not os.path.basename(p).startswith(".")]
        tgt = (lambda u: os.path.join(u, ".target")) if lanes else (lambda u: u)
        last = {}
        for u in units:
            last[u] = target_use(tgt(u))
            if lanes and not last[u]:   # never built: its tree's newest file
                last[u] = last_use(u, 2000)
        hot = {u for u in units if now - last[u] < 3600 or os.path.basename(u) in keep} | busy(units)
        incr = [e for u in units for p in profiles(tgt(u)) for e in children(os.path.join(p, "incremental"))]
        inhot = lambda e: any(e.startswith(u + os.sep) for u in hot)
        stale_incr = ([e for e in incr if now - mtime(e) > DAY and not inhot(e)] if lanes else [e for e in incr if now - mtime(e) > DAY], mtime)
        other_incr = ([e for e in incr if not inhot(e)], mtime)
        old_deps = ([e for u in units if u not in hot for p in profiles(tgt(u)) for sub in ("deps", "build", ".fingerprint") for e in children(os.path.join(p, sub))
                     if now - mtime(e) > 3 * DAY] if lanes else
                    [e for u in units for p in profiles(tgt(u)) for sub in ("deps", "build", ".fingerprint") for e in children(os.path.join(p, sub)) if now - mtime(e) > 3 * DAY], mtime)
        by_use = lambda u: last[u]
        cold = ([u for u in units if u not in hot], by_use)
        # a lane tree is a whole copy of a worktree: one unused for two days (its branch landed or was left) goes before any warm build state
        steps = [stale_incr, ([u for u in units if u not in hot and now - last[u] > 2 * DAY], by_use), other_incr, old_deps, cold] if lanes else \
                [stale_incr, other_incr, old_deps, cold]
        for cands, key in steps:
            for p in sorted(cands, key=key):
                if pr.under():
                    break
                if os.path.lexists(p):
                    pr.rm(p)
            if pr.under():
                break
    # sccache: its own SCCACHE_CACHE_SIZE only holds while its server runs; the files over the cap go, oldest first
    sc = os.environ.get("SCCACHE_DIR") or os.path.join(home, ".cache", "sccache")
    cap = size_of(os.environ.get("SCCACHE_CACHE_SIZE"))
    if os.path.isdir(sc) and du(sc) > cap:
        files = []
        for cur, ds, fs in os.walk(sc):
            for f in fs:
                p = os.path.join(cur, f)
                try:
                    st = os.lstat(p)
                    files.append((max(st.st_atime, st.st_mtime), st.st_blocks * 512, p))
                except OSError:
                    pass
        total = sum(f[1] for f in files)
        for _, n, p in sorted(files):
            if total <= cap * 0.9:
                break
            pr.rm(p, extra=[os.path.realpath(sc)])
            total -= n
    freed = sum(n for _, n in pr.removed)
    if pr.removed:
        print("boat-disk: %.0f%% -> %.0f%% of %.1fG, removed %.2fG in %d paths%s" % (start, pr.pct(), size / GB, freed / GB, len(pr.removed), " (dry run)" if dry else ""))
        for p, n in sorted(pr.removed, key=lambda x: -x[1])[:15]:
            print("boat-disk:   %.2fG %s" % (n / GB, os.path.relpath(p, home)))
    if pr.pct() >= high:
        print("boat-disk: still %.0f%% full after pruning (over %d%%)" % (pr.pct(), high))
    return pr


def report(home, cap_gb, lanes=None):
    used, size = usage(lanes or home, cap_gb)
    sc = os.environ.get("SCCACHE_DIR") or os.path.join(home, ".cache", "sccache")
    print(json.dumps({"at": int(time.time()), "pct": round(100.0 * used / size, 1) if size else 0, "used_gb": round(used / GB, 1), "size_gb": round(size / GB, 1),
                      "target_gb": round(du(lanes or os.path.join(home, "target")) / GB, 1), "sccache_gb": round(du(sc) / GB, 1)}))


def main(argv):
    if not argv or argv[0] not in ("guard", "report"):
        print(__doc__.split("\n")[0], file=sys.stderr)
        return 2
    opts = {"--home": os.path.expanduser("~"), "--high": "80", "--low": "70", "--cap-gb": "", "--lanes": "", "--keep": ""}
    dry, rest = False, argv[1:]
    while rest:
        a = rest.pop(0)
        if a == "--dry-run":
            dry = True
        elif a in opts and rest:
            opts[a] = rest.pop(0)
        else:
            print("boat-disk: unknown argument %r" % a, file=sys.stderr)
            return 2
    home = os.path.realpath(opts["--home"])
    if not os.path.isdir(home) or home == "/":
        print("boat-disk: no home %r" % home, file=sys.stderr)
        return 2
    lanes = os.path.realpath(opts["--lanes"]) if opts["--lanes"] else None
    if lanes and (not os.path.isdir(lanes) or lanes in ("/", home)):
        print("boat-disk: no lanes dir %r" % opts["--lanes"], file=sys.stderr)
        return 2
    if argv[0] == "report":
        report(home, opts["--cap-gb"], lanes)
    else:
        guard(home, float(opts["--high"]), float(opts["--low"]), opts["--cap-gb"], dry, lanes=lanes, keep=tuple(k for k in opts["--keep"].split(",") if k))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
