#!/usr/bin/env python3
"""Run the gates on this checkout, reusing passes recorded for byte-identical
inputs (scripts/ci-keys.py), and sending the rest to Boat sandboxes kept warm
per slot (scripts/boat-job.sh --slot) in parallel. bp-coord's local-ci.sh and
scripts/focus.sh run it.

  ci-fast.py --logs DIR [--full] [--focus BASE] [--steps check,proof,tests,js]
             [--shards N] [--local]

  --full          reuse nothing (every test, every file's check, the proof and
                  the js step run); passes are still recorded
  --focus BASE    only what the diff from BASE to the working tree can change
                  (scripts/ci-keys.py affected); the js step only when asked
                  with --steps
  --steps         which gates (default check,proof,tests,js)
  --shards N      at most N test shards (default 16, LOCAL_CI_SHARDS)
  --local         run every job here through bp-admit instead of on Boat

Records (only from a job that exited 0, only under the runner text the job
itself printed, and only when the inputs hash the same after every job as
before): $BACKPLANE_VERIFIED (default ~/.cache/backplane-bend/verified)/
{test,check,proof,js}/<key>. A test is recorded from its own verdict line
"test/X_test.bend: ok (...)" of a shard that exited 0 and printed no other
verdict for it. Boat sandboxes never write them: they only print.

Writes DIR/<step>.log, DIR/<step>.rc, DIR/tests-<n>.log and DIR/summary.txt
(one line per step: what ran, what was reused). Exit 0 only when every step
passed.
"""
import argparse
import fcntl
import importlib.util
import json
import math
import os
import re
import shlex
import subprocess
import sys
import threading
import time

# no __pycache__ beside ci-keys.py: an untracked file there would be an input
sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("ci_keys", os.path.join(HERE, "ci-keys.py"))
K = importlib.util.module_from_spec(spec)
spec.loader.exec_module(K)

CACHE = os.environ.get("BACKPLANE_VERIFIED") or os.path.join(
    os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache"), "backplane-bend", "verified")
ADMIT = os.path.expanduser(os.environ.get("BP_ADMIT", "~/Documents/i2cjak/bp-coord/bin/bp-admit.sh"))
TARGET = float(os.environ.get("LOCAL_CI_SHARD_SECS", "100"))  # test-seconds per shard
TEST_JOBS = os.environ.get("LOCAL_CI_TEST_JOBS", "2")
JS_CMD = ("BACKPLANE_JS_TARGET=linux-x64 scripts/build-js.sh src/server/main.bend dist-js/linux-x64/backplane-serve"
          " && bend src/web/index.html -o build/web >/dev/null"
          " && scripts/js-smoke.sh dist-js/linux-x64/backplane-serve build/web"
          " && bun test/tools/js_bake_test.ts && python3 test/tools/service_release_test.py"
          " && bend test/wire/index.html -o build/wire >/dev/null"
          " && BACKPLANE_SMOKE_FULL=1 scripts/js-smoke.sh dist-js/linux-x64/backplane-serve build/web")
# real peaks (GB) for a local run through bp-admit
MEM = {"check": 12, "proof": 16, "js": 13, "tests": 12}
RUNNER = re.compile(r"^\[ci-runner\] (.*)$")
VERDICT = re.compile(r"^(test/[^ :]+_test\.bend):(.*)$")
OK = re.compile(r"^ ok \(([0-9.]+)s(?:, ([0-9.]+) GB)?\)")
BOAT_ROOT = os.environ.get("BOAT_JOB_ROOT") or os.path.join(
    os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state"), "backplane", "boat")
MAX_NEW = int(os.environ.get("LOCAL_CI_MAX_NEW", "16"))  # new Boat sandboxes a run may start
PRIORITY = os.environ.get("LOCAL_CI_PRIORITY") == "1"     # a merge gate: bp-admit --priority


def warm_slots():
    """our ci-* slots with a warm sandbox now (boat-job.sh's status.json), longest left first"""
    try:
        d = json.load(open(os.path.join(BOAT_ROOT, "status.json")))
    except (OSError, ValueError):
        return []
    now = time.time()
    got = {}
    for x in d.get("sandboxes", []):
        sl = x.get("slot") or ""
        if x.get("state") == "warm" and sl.startswith("ci-") and (x.get("idle_until") or 0) > now + 120 and x.get("size") == "large":
            got[sl] = max(got.get(sl, 0), x["idle_until"])
    return sorted(got, key=lambda k: -got[k])


def say(*a):
    print("ci-fast:", *a, flush=True)


def runner_known(kind):
    try:
        return open(os.path.join(CACHE, "runner", kind + ".txt")).read()
    except OSError:
        return ""


def has(gate, key):
    return os.path.isfile(os.path.join(CACHE, gate, key))


def mark(gate, key, what):
    d = os.path.join(CACHE, gate)
    os.makedirs(d, exist_ok=True)
    tmp = os.path.join(d, ".%s.%d" % (key, os.getpid()))
    with open(tmp, "w") as f:
        f.write(what + "\n")
    os.replace(tmp, os.path.join(d, key))


def timings():
    t = K_timings_file(os.path.join("scripts", "test-timings.txt"))
    t.update(K_timings_file(os.path.join(CACHE, "timings.txt")))
    return t


def K_timings_file(p):
    out = {}
    try:
        for row in open(p).read().splitlines():
            row = row.strip()
            if not row or row.startswith("#"):
                continue
            s, _, n = row.partition(" ")
            try:
                out[n.strip()] = float(s)
            except ValueError:
                pass
    except OSError:
        pass
    return out


def save_timings(new):
    if not new:
        return
    t = K_timings_file(os.path.join(CACHE, "timings.txt"))
    t.update(new)
    os.makedirs(CACHE, exist_ok=True)
    tmp = os.path.join(CACHE, ".timings.%d" % os.getpid())
    with open(tmp, "w") as f:
        for n, s in sorted(t.items(), key=lambda x: (-x[1], x[0])):
            f.write("%.2f %s\n" % (s, n))
    os.replace(tmp, os.path.join(CACHE, "timings.txt"))


def peaks():
    return K_timings_file(os.path.join(CACHE, "peaks.txt"))


def save_peaks(new):
    if not new:
        return
    t = peaks()
    t.update(new)
    tmp = os.path.join(CACHE, ".peaks.%d" % os.getpid())
    with open(tmp, "w") as f:
        for n, s in sorted(t.items(), key=lambda x: (-x[1], x[0])):
            f.write("%.1f %s\n" % (s, n))
    os.replace(tmp, os.path.join(CACHE, "peaks.txt"))


def shard_mem(tests, pk):
    """a shard's real peak here: its TEST_JOBS largest tests at once, and a GB over"""
    top = sorted((pk.get(t, 7.0) for t in tests), reverse=True)[: int(TEST_JOBS)]
    return max(2, min(16, math.ceil(sum(top) + 1)))


def deal(tests, times, n):
    """longest first, each to the shard with the least so far"""
    fill = sorted(times.values())[len(times) // 2] if times else 20.0
    dur = {t: times.get(t, fill) for t in tests}
    groups, tot = [[] for _ in range(n)], [0.0] * n
    for t in sorted(tests, key=lambda t: (-dur[t], t)):
        i = min(range(n), key=lambda j: (tot[j], j))
        groups[i].append(t)
        tot[i] += dur[t]
    flat = [t for g in groups for t in g]
    assert sorted(flat) == sorted(tests) and len(flat) == len(set(flat))
    return [(sorted(g), tot[i]) for i, g in enumerate(groups) if g]


class Job:
    def __init__(self, name, slot, cmd, timeout, mem, logs, local):
        self.name, self.slot, self.cmd, self.timeout, self.mem = name, slot, cmd, timeout, mem
        self.log = os.path.join(logs, name + ".log")
        self.local, self.rc, self.secs, self.where = local, None, 0, ""
        self.waited, self.ran = 0.0, 0.0

    def argv(self, local):
        inner = "set -o pipefail; scripts/ci-runner-id.sh | sed 's/^/[ci-runner] /'; " + self.cmd
        if local:
            return [ADMIT] + (["--priority"] if PRIORITY else []) + ["--mem", str(self.mem), "--cores", "2", "--", "bash", "-c", inner]
        return [os.environ.get("CI_FAST_BOAT_JOB", "scripts/boat-job.sh"), "--size", "large", "--slot", self.slot, "--idle", "3600",
                "--timeout", str(self.timeout), "--budget-usd", "25", "--wait", "900", "--", "bash", "-c", inner]

    def run(self):
        t0 = time.time()
        tries = 0
        local = self.local
        self.waited = 0.0
        while True:
            tries += 1
            self.where = "local" if local else "boat:" + self.slot
            with open(self.log, "a") as f:
                f.write("[ci-fast] %s try %d on %s\n" % (self.name, tries, self.where))
                f.flush()
                at = f.tell()
                ts = time.time()
                p = subprocess.Popen(self.argv(local), stdout=f, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
                # the wait ends when bp-admit admits it, or when Boat starts the command;
                # nothing here has a deadline: a queue is waited for, however long
                mark = "bp-admit: admitted" if local else "running: "
                began = None
                while p.poll() is None:
                    if began is None:
                        try:
                            with open(self.log, encoding="utf-8", errors="replace") as g:
                                g.seek(at)
                                if mark in g.read():
                                    began = time.time()
                        except OSError:
                            pass
                    time.sleep(1)
                self.rc = p.returncode
                self.waited += (began or time.time()) - ts
            # Boat's own trouble (no exit status 70, setup 76): once more; refused (75): here
            if not local and self.rc in (70, 76) and tries == 1:
                continue
            if not local and self.rc == 75 and os.environ.get("LOCAL_CI_NO_LOCAL") != "1":
                local = True
                continue
            break
        self.secs = time.time() - t0
        self.ran = self.secs - self.waited

    # exit codes that are not the gate's verdict: Boat refused (75), lost the
    # status (70), failed setup (76) or cleanup (77); cancelled (143), killed
    # (137), timed out (124), or a signal. Such a job is incomplete, never failed.
    NOVERDICT = {70, 75, 76, 77, 124, 137, 143}

    def state(self):
        if self.rc is None or self.rc < 0 or self.rc in self.NOVERDICT or self.rc >= 128:
            return "incomplete"
        return "ok" if self.rc == 0 else "fail"

    def text(self):
        try:
            return open(self.log, encoding="utf-8", errors="replace").read()
        except OSError:
            return ""

    def runner(self):
        """the runner text of the last try (lines after the last [ci-fast] try line)"""
        t = self.text().split("[ci-fast] %s try " % self.name)[-1]
        rows = [m.group(1) for m in (RUNNER.match(l) for l in t.splitlines()) if m]
        return "\n".join(rows) + "\n" if rows and rows[0] == "ci-runner-v1" else ""

    def last(self):
        return self.text().split("[ci-fast] %s try " % self.name)[-1]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--logs", required=True)
    ap.add_argument("--full", action="store_true")
    ap.add_argument("--focus")
    ap.add_argument("--steps")
    ap.add_argument("--shards", type=int, default=int(os.environ.get("LOCAL_CI_SHARDS", "16")))
    ap.add_argument("--local", action="store_true")
    a = ap.parse_args()
    root = subprocess.run(["git", "-C", HERE, "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip()
    os.chdir(root)
    logs = os.path.abspath(a.logs)
    os.makedirs(logs, exist_ok=True)
    steps = (a.steps or ("check,proof,tests" if a.focus else "check,proof,tests,js")).split(",")
    full = a.full or os.environ.get("LOCAL_CI_FULL") == "1"
    os.makedirs(CACHE, exist_ok=True)
    # one run at a time on this machine: the slots are shared
    lock = open(os.path.join(CACHE, ".ci-fast.lock"), "w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        say("another local CI run holds the slots; waiting for it")
        fcntl.flock(lock, fcntl.LOCK_EX)
    t0 = time.time()
    kind = "local" if a.local else "boat-large"
    # a pass counts from any runner local CI runs on (Boat's large sandboxes, or here
    # when Boat refuses): each record is keyed by the toolchain that made it, and
    # the gate itself runs on either. LOCAL_CI_RUNNERS=boat-large reuses only Boat's.
    runners = [r for r in (runner_known(k) for k in os.environ.get("LOCAL_CI_RUNNERS", "boat-large local").split()) if r]
    files = K.checkout_files()

    # what each gate reads, hashed now (and again after the jobs)
    def plan():
        p = {}
        if "tests" in steps:
            p.update({("test", t): K.test_inputs(t, files) for t in K.all_tests()})
        if "check" in steps:
            p.update({("check", f): K.check_inputs(f) for f in K.check_files(files)})
        if "proof" in steps:
            p[("proof", "PROOF.bend")] = K.proof_inputs()
        if "js" in steps:
            p[("js", "js")] = set(files)
        return p

    try:
        inputs = plan()
    except K.Fail as e:
        say("cannot key the inputs (%s): nothing is reused and nothing recorded" % e)
        inputs, full, norecord = None, True, True
    else:
        norecord = False
        before = {k: K.key(k[0], k[1], "x", v) for k, v in inputs.items()}

    def keyof(g, n, runner):
        return K.key(g, n, runner, inputs[(g, n)])

    want = {}
    if a.focus:
        out = subprocess.run([sys.executable, os.path.join(HERE, "ci-keys.py"), "affected", a.focus], capture_output=True, text=True)
        if out.returncode != 0:
            say("cannot tell what the diff touches (%s); running every step in full" % out.stderr.strip())
            a.focus = None
        else:
            aff = out.stdout.split("\n")
            want["test"] = [l[5:] for l in aff if l.startswith("test ")]
            want["check"] = [l[6:] for l in aff if l.startswith("check ")]
            want["proof"] = ["PROOF.bend"] if "proof" in aff else []
            want["js"] = ["js"] if "js" in aff else []
    if not a.focus:
        want["test"] = K.all_tests()
        want["check"] = K.check_files(files)
        want["proof"] = ["PROOF.bend"]
        want["js"] = ["js"]

    def todo(g):
        names = want[g]
        if full or inputs is None:
            return list(names), []
        run = [n for n in names if not any(has(g, keyof(g, n, r)) for r in runners)]
        return run, [n for n in names if n not in run]

    summary, jobs = {}, []
    warm = [] if a.local else warm_slots()
    if "check" in steps:
        # the parts of check.sh that are not bend, cheap: always, here
        pre = subprocess.run(["sh", "-c", "python3 scripts/i18n-gen.py --check && ! grep -rnI -E '^(<<<<<<<|>>>>>>>)( |$)' --exclude-dir=.git --exclude-dir=node_modules --exclude-dir=build --exclude-dir=dist --include='*.bend' --include='*.c' --include='*.h' --include='*.js' --include='*.ts' --include='*.mjs' --include='*.py' --include='*.sh' --include='*.yml' --include='*.json' ."], capture_output=True, text=True)
        open(os.path.join(logs, "check-pre.log"), "w").write(pre.stdout + pre.stderr)
        run, cached = todo("check")
        summary["check"] = {"run": run, "cached": cached, "pre": pre.returncode}
        if run:
            sel = "" if sorted(run) == sorted(K.check_files(files)) else "CHECK_FILES=%s " % shlex.quote(" ".join(run))
            # its real peak: the full check (server.bend among it) about 10 GB; a
            # selection without the big programs far less
            big = {"LAWS.bend", "src/server/server.bend", "src/server/main.bend", "src/app/main.bend"}
            mem = MEM["check"] if not sel or big & set(run) or len(run) > 40 else 6
            jobs.append(Job("check", "", "CHECK_JOBS=1 %sscripts/check.sh check" % sel, 1800, mem, logs, a.local))
    if "proof" in steps:
        run, cached = todo("proof")
        summary["proof"] = {"run": run, "cached": cached}
        if run:
            jobs.append(Job("proof", "", "scripts/check.sh proof", 1800, MEM["proof"], logs, a.local))
    shards = []
    if "tests" in steps:
        run, cached = todo("test")
        summary["tests"] = {"run": run, "cached": cached}
        if run:
            times = timings()
            fill = sorted(times.values())[len(times) // 2] if times else 20.0
            total = sum(times.get(t, fill) for t in run)
            n = max(1, min(a.shards, len(run), math.ceil(total / TARGET)))
            if not a.local:
                # warm slots first; new sandboxes only as the work needs, at most MAX_NEW
                # a run (the other steps take theirs first)
                cap = len(warm) + MAX_NEW - len(jobs) - (1 if "js" in steps and todo("js")[0] else 0)
                n = max(1, min(n, cap))
            pk = peaks()
            for i, (g, secs) in enumerate(deal(run, times, n), 1):
                cmd = "TEST_JOBS=%s TEST_ONLY=%s scripts/test.sh" % (TEST_JOBS, shlex.quote(" ".join(g)))
                j = Job("tests-%d" % i, "", cmd, 1800, shard_mem(g, pk), logs, a.local)
                j.tests, j.est = g, secs
                shards.append(j)
                jobs.append(j)
            say("tests: %d to run in %d shards (about %.0f test-s), %d reused" % (len(run), len(shards), total, len(cached)))
    if "js" in steps:
        run, cached = todo("js")
        summary["js"] = {"run": run, "cached": cached}
        if run:
            jobs.append(Job("js", "", JS_CMD, 1800, MEM["js"], logs, a.local))
    for g in ("check", "proof"):
        if g in summary:
            s = summary[g]
            say("%s: %d to run, %d reused" % (g, len(s["run"]), len(s["cached"])))
    if "js" in summary:
        say("js: %s" % ("runs" if summary["js"]["run"] else "reused"))

    # every job takes a slot: the warm ones first (no Boat start), then fresh names
    used = set()
    free = list(warm)
    k = 1
    for j in jobs:
        if free:
            j.slot = free.pop(0)
        else:
            while "ci-%d" % k in warm or "ci-%d" % k in used:
                k += 1
            j.slot = "ci-%d" % k
        used.add(j.slot)
    if not a.local:
        say("Boat: %d warm slots reused, %d new sandboxes at most" % (min(len(warm), len(jobs)), max(0, len(jobs) - len(warm))))
    ts = [threading.Thread(target=j.run) for j in jobs]
    for t in ts:
        t.start()
    for t in ts:
        t.join()

    # the inputs must hash as they did before the jobs, or nothing is recorded
    if not norecord:
        K._hash.clear()
        K._text.clear()
        K._deps.clear()
        try:
            files = K.checkout_files()
            again = plan()
            after = {k: K.key(k[0], k[1], "x", v) for k, v in again.items()}
        except K.Fail:
            after = None
        if after != before:
            say("the inputs changed while the jobs ran: nothing recorded")
            norecord = True
    commit = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    stamp = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

    def record(g, names, job):
        r = job.runner()
        if norecord or job.rc != 0 or not r:
            return 0
        kn = "local" if job.where == "local" else "boat-large"
        os.makedirs(os.path.join(CACHE, "runner"), exist_ok=True)
        open(os.path.join(CACHE, "runner", kn + ".txt"), "w").write(r)
        for n in names:
            mark(g, keyof(g, n, r), "%s %s %s %s %s" % (g, n, commit, stamp, job.where))
        return len(names)

    states = {}
    newtimes, newpeaks = {}, {}
    for j in jobs:
        if j.name in ("check", "proof", "js"):
            states[j.name] = [j.state()]
            record(j.name, summary[j.name]["run"] if j.name != "proof" else ["PROOF.bend"], j)
    tok, tfail, tmissing = 0, [], 0
    for j in shards:
        verdict = {}
        for l in j.last().splitlines():
            m = VERDICT.match(l)
            if m and m.group(1) in j.tests:
                verdict.setdefault(m.group(1), []).append(m.group(2))
        good, bad = [], []
        for t in j.tests:
            v = verdict.get(t, [])
            m = OK.match(v[0]) if len(v) == 1 else None
            if m:
                good.append(t)
                newtimes[t] = float(m.group(1))
                if m.group(2):
                    newpeaks[t] = float(m.group(2))
            elif v:
                bad.append(t)  # it ran to a verdict, and the verdict is not ok
        tok += len(good)
        tfail += bad
        tmissing += len(j.tests) - len(good) - len(bad)
        st = j.state()
        if bad:
            j.verdict = "fail"
        elif st == "ok" and len(good) == len(j.tests):
            j.verdict = "ok"
            record("test", good, j)
        elif st == "fail":
            j.verdict = "fail"  # test.sh itself refused the run (exit 2)
        else:
            j.verdict = "incomplete"
        states.setdefault("tests", []).append(j.verdict)
    save_timings(newtimes)
    save_peaks(newpeaks)
    if "check" in summary and summary["check"]["pre"] != 0:
        states.setdefault("check", []).append("fail")

    def worst(xs):
        return "fail" if "fail" in xs else ("incomplete" if "incomplete" in xs else "ok")

    # the logs as local-ci.sh reads them: summary.txt, one line per step, then the jobs
    lines, overall = [], []
    for g in ("check", "proof", "tests", "js"):
        if g not in summary:
            continue
        s = summary[g]
        st = worst(states.get(g, []))
        overall.append(st)
        open(os.path.join(logs, g + ".rc"), "w").write({"ok": "0", "fail": "1", "incomplete": "3"}[st] + "\n")
        mine = [j for j in jobs if (j.name.startswith("tests-") if g == "tests" else j.name == g)]
        if g == "tests":
            word = "%d run (%d ok, %d fail, %d no verdict) in %d shards, %d cached" % (len(s["run"]), tok, len(tfail), tmissing, len(shards), len(s["cached"]))
            with open(os.path.join(logs, "tests.log"), "w") as f:
                for t in s["cached"]:
                    f.write("%s: cached ok\n" % t)
                for j in shards:
                    f.write("== %s (%s, %s, rc %s, waited %.0fs, ran %.0fs, est %.0f test-s): %s\n" % (j.name, j.where, j.verdict, j.rc, j.waited, j.ran, j.est, " ".join(j.tests)))
                    f.write("\n".join(l for l in j.last().splitlines() if VERDICT.match(l) or l.startswith("FAIL") or l.startswith("[boat-job]") or "synced" in l or "pushed in" in l or "sandbox bx" in l or l.startswith("bp-admit: admitted")) + "\n")
        else:
            word = "%d run, %d cached" % (len(s["run"]), len(s["cached"])) if g == "check" else ("run" if s["run"] else "cached")
            if not s["run"]:
                open(os.path.join(logs, g + ".log"), "w").write("%s: cached ok (%s)\n" % (g, ", ".join(s["cached"]) if g != "check" else "%d files" % len(s["cached"])))
        if mine:
            nb = sum(1 for j in mine if j.where.startswith("boat"))
            word += "; %s, waited %.0fs, ran %.0fs" % (
                ", ".join(x for x in ("%d on Boat" % nb if nb else "", "%d here" % (len(mine) - nb) if len(mine) > nb else "") if x),
                max(j.waited for j in mine), max(j.ran for j in mine))
        tag = {"ok": "ok", "fail": "FAIL", "incomplete": "INCOMPLETE"}[st]
        lines.append("%s=%s %s" % (g, tag, word))
    for j in jobs:
        lines.append("  %s on %s: %s, rc %s, waited %.0fs, ran %.0fs" % (j.name, j.where, getattr(j, "verdict", j.state()), j.rc, j.waited, j.ran))
    verdict = worst(overall)
    lines.append("verdict %s" % {"ok": "PASS", "fail": "FAIL", "incomplete": "INCOMPLETE"}[verdict])
    lines.append("wall %.0fs" % (time.time() - t0))
    open(os.path.join(logs, "summary.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines), flush=True)
    # 0 pass, 1 a gate's own verdict failed, 3 a verdict never came (queued, refused, killed)
    return {"ok": 0, "fail": 1, "incomplete": 3}[verdict]

if __name__ == "__main__":
    sys.exit(main())
