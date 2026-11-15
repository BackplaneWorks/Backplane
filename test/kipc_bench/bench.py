#!/usr/bin/env python3
"""The KiCad-IPC token benchmark (docs/kicad-ipc-bench.md), on a hub started by hub.mjs with WORK:
  bench.py ref WORK                              the reference result of every task (its oracle, no model), kept in WORK/ref
  bench.py e2e WORK                              every task as one kicad_edit call (edit_e2e.py, no model) must give the reference result
  bench.py run WORK VARIANT [--tasks a,b] [--runs N] [--par N]   real turns; one line per run in WORK/results.jsonl
  bench.py table WORK [VARIANT ...]              per task and variant: passes, tokens, tool calls, seconds (medians)"""
import concurrent.futures, json, os, shutil, statistics, subprocess, sys, time
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
import norm
TASKS = json.load(open(HERE + "/tasks.json"))
def env(work): return json.load(open(work + "/bench.json"))
def turn(work, d, text, ms=int(os.environ.get("KIPC_BENCH_TURN_MS", "600000"))):  # a turn that hangs is a failed run, never a stalled benchmark
    r = subprocess.run(["bun", HERE + "/turn.mjs", work, d, text], capture_output=True, text=True, env={**os.environ, "TURN_MS": str(ms)})
    try: return json.loads(r.stdout.strip().splitlines()[-1])
    except Exception: return {"state": "driver-failed", "err": (r.stdout + r.stderr)[-300:]}
def fresh(work, name, fixture):
    d = f"{work}/runs/{name}"; shutil.rmtree(d, ignore_errors=True); shutil.copytree(f"{env(work)['fixtures']}/{fixture}", d); return d
def usage(log):
    """Tokens, tool calls and time from the CLI's stream: its `result` line's totals, and each tool call by name."""
    o = {"tools": {}, "calls": 0, "tool_result_chars": 0}; seen = set()
    for l in open(log):
        try: t, j = l.split("\t", 1); j = json.loads(j)
        except ValueError: continue
        if j.get("type") == "assistant":
            for b in j["message"].get("content", []):
                if b.get("type") == "tool_use" and b["id"] not in seen:
                    seen.add(b["id"]); n = b["name"]
                    if n == "mcp__backplane__kicad_ipc": n = "kicad_ipc:" + str(b["input"].get("type", "")).rsplit(".", 1)[-1]
                    o["tools"][n] = o["tools"].get(n, 0) + 1; o["calls"] += 1
        if j.get("type") == "user":
            c = j["message"].get("content", [])
            for b in c if isinstance(c, list) else []:
                if b.get("type") == "tool_result": o["tool_result_chars"] += len(json.dumps(b.get("content", "")))
        if j.get("type") == "result":
            u = j.get("usage", {}); o.update(input=u.get("input_tokens", 0), cache_write=u.get("cache_creation_input_tokens", 0), cache_read=u.get("cache_read_input_tokens", 0), output=u.get("output_tokens", 0),
                cost=j.get("total_cost_usd", 0), api_turns=j.get("num_turns", 0), cli_ms=j.get("duration_ms", 0), api_ms=j.get("duration_api_ms", 0), is_error=j.get("is_error", False), models=sorted(j.get("modelUsage", {}).keys()))
            o["context"] = o["input"] + o["cache_write"] + o["cache_read"]
    return o
def one(work, variant, task, n):
    name = f"{variant}-{task['id']}-{n}"; d = fresh(work, name, task["fixture"]); t0 = time.time()
    log = f"{work}/out/{name}.jsonl"
    if os.path.exists(log): os.remove(log)
    r = turn(work, d, task["prompt"])
    o = {"variant": variant, "task": task["id"], "run": n, "state": r.get("state"), "wall_s": round(time.time() - t0, 1), "at": int(t0)}
    if os.path.exists(log): o.update(usage(log))
    df = norm.diff(json.load(open(f"{work}/ref/{task['id']}.json")), norm.norm(d, env(work)["cli"]))
    o["pass"] = not df and r.get("state") == "completed"; o["diff"] = df[:6]
    open(work + "/results.jsonl", "a").write(json.dumps(o) + "\n")
    print(f"{name}: {'PASS' if o['pass'] else 'FAIL'} ctx={o.get('context')} out={o.get('output')} calls={o.get('calls')} wall={o['wall_s']}s ${o.get('cost', 0):.2f} {o['diff'][:2] if df else ''}", flush=True)
    return o
def med(xs): return statistics.median(xs) if xs else 0
def main():
    cmd, work = sys.argv[1], sys.argv[2]; a = sys.argv[3:]
    opt = lambda k, d: a[a.index(k) + 1] if k in a else d
    tasks = [t for t in TASKS if opt("--tasks", "") in ("", None) or t["id"] in opt("--tasks", "").split(",")]
    if cmd == "ref":
        os.makedirs(work + "/ref", exist_ok=True)
        for t in tasks:
            d = fresh(work, "ref-" + t["id"], t["fixture"]); r = turn(work, d, f"ORACLE {HERE}/oracles/{t.get('oracle', t['id'])}.py", 120000)
            n = norm.norm(d, env(work)["cli"]); json.dump(n, open(f"{work}/ref/{t['id']}.json", "w"))
            base = norm.diff(norm.norm(f"{env(work)['fixtures']}/{t['fixture']}", env(work)["cli"]), n)
            print(t["id"], r.get("state"), open(f"{work}/out/ref-{t['id']}.oracle.log").read().strip().splitlines()[-1:], "| vs fixture:", len(base), "differences", base[:3], flush=True)
    elif cmd == "e2e":  # kicad_edit against a real KiCad: each task as one call must give the reference result
        bad = 0
        for t in tasks:
            d = fresh(work, "e2e-" + t["id"], t["fixture"]); r = turn(work, d, f"ORACLE {HERE}/edit_e2e.py", 120000)
            df = norm.diff(json.load(open(f"{work}/ref/{t['id']}.json")), norm.norm(d, env(work)["cli"]))
            if "misspelt field refused" not in open(f"{work}/out/e2e-{t['id']}.oracle.log").read(): df.append("kicad_ipc took a misspelt field")
            bad += bool(df)
            print(t["id"], "same as the reference" if not df else df[:4], "|", open(f"{work}/out/e2e-{t['id']}.oracle.log").read().strip()[-400:].replace("\n", " | "), flush=True)
        sys.exit(1 if bad else 0)
    elif cmd == "run":
        variant = a[0]; runs = int(opt("--runs", "3")); par = int(opt("--par", "2"))
        jobs = [(t, n) for n in range(1, runs + 1) for t in tasks]
        with concurrent.futures.ThreadPoolExecutor(par) as ex: list(ex.map(lambda j: one(work, variant, j[0], j[1]), jobs))
    elif cmd == "table":
        rs = [json.loads(l) for l in open(work + "/results.jsonl")]; vs = a or sorted({r["variant"] for r in rs}, key=lambda v: min(r["at"] for r in rs if r["variant"] == v))
        print("| task | variant | pass | context tok | output tok | tool calls | wall s | cost $ |\n|---|---|---|---|---|---|---|---|")
        tot = {v: [0, 0, 0, 0, 0, 0, 0] for v in vs}
        for t in TASKS:
            for v in vs:
                x = [r for r in rs if r["variant"] == v and r["task"] == t["id"] and "context" in r]
                if not x: continue
                row = [med([r["context"] for r in x]), med([r["output"] for r in x]), med([r["calls"] for r in x]), med([r["wall_s"] for r in x]), med([r["cost"] for r in x])]
                print(f"| {t['id']} | {v} | {sum(r['pass'] for r in x)}/{len(x)} | {row[0]:,.0f} | {row[1]:,.0f} | {row[2]:.0f} | {row[3]:.0f} | {row[4]:.2f} |")
                for i, c in enumerate(row): tot[v][i] += c
                tot[v][5] += sum(r["pass"] for r in x); tot[v][6] += len(x)
        for v in vs: c = tot[v]; print(f"| **all (sum of medians)** | {v} | {c[5]}/{c[6]} | {c[0]:,.0f} | {c[1]:,.0f} | {c[2]:.0f} | {c[3]:.0f} | {c[4]:.2f} |")
main()
