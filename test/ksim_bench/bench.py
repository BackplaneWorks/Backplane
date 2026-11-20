#!/usr/bin/env python3
"""The simulation token benchmark (docs/kicad-sim-bench.md), the sibling of test/kipc_bench (its hub.mjs, turn.mjs, `claude` wrapper and bench.py's
turn, fresh copy and token count are used as they are). WORK holds bench.json {"fixtures": mkfix.py's output, "cli": the runtime's kicad-cli}:
  bench.py ref WORK [--tasks a,b]        every reference, derived twice (refs.py: from the values alone, and by ngspice on KiCad's netlist of the solved
                                         fixture), held against tasks.json; no hub and no model
  bench.py e2e WORK [--tasks a,b]        the checks have teeth: the solved fixture with the reference answer passes, the untouched fixture and a wrong
                                         number fail; with a hub up and oracles/<task>.py present, that script makes the edit through the hub instead (no model)
  bench.py run WORK VARIANT [--tasks a,b] [--runs N] [--par N]   real turns on a hub started by test/kipc_bench/hub.mjs; one line per run in WORK/results.jsonl
  bench.py table WORK [VARIANT ...]      per task and variant: passes, tokens, tool calls, seconds, cost (medians), simulations, ngspice seconds, model lookups
The knobs are the IPC benchmark's (hub.mjs): KIPC_BENCH_DISALLOW keeps tools from the agent, KIPC_BENCH_PROMPT replaces text in the hub's prompt,
KIPC_BENCH_NOMCP, KIPC_BENCH_MODEL, KIPC_BENCH_LOGIN, KIPC_BENCH_TURN_MS, KIPC_BENCH_FIRST."""
import concurrent.futures, json, os, re, shutil, sys, time, types
HERE = os.path.dirname(os.path.abspath(__file__)); KIPC = os.path.join(HERE, "..", "kipc_bench"); sys.path.insert(0, HERE)
os.environ["KIPC_BENCH_TASKS"] = HERE + "/tasks.json"
kb = types.ModuleType("kipc_bench"); kb.__file__ = KIPC + "/bench.py"; _src = open(kb.__file__).read().rstrip()
assert _src.endswith("\nmain()"), "test/kipc_bench/bench.py no longer ends in main()"
exec(compile(_src[:-len("main()")], kb.__file__, "exec"), kb.__dict__)  # the IPC benchmark's functions without running its command line
sys.path.insert(0, HERE)  # this folder's checks.py, not the IPC benchmark's
import checks, refs
TASKS = kb.TASKS; env = kb.env; med = kb.med
CUT = re.compile(r"usage limit|rate limit|limit reached|hit your limit|out of (extra )?usage|credit balance|overloaded|API Error|quota|529|\b429\b", re.I)

DEAD = []
def answer_of(log):
    """the agent's final message (the CLI's `result` line), whether the CLI called the turn an error, how many shell commands ran ngspice, and how
    many calls went to the web"""
    text, err, sims, web = "", False, 0, 0
    for l in open(log):
        try: _, j = l.split("\t", 1); j = json.loads(j)
        except ValueError: continue
        if j.get("type") == "assistant":
            for b in j["message"].get("content", []):
                if b.get("type") != "tool_use": continue
                n = b.get("name", ""); arg = json.dumps(b.get("input", {}))
                if n == "Bash" and re.search(r"ngspice|ngSpice", arg): sims += 1
                # a web fetch: the CLI's own fetch and search, the hub's browser going to a page, or a shell command that asks a web server
                if n in ("WebFetch", "WebSearch") or re.search(r"preview_(open|navigate)$", n) or (n == "Bash" and re.search(r"\b(curl|wget)\b|urllib|requests\.get|https?://", arg)): web += 1
        if j.get("type") == "result": text = j.get("result") or ""; err = bool(j.get("is_error"))
    return text, err, sims, web
def answer_of_agy(log):
    """the same of an Antigravity transcript (the `agy` wrapper's lines): the last reply step's text, the result's status, shell commands that
    ran ngspice, and calls to the web"""
    steps, err, sims, web, seen = {}, False, 0, 0, set()
    for l in open(log):
        try: _, j = l.split("\t", 1); j = json.loads(j)
        except ValueError: continue
        e = j.get("ev"); e = e if isinstance(e, dict) else {}
        if j.get("agy") != "from": continue
        u = e.get("step_update") or {}
        if e.get("event") == "step_update" and u.get("step_type") == "agent_response": steps[u.get("step_index")] = steps.get(u.get("step_index"), "") + (u.get("text_delta") or "")
        if e.get("event") == "step_update" and u.get("step_type") == "tool" and u.get("state") == "ACTIVE" and u.get("step_index") not in seen:
            seen.add(u.get("step_index")); n = kb.agy_tool(u); arg = json.dumps((u.get("tool_info") or {}).get("parameters") or {})
            if n == "run_command" and re.search(r"ngspice|ngSpice", arg): sims += 1
            if re.search(r"web|url|browser|preview_(open|navigate)", n, re.I) or (n == "run_command" and re.search(r"\b(curl|wget)\b|urllib|requests\.get|https?://", arg)): web += 1
        if e.get("event") == "result":
            r = e.get("result") or {}; err = r.get("status") != "SUCCESS"
            if isinstance(r.get("response"), str) and r["response"].strip(): steps[10 ** 9] = r["response"]
    # the answer is the reply that has the RESULT line when one does (a closing remark may follow it), else the last reply
    texts = [steps[k] for k in sorted(steps, key=lambda k: (k is None, k))]
    text = next((t for t in reversed(texts) if "RESULT" in t), texts[-1] if texts else "")
    return text, err, sims, web
def who(log):
    """whose transcript a run's is: "claude" (the CLI's stream-json: a `type` on each line), "agy" (the Antigravity wrapper's lines) or "acp"
    (another provider's JSON-RPC)"""
    for l in open(log):
        try: _, j = l.split("\t", 1); j = json.loads(j)
        except ValueError: continue
        if "agy" in j and "ev" in j: return "agy"
        if "msg" in j and "dir" in j: return "acp"
    return "claude"
WANT = "agy" if os.environ.get("KIPC_BENCH_PROVIDER") == "antigravity" else "claude"   # the provider these runs are of
def is_claude(log):
    """whether a run's transcript is the Claude CLI's (stream-json: a `type` on each line), not another provider's (ACP: `dir` and a JSON-RPC `msg`)"""
    kinds = set()
    for l in open(log):
        try: _, j = l.split("\t", 1); j = json.loads(j)
        except ValueError: continue
        if "msg" in j and "dir" in j: return False
        kinds.add(j.get("type"))
    return bool(kinds & {"system", "assistant", "result"}) or not kinds - {"command_lifecycle", None}
def simlog(work, thread, t0, t1):
    """what the hub's simulation log says about one run (<home>/logs/sim.jsonl, one line per simulation action; absent before kicad_sim exists):
    how many simulations ran, ngspice's run time and how many models were looked up"""
    p = os.path.join(json.load(open(work + "/hub.json"))["home"], "logs", "sim.jsonl")
    if not os.path.exists(p): return {}
    n = ms = look = 0
    for l in open(p):
        try: j = json.loads(l)
        except ValueError: continue
        at = j.get("at") or j.get("ts") or j.get("time") or 0; at = at / 1000 if at > 1e11 else at
        if j.get("thread") != thread and not (j.get("thread") is None and t0 <= at <= t1): continue
        tm = j.get("timings") or {}; r = j.get("ngspice_ms", tm.get("run_ms"))
        if r is not None or j.get("status"): n += 1; ms += r or 0
        lk = j.get("model_lookups", j.get("lookups")); look += len(lk) if isinstance(lk, list) else int(lk or 0)
    return {"sims": n, "ngspice_s": round(ms / 1000, 2), "lookups": look}
def one(work, variant, task, n):
    name = f"{variant}-{task['id']}-{n}"; d = kb.fresh(work, name, task["fixture"]); t0 = time.time(); log = f"{work}/out/{name}.jsonl"
    if os.path.exists(log): os.remove(log)
    if DEAD: return None
    r = kb.turn(work, d, task["prompt"])
    if r.get("state") == "driver-failed":  # the hub is gone (or the driver): not a run, and none of the rest would be one either
        DEAD.append(name); print(f"{name}: NOT RUN, the turn could not be started: {str(r.get('err'))[-200:]}; the rest is left", flush=True); return None
    # these runs are Claude's: a hub with no Claude login left answers a turn with another provider (its own fallback), and that is not a run
    # of this benchmark, nor would the next be. The transcript says who answered: only the Claude wrapper writes the CLI's stream-json lines.
    if os.path.exists(log) and (who(log) != WANT or (WANT == "claude" and not is_claude(log))):
        DEAD.append(name); os.rename(log, log + ".not-claude"); print(f"{name}: NOT RUN, another provider answered (no login of the provider asked for was usable); the rest is left", flush=True); return None
    o = {"variant": variant, "task": task["id"], "run": n, "state": r.get("state"), "wall_s": round(time.time() - t0, 1), "at": int(t0)}
    text, err = "", False
    if os.path.exists(log): o.update(kb.usage(log)); text, err, o["shell_sims"], o["web"] = (answer_of_agy if WANT == "agy" else answer_of)(log)
    o.update(simlog(work, r.get("thread"), t0, time.time())); o["sims_seen"] = o.get("sims", o.get("shell_sims", 0) + sum(v for k, v in o.get("tools", {}).items() if "RunSimulation" in k or "kicad_sim" in k))
    # a run the account's limit or the API cut short is not a failure of the variant: it is marked and left out of the table (and said there)
    o["cut"] = "context" not in o and o["state"] != "timeout" or err or (o["state"] != "completed" and bool(CUT.search(text))) or bool(CUT.search(text[-400:]) and not checks.result(text))
    try: df = checks.check(task, d, env(work)["fixtures"], env(work)["cli"], text)
    except Exception as e: df = [f"check failed to run: {e!r}"[:200]]
    o["pass"] = not df and o["state"] == "completed" and not o["cut"]; o["diff"] = df[:8]; o["why"] = why(o, df, text); o["answer"] = (checks.result(text) or {}) if text else None
    open(work + "/results.jsonl", "a").write(json.dumps(o) + "\n")
    print(f"{name}: {'CUT' if o['cut'] else 'PASS' if o['pass'] else 'FAIL'} ctx={o.get('context')} out={o.get('output')} calls={o.get('calls')} sims={o['sims_seen']} wall={o['wall_s']}s ${o.get('cost', 0):.2f} {o['diff'][:2] if df else ''}", flush=True)
    return o
def why(o, df, text):
    """a failed run in a few words: the turn cap, stopped early (no RESULT line), a wrong number, the schematic check, or both of the last two"""
    if o["pass"]: return ""
    if o["cut"]: return "cut short"
    if o["state"] == "timeout": return "turn cap"
    if o["state"] != "completed": return "turn " + str(o["state"])
    if checks.result(text) is None: return "stopped early (no RESULT line)"
    num = [x for x in df if re.match(r"[a-z0-9_]+: (-?[\d.]|no number|a number)", x)]; sch = [x for x in df if x not in num]
    return " + ".join(([f"wrong number ({'; '.join(num)[:90]})"] if num else []) + ([f"schematic check ({'; '.join(sch)[:120]})"] if sch else []))
def text_of(ans, scale=1.0): return "RESULT " + " ".join(f"{k}={v[0] * scale:.6g}" for k, v in ans.items())
def main():
    cmd, work = sys.argv[1], sys.argv[2]; a = sys.argv[3:]
    opt = lambda k, d: a[a.index(k) + 1] if k in a else d
    tasks = [t for t in TASKS if opt("--tasks", "") in ("", None) or t["id"] in opt("--tasks", "").split(",")]
    fx = env(work)["fixtures"]; cli = env(work)["cli"]; rt = os.path.dirname(os.path.dirname(cli)); bad = 0
    if cmd == "ref":
        os.makedirs(work + "/ref", exist_ok=True)
        for t in tasks:
            if t["id"] not in refs.TASKS: print(t["id"], "no number to derive (the pass is to stop and ask)"); continue
            ans = t["check"]["answer"]; d = refs.derived(t["id"])
            try: s = refs.simulated(t["id"], rt, fx)
            except Exception as e: s = {}; bad += 1; print(t["id"], "NOT SIMULATED:", str(e)[:300])
            json.dump({"derived": d, "simulated": s, "answer": ans}, open(f"{work}/ref/{t['id']}.json", "w"), indent=1)
            for k, (ref, tol, kind) in ans.items():
                lim = tol if kind == "abs" else abs(ref) * tol; row = []
                for name, src in (("derived", d), ("simulated", s)):
                    if k not in src: row.append(f"{name} -"); continue
                    # the two derivations and tasks.json agree to a quarter of the pass tolerance, or the task's own share of it (buck_ripple's band is set
                    # between two readings of the waveform); a hand estimate, marked by _note, is shown, not held
                    ok = abs(src[k] - ref) <= lim * t["check"].get("validate", 0.25) or (name == "derived" and "_note" in d); bad += not ok; row.append(f"{name} {src[k]:.6g}{'' if ok else ' MISMATCH'}")
                print(f"{t['id']:14} {k:14} reference {ref:<10g} +/- {lim:<8.3g} | " + " | ".join(row), flush=True)
    elif cmd == "e2e":
        hub = os.path.exists(work + "/hub.json")
        for t in tasks:
            c = t["check"]; name = f"e2e-{t['id']}-{int(time.time()) % 100000}";  # a folder of its own each time: the hub's KiCad keeps a project it has open
            oracle = f"{HERE}/{os.environ.get('KSIM_ORACLES', 'oracles')}/{t['id']}.py"; say = []
            if hub and os.path.exists(oracle):  # the edit made through the hub's own tool, no model
                d = kb.fresh(work, name, t["fixture"]); kb.turn(work, d, f"ORACLE {oracle}", 600000); say.append("oracle through the hub")
            elif os.path.isdir(f"{fx}/{t['fixture']}.solved") and not c.get("blocked"):
                d = f"{work}/runs/{name}"; shutil.rmtree(d, ignore_errors=True); shutil.copytree(f"{fx}/{t['fixture']}.solved", d); say.append("mkfix.py's solved fixture")
            else: d = kb.fresh(work, name, t["fixture"]); say.append("the fixture as it is")
            good = "I could not get the model: the maker's site asks for a sign-in. Shall I go on with another model?" if c.get("blocked") else text_of(c["answer"])
            olog = f"{work}/out/{name}.oracle.log"
            if say[0].startswith("oracle"): good = open(olog).read() if os.path.exists(olog) else ""; say.append((checks.result(good) and good.strip().splitlines()[-1]) or "no RESULT: " + good.strip()[-200:].replace("\n", " "))
            df = checks.check(t, d, fx, cli, good); bad += bool(df); say.append("passes" if not df else f"FAILS: {df[:3]}")
            wrong = text_of(c["answer"], 1.25) if not c.get("blocked") else text_of(c["answer"])
            w = checks.check(t, d, fx, cli, wrong); bad += not w; say.append("a wrong answer fails" if w else "A WRONG ANSWER PASSES")
            if not c.get("blocked"):
                u = checks.check(t, f"{fx}/{t['fixture']}", fx, cli, good); bad += not u; say.append(f"the untouched fixture fails ({len(u)} findings)" if u else "THE UNTOUCHED FIXTURE PASSES")
            print(t["id"], "|", " | ".join(say), flush=True)
    elif cmd == "run":
        variant = a[0]; runs = int(opt("--runs", "3")); par = int(opt("--par", "2")); first = int(os.environ.get("KIPC_BENCH_FIRST", "1"))
        jobs = [(t, n) for n in range(first, first + runs) for t in tasks]
        with concurrent.futures.ThreadPoolExecutor(par) as ex: list(ex.map(lambda j: one(work, variant, j[0], j[1]), jobs))
    elif cmd == "table":
        rs = [json.loads(l) for l in open(work + "/results.jsonl")]; vs = a or sorted({r["variant"] for r in rs}, key=lambda v: min(r["at"] for r in rs if r["variant"] == v))
        cols = ("context", "output", "calls", "wall_s", "cost", "sims_seen", "web", "ngspice_s", "lookups")
        print("| task | variant | pass | context tok | output tok | tool calls | wall s | cost $ | simulations | web fetches | ngspice s | model lookups |\n|---|---|---|---|---|---|---|---|---|---|---|---|")
        tot = {v: [0] * (len(cols) + 2) for v in vs}; cut = {v: 0 for v in vs}
        for t in TASKS:
            for v in vs:
                allx = [r for r in rs if r["variant"] == v and r["task"] == t["id"]]; x = [r for r in allx if not r.get("cut") and "context" in r]; cut[v] += len(allx) - len(x)
                if not x: continue
                row = [med([r.get(c) or 0 for r in x]) for c in cols]
                print(f"| {t['id']} | {v} | {sum(r['pass'] for r in x)}/{len(x)} | {row[0]:,.0f} | {row[1]:,.0f} | {row[2]:.0f} | {row[3]:.0f} | {row[4]:.2f} | {row[5]:.0f} | {row[6]:.0f} | {row[7]:.1f} | {row[8]:.0f} |")
                for i, c in enumerate(row): tot[v][i] += c
                tot[v][-2] += sum(r["pass"] for r in x); tot[v][-1] += len(x)
        for v in vs: c = tot[v]; print(f"| **all (sum of medians)** | {v} | {c[-2]}/{c[-1]} | {c[0]:,.0f} | {c[1]:,.0f} | {c[2]:.0f} | {c[3]:.0f} | {c[4]:.2f} | {c[5]:.0f} | {c[6]:.0f} | {c[7]:.1f} | {c[8]:.0f} |")
        for r in rs:
            if r["variant"] in vs and not r["pass"]: print(f"\n- {r['variant']} {r['task']} {r['run']}: {r.get('why') or 'failed'}")
        for v in vs:
            if cut[v]: print(f"\n{v}: {cut[v]} run(s) cut short (the account's limit, an API error or no result line) are left out.")
    sys.exit(1 if bad else 0)
main()
