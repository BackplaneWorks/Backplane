#!/usr/bin/env python3
"""One project list across machines (docs/ALWAYS-ON.md section 5): three temp-home hubs, the way the person's are after the home moved to boat-hub.
  boat-hub (the hub the clients are on) has no copy of the repository; desktop and framework each have one (origins in two forms) with a thread.
  boat-hub is linked to both. Judged on boat-hub's native sidebar (view.open, what GPUI draws) and data/1 projects.list (what the phones draw):
  - the repository is ONE row, not "repo on desktop" and "repo on framework", holding both machines' threads, each with its machine's mark;
  - the row's machine chips name both machines; a project with no remote stays per machine.
Then desktop is stopped: the row stays (from the registry on the home), its chip dimmed "desktop is off", its thread still listed.

  split.py --hub PATH/backplane-hub [--port 3911] [--hold DIR] [--expect split]
  --expect split: the checks expect today's split (the "before" run), and only report what they see.
  --hold DIR: write DIR/rig.json ({url, token_file} of boat-hub) and keep the hubs up until DIR/stop exists (screenshots).
"""
import argparse, json, os, shutil, subprocess, sys, tempfile, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from proto import Hub, check, report, finish  # noqa: E402
from link import rpc  # noqa: E402
from phone_machines import Data  # noqa: E402
import bots, unified  # noqa: E402

NAMES = {"bh": "boat-hub", "desk": "desktop", "fw": "framework"}


def seed_home(root, name):
    home = os.path.join(root, name, "home"); os.makedirs(home, exist_ok=True)
    notes = os.path.join(root, name, "notes"); os.makedirs(notes, exist_ok=True)
    ev = [{"$": "SettingSet", "key": k, "value": v, "at": int(time.time()) - 90} for k, v in (("onboard.done", "1"), ("onboard.uber", "1"), ("restart.continue", "off"))]
    ev += [{"$": "ProjectCreated", "id": "p5", "title": "notes-" + name, "root": notes, "at": int(time.time()) - 80}]
    with open(os.path.join(home, "events.jsonl"), "w") as f:
        for e in ev: f.write(json.dumps(e) + "\n")


def side(hub):
    ps, _ = unified.sidebar(hub)
    return ps


def summary(ps):
    return [(p.get("title"), p.get("machine"), p.get("key"), [m.get("short") for m in (p.get("machineChips") or p.get("marks") or [])], [r.get("id", "").split("|")[-1] for r in p.get("threads", [])]) for p in ps]


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--hub", required=True); a.add_argument("--port", type=int, default=3911); a.add_argument("--hold", default=""); a.add_argument("--expect", default="merged")
    o = a.parse_args()
    bun = shutil.which("bun")
    if not bun:
        report("one project list", "not run", "bun (the stand-in agent's runtime) is not installed"); return finish()
    base = os.environ.get("BP_TMP", os.path.expanduser("~/.cache/bp-tmp")); os.makedirs(base, exist_ok=True)
    root = tempfile.mkdtemp(prefix="bpt-split-", dir=base)
    agents = os.path.join(root, "bin"); os.makedirs(agents)
    with open(os.path.join(agents, "claude"), "w") as f: f.write(f'#!/bin/sh\nexec {bun} {bots.ROOT}/test/tools/claude_standin.ts "$@"\n')
    for n in ("codex", "grok", "muse", "opencode"):
        with open(os.path.join(agents, n), "w") as f: f.write("#!/bin/sh\nexit 1\n")
    for n in os.listdir(agents): os.chmod(os.path.join(agents, n), 0o755)
    seed_home(root, "bh")
    unified.seed(root, "desk", "git@github.com:o/r.git", False); unified.seed(root, "fw", "https://github.com/o/r", False)
    ports = {"bh": o.port, "desk": o.port + 1, "fw": o.port + 2}
    for k, n in NAMES.items(): os.environ["BP_NAME_" + k.upper()] = n
    procs, homes = {}, {}
    for k in ("bh", "desk", "fw"):
        procs[k], homes[k] = bots.start(o.hub, root, k, ports[k], ports["bh"] if k != "bh" else ports["desk"], agents)
    H = {k: Hub(f"http://127.0.0.1:{p}", "") for k, p in ports.items()}
    merged = o.expect != "split"
    try:
        if not all(h.wait_up(60) for h in H.values()):
            check("three hubs up", False); return finish()
        for k, h in H.items(): h.root = open(os.path.join(homes[k], "token")).read().strip()
        for k in ("desk", "fw"):
            inv = rpc(H["bh"], "bots.invite"); j = rpc(H[k], "bots.join", {"invite": inv["invite"]})
            check(f"boat-hub linked with {NAMES[k]}", bool(j.get("peer")), json.dumps(j)[:200]); time.sleep(1)
            for h in (H["bh"], H[k]):
                r = rpc(h, "bots.peer_own", {"peer": inv["peer"], "own": True}); check("marked the owner's own", r.get("ok") is True, json.dumps(r)[:200])
            rpc(H[k], "bots.create", {"name": "bot" + k})  # a change after the link pushes to boat-hub, whose gap pulls the history
        # both machines' threads reach boat-hub's mirrors and their origins are asked (once a minute)
        def both():
            ps = side(H["bh"]); ids = [r.get("id", "") for p in ps for r in p.get("threads", [])]
            if not (any(i.endswith("~t-desk") for i in ids) and any(i.endswith("~t-fw") for i in ids)): return None
            repo = [p for p in ps if (p.get("title") or "").startswith("repo-")]
            return ps if (len(repo) == 1 or not merged) else None
        ps = bots.until(both, 150) or side(H["bh"])
        repo = [p for p in ps if (p.get("title") or "").startswith("repo-")]
        print("boat-hub sidebar:", json.dumps(summary(ps)), flush=True)
        d = Data(H["bh"], H["bh"].root)
        pl = d.ok("projects.list", {}); prows = [p for p in (pl.get("projects") or []) if (p.get("title") or "").startswith("repo-")]
        print("boat-hub projects.list:", json.dumps([(p.get("title"), p.get("machine"), p.get("key"), [m.get("short") for m in p.get("machines") or []] if isinstance(p.get("machines"), list) else p.get("machines")) for p in pl.get("projects") or []]), flush=True)
        if not merged:
            report("before: the sidebar's rows for the repository", "seen", f"{len(repo)} rows: {[(p.get('title'), p.get('machine')) for p in repo]}")
            report("before: projects.list rows for the repository", "seen", f"{len(prows)} rows: {[(p.get('title'), p.get('machine')) for p in prows]}")
        else:
            check("the sidebar shows the repository once", len(repo) == 1, json.dumps(summary(ps)))
            if repo:
                r = repo[0]; ids = [t.get("id", "") for t in r.get("threads", [])]
                check("it holds both machines' threads", any(i.endswith("~t-desk") for i in ids) and any(i.endswith("~t-fw") for i in ids), str(ids))
                check("each thread names its machine", all((t.get("mark") or {}).get("short") in ("desktop", "framework") for t in r.get("threads", []) if "~t-" in t.get("id", "")), json.dumps(r.get("threads"))[:400])
                chips = [m.get("short") for m in r.get("machineChips") or []]
                check("its machine chips name both machines", sorted(chips) == ["desktop", "framework"], json.dumps(r.get("machineChips")))
                check("its key is the normalized origin", r.get("key") == "github.com/o/r", str(r.get("key")))
            check("projects.list has the repository once", len(prows) == 1, json.dumps(pl)[:600])
            if prows:
                p = prows[0]
                check("projects.list's row has key, machines and default", p.get("key") == "github.com/o/r" and sorted(m.get("short") for m in p.get("machines") or []) == ["desktop", "framework"] and bool(p.get("default")), json.dumps(p)[:500])
            notes = [p for p in ps if (p.get("title") or "").startswith("notes-")]
            check("projects with no remote stay per machine", len(notes) == 3, str([p.get("title") for p in notes]))
            # the registry: boat-hub (the home) holds every machine's rows
            reg = bots.until(lambda: (lambda r: r if any(x.get("key") == "github.com/o/r" and len(x.get("machines") or []) == 2 for x in r.get("projects", [])) else None)(rpc(H["bh"], "registry.list")), 90)
            check("the home's registry holds the repository on both machines", bool(reg), json.dumps(rpc(H["bh"], "registry.list"))[:500])
            if o.hold:
                os.makedirs(o.hold, exist_ok=True)
                with open(os.path.join(o.hold, "rig.json"), "w") as f: json.dump({"url": f"http://127.0.0.1:{ports['bh']}", "token_file": os.path.join(homes["bh"], "token"), "phase": "both on"}, f)
                print("holding (both on): touch", os.path.join(o.hold, "next"), flush=True)
                while not os.path.exists(os.path.join(o.hold, "next")) and not os.path.exists(os.path.join(o.hold, "stop")): time.sleep(1)
            # desktop goes off: its project and thread stay on boat-hub, dimmed
            procs["desk"].terminate(); procs["desk"].wait(15)
            def off():
                ps = side(H["bh"]); repo = [p for p in ps if (p.get("title") or "").startswith("repo-")]
                if len(repo) != 1: return None
                ch = {m.get("short"): m for m in repo[0].get("machineChips") or []}
                return (ps, repo[0]) if ch.get("desktop", {}).get("dim") else None
            got = bots.until(off, 150)
            check("with desktop off, the repository is still one row with desktop's chip dimmed", bool(got), json.dumps(summary(side(H["bh"]))))
            if got:
                r = got[1]; ch = {m.get("short"): m for m in r.get("machineChips") or []}
                check("desktop's chip says it is off or away", ch["desktop"].get("note") in ("desktop is off", "desktop is away"), json.dumps(ch["desktop"]))
                check("desktop's thread is still listed", any(t.get("id", "").endswith("t-desk") for t in r.get("threads", [])), json.dumps(r.get("threads"))[:400])
        if o.hold:
            os.makedirs(o.hold, exist_ok=True)
            with open(os.path.join(o.hold, "rig.json"), "w") as f: json.dump({"url": f"http://127.0.0.1:{ports['bh']}", "token_file": os.path.join(homes["bh"], "token"), "phase": "final"}, f)
            print("holding: touch", os.path.join(o.hold, "stop"), flush=True)
            end = time.time() + 3 * 3600
            while time.time() < end and not os.path.exists(os.path.join(o.hold, "stop")): time.sleep(1)
    finally:
        for p in procs.values(): p.terminate()
        for p in procs.values():
            try: p.wait(10)
            except subprocess.TimeoutExpired: p.kill()
        if os.environ.get("KEEP") != "1": shutil.rmtree(root, ignore_errors=True)
        else: print("kept", root)
    return finish()


if __name__ == "__main__":
    sys.exit(main())
