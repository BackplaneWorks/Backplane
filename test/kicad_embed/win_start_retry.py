"""KiCad's start path on Windows (Lane K's one retry, 08611646a; Lane B's Try again, cb434891d), the Windows twin of
embed_start_retry_e2e.sh.

  win_start_retry.py <first|1|2> <CLIENT_EXE> <HUB_EXE> <ASSETS_DIR> <KICAD_DIR> <OUT_DIR> [PORT]

first: a plain start (no knobs, no warm start), to read what a real start waited for: point KICAD_DIR at a freshly staged
       runtime and the first start is the one the antivirus reads the new files in. Prints the hub's start lines.
1:     the hub's first start stalls (BACKPLANE_KICAD_TEST_STALLS=1, a 20 s window): the hub tries once more by itself,
       Edit shows the editor, the pane never says KiCad could not start.
2:     both starts stall: the pane says KiCad could not start, offers Try again (kicad:retry), and Try again shows the editor.
"""

import glob
import json
import os
import re
import subprocess
import sys
import time

import win_embed_e2e as E

START_LINE = re.compile(r"kicad embed: .*(start|started|starting it once more|did not)", re.I)


def hub_start_lines(h):
    try:
        text = open(h.p("hub.log"), encoding="utf-8", errors="replace").read()
    except OSError:
        return []
    return [l.strip() for l in text.splitlines() if START_LINE.search(l)]


def kicad_state(h, tag):
    """What the retry meets on Windows: every kicad.exe (pid, start time), KiCad's top-level windows with their titles
    (a modal prompt shows here even while KiCad's IPC is silent), the project's lock files, and a screenshot."""
    ps = ("Get-CimInstance Win32_Process -Filter \"Name='kicad.exe'\" | ForEach-Object { '{0} {1:HH:mm:ss.fff}' -f "
          "$_.ProcessId, $_.CreationDate }")
    procs = subprocess.run(["powershell", "-NoProfile", "-Command", ps], capture_output=True, text=True, timeout=30).stdout.split("\n")
    pids = {int(l.split()[0]) for l in procs if l.strip()}
    wins = []

    @E.WNDENUMPROC
    def cb(hw, _):
        if E.hwnd_pid(int(hw or 0)) in pids and E.u32.IsWindowVisible(hw):
            wins.append({"pid": E.hwnd_pid(int(hw)), "class": E.hwnd_class(int(hw)), "title": E.hwnd_title(int(hw))})
        return True

    E.u32.EnumWindows(cb, 0)
    locks = [os.path.basename(f) for f in glob.glob(h.p("proj", "*.lck")) + glob.glob(h.p("proj", "~*"))]
    shot = h.shot(f"retry_{tag}")
    print("DIAG retry_state " + json.dumps({"at": tag, "kicad": [l.strip() for l in procs if l.strip()], "windows": wins,
                                            "locks": sorted(set(locks)), "shot": shot}), flush=True)


def watch_retry(h, timeout_s=120):
    """Once the hub says it starts KiCad once more, look 4, 8 and 12 s later (the second start's socket came at 5-8 s, and
    then no Ping on bp-win, twice)."""
    end = time.time() + timeout_s
    while time.time() < end:
        try:
            if "starting it once more" in open(h.p("hub.log"), encoding="utf-8", errors="replace").read():
                break
        except OSError:
            pass
        time.sleep(0.25)
    else:
        return
    t0 = time.time()
    for at in (4, 8, 12):
        time.sleep(max(0.0, t0 + at - time.time()))
        kicad_state(h, f"{at}s")


def run(h, mode):
    if mode != "first":
        os.environ["BACKPLANE_KICAD_TEST_STALLS"] = mode
        # 20 s, not Linux's 8: on bp-win the hub's second start had its socket at 5.1 s and missed Ping within 8 (start-1)
        os.environ.setdefault("BACKPLANE_KICAD_START_WINDOW_S", "20")
    os.environ["BACKPLANE_KICAD_WARM"] = "0"  # the Edit's start is the first one
    h.init_dirs()
    try:
        h.start_hub(embed_mode="fork")
        s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 4500}, {"click": "ui:Edit", "within": 10000}]  # 4.5 s: a client without `within` clicked too early (2b)
        if mode == "2":
            window_ms = int(os.environ.get("BACKPLANE_KICAD_START_WINDOW_S", "20")) * 1000
            s += [{"wait": 2 * window_ms + 10000}, {"kicad": {"report": True, "tag": "failed"}}, {"mark": "failed"}, {"wait": 2500},
                  {"click": "kicad:retry", "within": 5000}, {"kicad": {"wait_painted": 90000, "kind": "PCB", "tag": "retried"}}]
        else:
            s += [{"kicad": {"wait_painted": 90000, "kind": "PCB", "tag": "edit"}}]
        s += [{"mark": "done"}, {"wait": 1500}]
        h.start_client(s, exit_after=300)
        if mode != "first":
            watch_retry(h)
        if mode == "2" and h.wait_for_mark("failed", timeout_s=200):
            h.shot("failed_pane")  # the pane's words and its Try again button, for the person
        h.wait_for_mark("done", timeout_s=290)
        lines = hub_start_lines(h)
        for l in lines:
            print("INFO " + l[:400], flush=True)
        log = open(h.p("client.log"), encoding="utf-8", errors="replace").read()
        once_more = sum("starting it once more" in l for l in lines)
        if mode == "first":
            r = h.last_kicad_report("edit")
            h.check("a plain start shows the editor", r.get("state") == "ready", {k: r.get(k) for k in ("state", "wait_ms", "say")})
        elif mode == "1":
            r = h.last_kicad_report("edit")
            h.check("one stalled start: the hub tried once more by itself", once_more == 1 and any("started after a missed start window" in l for l in lines), lines[-3:])
            h.check("one stalled start: Edit shows the editor", r.get("state") == "ready", {k: r.get(k) for k in ("state", "wait_ms", "say")})
            h.check("one stalled start: the pane never said KiCad could not start", '"state":"failed"' not in log)
        else:
            f, r = h.last_kicad_report("failed"), h.last_kicad_report("retried")
            h.check("two stalled starts: the hub retried once, then the pane says KiCad could not start",
                    once_more == 1 and "could not start" in (f.get("say") or ""), {"say": (f.get("say") or "")[:240], "once_more": once_more})
            h.check("two stalled starts: the pane offers Try again (kicad:retry)", '"id":"kicad:retry","found":false' not in log and "click:kicad:retry" in log)
            h.check("two stalled starts: Try again starts KiCad and shows the editor", r.get("state") == "ready", {k: r.get(k) for k in ("state", "wait_ms", "say")})
        return h.finish()
    finally:
        h.cleanup()


if __name__ == "__main__":
    if len(sys.argv) not in (7, 8) or sys.argv[1] not in ("first", "1", "2"):
        sys.exit(__doc__)
    mode, args = sys.argv[1], sys.argv[2:7]
    port = int(sys.argv[7]) if len(sys.argv) == 8 else 3951
    sys.exit(E.guarded_run(lambda: run(E.SuiteHarness("start_retry_" + mode, *args, port), mode), args[0], args[3]))
