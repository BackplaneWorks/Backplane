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
OK = re.compile(r"^ ok \(([0-9.]+)s")


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

    def argv(self, local):
        inner = "set -o pipefail; scripts/ci-runner-id.sh | sed 's/^/[ci-runner] /'; " + self.cmd
        if local:
            return [ADMIT, "--mem", str(self.mem), "--cores", "2", "--", "bash", "-c", inner]
        return ["scripts/boat-job.sh", "--size", "large", "--slot", self.slot, "--idle", "3600",
                "--timeout", str(self.timeout), "--budget-usd", "25", "--wait", "900", "--", "bash", "-c", inner]

    def run(self):
        t0 = time.time()
        tries = 0
        local = self.local
        while True:
            tries += 1
            self.where = "local" if local else "boat:" + self.slot
            with open(self.log, "a") as f:
                f.write("[ci-fast] %s try %d on %s\n" % (self.name, tries, self.where))
                f.flush()
                self.rc = subprocess.call(self.argv(local), stdout=f, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
            # Boat's own trouble (no exit status 70, setup 76): once more; refused (75): here
            if not local and self.rc in (70, 76) and tries == 1:
                continue
            if not local and self.rc == 75 and os.environ.get("LOCAL_CI_NO_LOCAL") != "1":
                local = True
                continue
            break
        self.secs = time.time() - t0

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
    known = runner_known(kind) or "unknown runner\n"
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
        run = [n for n in names if not has(g, keyof(g, n, known))]
        return run, [n for n in names if n not in run]

    summary, jobs = {}, []
    if "check" in steps:
        # the parts of check.sh that are not bend, cheap: always, here
        pre = subprocess.run(["sh", "-c", "python3 scripts/i18n-gen.py --check && ! grep -rnI -E '^(<<<<<<<|>>>>>>>)( |$)' --exclude-dir=.git --exclude-dir=node_modules --exclude-dir=build --exclude-dir=dist --include='*.bend' --include='*.c' --include='*.h' --include='*.js' --include='*.ts' --include='*.mjs' --include='*.py' --include='*.sh' --include='*.yml' --include='*.json' ."], capture_output=True, text=True)
        open(os.path.join(logs, "check-pre.log"), "w").write(pre.stdout + pre.stderr)
        run, cached = todo("check")
        summary["check"] = {"run": run, "cached": cached, "pre": pre.returncode}
        if run:
            sel = "" if sorted(run) == sorted(K.check_files(files)) else "CHECK_FILES=%s " % shlex.quote(" ".join(run))
            jobs.append(Job("check", "ci-check", "CHECK_JOBS=1 %sscripts/check.sh check" % sel, 1800, MEM["check"], logs, a.local))
    if "proof" in steps:
        run, cached = todo("proof")
        summary["proof"] = {"run": run, "cached": cached}
        if run:
            jobs.append(Job("proof", "ci-proof", "scripts/check.sh proof", 1800, MEM["proof"], logs, a.local))
    shards = []
    if "tests" in steps:
        run, cached = todo("test")
        summary["tests"] = {"run": run, "cached": cached}
        if run:
            times = timings()
            fill = sorted(times.values())[len(times) // 2] if times else 20.0
            total = sum(times.get(t, fill) for t in run)
            n = max(1, min(a.shards, len(run), math.ceil(total / TARGET)))
            for i, (g, secs) in enumerate(deal(run, times, n), 1):
                cmd = "TEST_JOBS=%s TEST_ONLY=%s scripts/test.sh" % (TEST_JOBS, shlex.quote(" ".join(g)))
                j = Job("tests-%d" % i, "ci-t%d" % i, cmd, 1800, MEM["tests"], logs, a.local)
                j.tests, j.est = g, secs
                shards.append(j)
                jobs.append(j)
            say("tests: %d to run in %d shards (about %.0f test-s), %d reused" % (len(run), len(shards), total, len(cached)))
    if "js" in steps:
        run, cached = todo("js")
        summary["js"] = {"run": run, "cached": cached}
        if run:
            jobs.append(Job("js", "ci-js", JS_CMD, 1800, MEM["js"], logs, a.local))
    for g in ("check", "proof"):
        if g in summary:
            s = summary[g]
            say("%s: %d to run, %d reused" % (g, len(s["run"]), len(s["cached"])))
    if "js" in summary:
        say("js: %s" % ("runs" if summary["js"]["run"] else "reused"))

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

    results = {}
    newtimes = {}
    for j in jobs:
        if j.name == "check":
            results["check"] = j.rc
            record("check", summary["check"]["run"], j)
        elif j.name == "proof":
            results["proof"] = j.rc
            record("proof", ["PROOF.bend"], j)
        elif j.name == "js":
            results["js"] = j.rc
            record("js", ["js"], j)
    tok, tfail = 0, []
    for j in shards:
        verdict = {}
        for l in j.last().splitlines():
            m = VERDICT.match(l)
            if m and m.group(1) in j.tests:
                verdict.setdefault(m.group(1), []).append(m.group(2))
        good = []
        for t in j.tests:
            v = verdict.get(t, [])
            m = OK.match(v[0]) if len(v) == 1 else None
            if m:
                good.append(t)
                newtimes[t] = float(m.group(1))
            else:
                tfail.append(t)
        tok += len(good)
        if j.rc == 0 and len(good) == len(j.tests):
            record("test", good, j)
        elif j.rc == 0:
            # exit 0 without a verdict for each test is not a pass
            j.rc = 1
    save_timings(newtimes)
    if shards:
        results["tests"] = max(j.rc for j in shards)

    # the logs as local-ci.sh reads them
    lines = []
    for g in ("check", "proof", "tests", "js"):
        if g not in summary:
            continue
        s = summary[g]
        rc = results.get(g, 0)
        if g == "check" and s["pre"] != 0:
            rc = rc or 1
        open(os.path.join(logs, g + ".rc"), "w").write("%d\n" % rc)
        if g == "tests":
            word = "%d run (%d ok, %d fail) in %d shards, %d cached" % (len(s["run"]), tok, len(tfail), len(shards), len(s["cached"]))
            with open(os.path.join(logs, "tests.log"), "w") as f:
                for t in s["cached"]:
                    f.write("%s: cached ok\n" % t)
                for j in shards:
                    f.write("== %s (%s, rc %s, %.0fs, est %.0f test-s): %s\n" % (j.name, j.where, j.rc, j.secs, j.est, " ".join(j.tests)))
                    f.write("\n".join(l for l in j.last().splitlines() if VERDICT.match(l) or l.startswith("FAIL") or l.startswith("[boat-job]") or "synced" in l or "pushed in" in l or "sandbox bx" in l) + "\n")
        else:
            word = "%d run, %d cached" % (len(s["run"]), len(s["cached"])) if g == "check" else ("run" if s["run"] else "cached")
            if not s["run"]:
                open(os.path.join(logs, g + ".log"), "w").write("%s: cached ok (%s)\n" % (g, ", ".join(s["cached"]) if g != "check" else "%d files" % len(s["cached"])))
        lines.append("%s=%s %s" % (g, "ok" if rc == 0 else "FAIL(rc %d)" % rc, word))
    for j in jobs:
        lines.append("  %s on %s: rc %s in %.0fs" % (j.name, j.where, j.rc, j.secs))
    lines.append("wall %.0fs" % (time.time() - t0))
    open(os.path.join(logs, "summary.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines), flush=True)
    return 0 if all(results.get(g, 0) == 0 for g in results) and summary.get("check", {}).get("pre", 0) == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
