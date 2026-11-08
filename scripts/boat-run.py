#!/usr/bin/env python3
"""boat-run.py SANDBOX COMMAND...: run COMMAND on a Boat sandbox like a local command.

Its stdout and stderr go to ours (separately, as they were written), its exit code is ours. The command runs detached on the sandbox and this process
attaches to it, so a dropped connection re-attaches and carries on from the byte it had printed instead of losing the run. A signal here (^C, a stopped
build) kills the remote command and everything it started. A transport that stays down is exit code 97 (an infrastructure failure, never a test result).
Used by scripts/lib-boat.sh; the sandbox is already prepared there.
"""
import json, os, signal, subprocess, sys, time

box, cmd = sys.argv[1], sys.argv[2:]
pid = None
done = False

def boat(*a):
    return subprocess.run(["boat", "--no-update", "exec", box, *a], capture_output=True, text=True)

def stop(sig, _frame):
    if pid is not None and not done:
        boat("--kill", str(pid))
    sys.exit(128 + sig)

for s in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
    signal.signal(s, stop)

for attempt in range(5):
    r = boat("--json", "--detach", " ".join(cmd))
    try:
        pid = json.loads(r.stdout)["pid"]
        break
    except Exception:
        time.sleep(2)
else:
    sys.stderr.write("boat-run: could not start the command on %s\n" % box)
    sys.exit(97)

out = {"stdout": 0, "stderr": 0}  # bytes already written, per stream
fails = 0
while True:
    p = subprocess.Popen(["boat", "--no-update", "exec", box, "--json", "--attach", str(pid)], stdout=subprocess.PIPE, text=True, bufsize=1)
    seen = {"stdout": 0, "stderr": 0}
    for line in p.stdout:
        try:
            j = json.loads(line)
        except ValueError:
            continue
        t = j.get("type")
        if t in ("stdout", "stderr"):
            data = j["data"].encode()
            end = seen[t] + len(data)
            skip = max(0, out[t] - seen[t])  # a re-attach replays from the start: drop what was printed
            if skip < len(data):
                fd = sys.stdout.buffer if t == "stdout" else sys.stderr.buffer
                fd.write(data[skip:]); fd.flush()
                out[t] += len(data) - skip
            seen[t] = end
            fails = 0
        elif t == "exit":
            done = True
            code = j.get("exitCode")
            sys.exit(code if code is not None else 128 + (j.get("signal") or 1))
    p.wait()
    fails += 1
    if fails > 40:
        sys.stderr.write("boat-run: lost the connection to %s\n" % box)
        sys.exit(97)
    time.sleep(3)
