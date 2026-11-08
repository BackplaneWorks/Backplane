#!/usr/bin/env python3
"""One repository on two linked hubs is one sidebar project. Two temp-home hubs (bots.py's rig: stand-in agents, --no-tailscale), each with a
project folder that is a git repository with the same origin in another form (A `git@github.com:o/r.git`, B `https://github.com/o/r`) and a
thread; linked and marked the owner's own. Then on A:
  - machine_list (a bot's mcpcall) gives B's project with `origin` github.com/o/r;
  - the native sidebar (view.open) shows ONE project for the repository, holding A's thread (no marker) and B's thread (`mark` naming B);
  - a project of another repository on B keeps its own row, labelled by machine;
  - the place words: the merged project says `also` "also on <B>", B's own project `mark.label` "on <B>", B's thread `mark.runs` "runs on <B>".

  unified.py --hub PATH/backplane-hub [--port-a 3831] [--port-b 3832] [--hold DIR]
  --hold DIR: after the checks, write DIR/rig.json ({url, token_file} of A) and keep both hubs up until DIR/stop exists (a window to look at).
"""
import argparse, json, os, shutil, subprocess, sys, tempfile, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from proto import Hub, check, report, finish  # noqa: E402
from settings_revoke import View  # noqa: E402
from link import rpc  # noqa: E402
import bots  # noqa: E402


def seed(root, name, origin, other):
    """The home's first events: the shared repository's project and a thread, and (B only) a project of another repository."""
    home, proj = os.path.join(root, name, "home"), os.path.join(root, name, "proj")
    os.makedirs(home, exist_ok=True); os.makedirs(proj, exist_ok=True)
    subprocess.run(["git", "init", "-q"], cwd=proj, check=True)
    subprocess.run(["git", "remote", "add", "origin", origin], cwd=proj, check=True)
    # (set up and not resuming: a window opened on the held rig shows the sidebar, not the first-run tour)
    ev = [{"$": "SettingSet", "key": k, "value": v, "at": int(time.time()) - 90} for k, v in (("onboard.done", "1"), ("onboard.uber", "1"), ("restart.continue", "off"))]
    ev += [{"$": "ProjectCreated", "id": "p1", "title": "repo-" + name, "root": proj, "at": int(time.time()) - 60},
          {"$": "ThreadCreated", "id": "t-" + name, "project": "p1", "title": "thread on " + name, "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": int(time.time()) - 30}]
    if other:
        o = os.path.join(root, name, "other"); os.makedirs(o, exist_ok=True)
        subprocess.run(["git", "init", "-q"], cwd=o, check=True); subprocess.run(["git", "remote", "add", "origin", "git@github.com:o/elsewhere.git"], cwd=o, check=True)
        ev += [{"$": "ProjectCreated", "id": "p2", "title": "other-" + name, "root": o, "at": int(time.time()) - 50},
               {"$": "ThreadCreated", "id": "u-" + name, "project": "p2", "title": "other thread", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": int(time.time()) - 20}]
    with open(os.path.join(home, "events.jsonl"), "w") as f:
        for e in ev: f.write(json.dumps(e) + "\n")


def sidebar(hub):
    v = View(hub)
    try:
        r = v.request("view.open", {"now": int(time.time() * 1000)})
    finally:
        v.close()
    res = r.get("result") or r
    shown = (res.get("shown") if isinstance(res, dict) else None) or (res.get("view", {}) or {}).get("shown") or {}
    return shown.get("projects") or [], r


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--hub", required=True)
    a.add_argument("--port-a", type=int, default=3831)
    a.add_argument("--port-b", type=int, default=3832)
    a.add_argument("--hold", default="")
    o = a.parse_args()
    bun = shutil.which("bun")
    if not bun:
        report("unified projects", "not run", "bun (the stand-in agent's runtime) is not installed")
        return finish()
    base = os.environ.get("BP_TMP", os.path.expanduser("~/.cache/bp-tmp")); os.makedirs(base, exist_ok=True)
    root = tempfile.mkdtemp(prefix="bpt-unified-", dir=base)
    agents = os.path.join(root, "bin"); os.makedirs(agents)
    with open(os.path.join(agents, "claude"), "w") as f: f.write(f'#!/bin/sh\nexec {bun} {bots.ROOT}/test/tools/claude_standin.ts "$@"\n')
    for n in ("codex", "grok", "muse", "opencode"):
        with open(os.path.join(agents, n), "w") as f: f.write("#!/bin/sh\nexit 1\n")
    for n in os.listdir(agents): os.chmod(os.path.join(agents, n), 0o755)
    seed(root, "a", "git@github.com:o/r.git", False); seed(root, "b", "https://github.com/o/r", True)
    pa, ha = bots.start(o.hub, root, "a", o.port_a, o.port_b, agents)
    A, B = (Hub(f"http://127.0.0.1:{p}", "") for p in (o.port_a, o.port_b))
    A.wait_up(60)
    pb, hb = bots.start(o.hub, root, "b", o.port_b, o.port_a, agents)
    try:
        if not (A.wait_up(60) and B.wait_up(60)):
            check("both hubs up", False); return finish()
        pa.terminate(); pa.wait(15)
        pa, ha = bots.start(o.hub, root, "a", o.port_a, o.port_b, agents)
        if not A.wait_up(60):
            check("A back up", False); return finish()
        time.sleep(3)
        A.root = open(os.path.join(ha, "token")).read().strip(); B.root = open(os.path.join(hb, "token")).read().strip()
        inv = rpc(A, "bots.invite"); j = rpc(B, "bots.join", {"invite": inv["invite"]}); link = inv["peer"]
        check("linked", bool(j.get("peer")), json.dumps(j)[:200])
        time.sleep(1)
        for hub in (A, B):
            r = rpc(hub, "bots.peer_own", {"peer": link, "own": True}); check("link marked as the owner's own", r.get("ok") is True, json.dumps(r)[:200])
        # machine_list from a bot on A: B's project carries its origin
        ada = rpc(A, "bots.create", {"name": "ada"})["bot"]
        tha = next(e["thread"] for e in bots.log_of(ha) if e.get("$") == "BotSet" and e.get("id") == ada)
        bots.until(lambda: any(e.get("$") == "WorktreeSet" and e.get("id") == tha for e in bots.log_of(ha)))
        rpc(A, "bots.say", {"bot": ada, "text": "mcpcall:" + json.dumps({"name": "machine_list", "arguments": {}})})
        ml = bots.until(lambda: next((e for e in bots.log_of(ha) if e.get("$") == "MessagePosted" and e.get("thread") == tha and "MCPCALL machine_list" in e.get("text", "")), None))
        t = (ml or {}).get("text", "").replace("\\", "")
        far_part = t.split('"link"', 1)[-1] if '"link"' in t else ""  # (the posted text is clipped: B's first project is enough)
        check("machine_list gives the linked machine's projects with their origin", '"origin":"github.com/o/r"' in far_part.replace(" ", ""), t[:400])
        # a change on B after the link pushes to A, whose gap pulls B's history (else the five-minute pull would)
        rpc(B, "bots.create", {"name": "bob"})
        # the sidebar: one project for the repository (the origins are asked for once a minute; B's threads arrive by the mirror)
        def unified():
            ps, _ = sidebar(A)
            repo = [p for p in ps if p.get("title") == "repo-a"]
            if not repo or not any(r.get("id", "").endswith("~t-b") for r in repo[0].get("threads", [])): return None
            return ps
        ps = bots.until(unified, 150)
        if not check("A's sidebar shows the repository once, with B's thread in it", bool(ps), json.dumps([(p.get("title"), p.get("machine"), [r.get("id") for r in p.get("threads", [])]) for p in sidebar(A)[0]])):
            return finish()
        titles = [p.get("title") for p in ps]
        check("B's project of the same repository has no row of its own", "repo-b" not in titles, str(titles))
        repo = next(p for p in ps if p.get("title") == "repo-a")
        rows = {r["id"].split("|")[-1]: r for r in repo["threads"]}
        mine = rows.get("t-a", {}); far = next(r for k, r in rows.items() if k.endswith("~t-b"))
        check("A's own thread has no machine marker", "mark" not in mine and "machine" not in mine, json.dumps(mine)[:200])
        check("B's thread names its machine", isinstance(far.get("mark"), dict) and far["mark"].get("name") and far["mark"].get("away") is False, json.dumps(far)[:300])
        other = next((p for p in ps if p.get("title") == "other-b"), None)
        check("a linked project of another repository keeps its own row, labelled", bool(other) and other.get("machine") == far["mark"]["name"], str(titles))
        short = far["mark"].get("short", "")
        check("the merged project says where else it is", repo.get("also") == f"also on {short}" and "mark" not in repo, json.dumps({k: repo.get(k) for k in ("also", "marks", "mark")})[:300])
        check("the other machine's project says where it is", bool(other) and (other.get("mark") or {}).get("label") == f"on {short}", json.dumps((other or {}).get("mark"))[:200])
        check("the other machine's thread says where it runs", far["mark"].get("runs") == f"runs on {short}", json.dumps(far["mark"])[:200])
        if o.hold:
            os.makedirs(o.hold, exist_ok=True)
            with open(os.path.join(o.hold, "rig.json"), "w") as f: json.dump({"url": f"ws://127.0.0.1:{o.port_a}", "token_file": os.path.join(ha, "token"), "far_thread": far["id"]}, f)
            print("holding: touch", os.path.join(o.hold, "stop"), flush=True)
            end = time.time() + 3 * 3600
            while time.time() < end and not os.path.exists(os.path.join(o.hold, "stop")): time.sleep(1)
    finally:
        for p in (pa, pb): p.terminate()
        for p in (pa, pb):
            try: p.wait(10)
            except subprocess.TimeoutExpired: p.kill()
        if os.environ.get("KEEP") != "1": shutil.rmtree(root, ignore_errors=True)
        else: print("kept", root)
    return finish()


if __name__ == "__main__":
    sys.exit(main())
