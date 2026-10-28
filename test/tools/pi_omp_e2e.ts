// Pi and Oh My Pi end to end through a real headless hub (Grok's runner,
// MS.PPi / MS.POmp, the hub's asks), with no real model and no account.
//
// Pi: the REAL `pi` binary (REAL_PI, else `pi` on PATH; the section is
// skipped when there is none). It runs with an isolated PI_CODING_AGENT_DIR
// and offline, against a stand-in OpenAI-compatible model server started
// here. It loads Backplane's extension as the hub writes it.
//   models     `pi --list-models` reaches every client (about.pi.cli, the
//              efforts of a model that does not think)
//   turn       a supervised turn completes, the reply reaches the thread,
//              the extension is written, Backplane's MCP tools reach the model
//   approval   a bash call opens an approval ask with its command; accept
//              runs it; decline blocks it and the model hears so
//   resume     the next turn sends the earlier conversation (one session)
//   lost       a session Pi no longer has is said once
//   stop       turn.interrupt ends a running turn
// Oh My Pi: a stand-in `omp` (test/tools/omp_standin.ts) shaped as omp
// 18.12.6's RPC mode, loading Backplane's real extension (its tools over
// the hub's MCP); no model runs.
//   models     `omp models --json`, its labels and thinking levels
//   turn       --mode rpc-ui, --approval-mode always-ask, --append-system-prompt,
//              the model and thinking as flags, the thread's session folder,
//              the RPC exchange, the prompt is the text alone, Backplane's
//              tools registered by the extension, the turn's usage and cost
//   resume     the session goes on (--continue); a lost one starts afresh, said
//   asks       a bash approval (select, Approve), a write approval declined
//              (Deny), a question, Backplane's own tool through the extension
//
//   bun test/tools/pi_omp_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { cbor, plain } from "../../src/web/wire.js";
import { encode } from "./ios_test_cbor.ts";
const [bin = "build/backplane"] = process.argv.slice(2);
const codecSource=readFileSync(resolve("src/core/cbor.bend"),"utf8");
const dict=(name: string)=>JSON.parse(new RegExp("def Cbor\\."+name+"\\(\\) -> List<&2, String>:\\s*(\\[[\\s\\S]*?\\])\\n").exec(codecSource)![1]);
const keys=dict("keys"),words=dict("words");
const W={encode(text: string){const bytes:number[]=[];encode(JSON.parse(text),bytes);return Uint8Array.from(bytes);},decode(bytes:Uint8Array){return JSON.stringify(plain(cbor(bytes,keys,words)));}};
const out=process.env.PI_OMP_OUT;
if(!out)throw Error("PI_OMP_OUT is required for durable qualification");
mkdirSync(out,{recursive:true});
const checks:string[]=[];
let ws:WebSocket|undefined;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const realPi = process.env.REAL_PI ?? Bun.which("pi") ?? "";

const root = mkdtempSync(join(tmpdir(), "bp-piomp-"));
const fake = join(root, "bin");
const piagent = join(root, "piagent");
const omplog = join(root, "omp.log");
mkdirSync(fake);
mkdirSync(piagent);
for (const c of ["claude", "codex", "grok", "muse", "opencode"]) { writeFileSync(join(fake, c), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, c), 0o755); }
writeFileSync(join(fake, "omp"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/omp_standin.ts")} "$@"\n`);
chmodSync(join(fake, "omp"), 0o755);
if (realPi) {
  writeFileSync(join(fake, "pi"), `#!/bin/sh\nPI_CODING_AGENT_DIR='${piagent}' PI_OFFLINE=1 PI_TELEMETRY=0 exec '${realPi}' "$@"\n`);
  chmodSync(join(fake, "pi"), 0o755);
}
const home = join(root, "home");
const proj = join(root, "proj");
mkdirSync(proj);
mkdirSync(home);
writeFileSync(join(home, "events.jsonl"), ["mail.on","restart.continue","telemetry.share"].map(key=>JSON.stringify({$:"SettingSet",key,value:"off",at:Math.floor(Date.now()/1000)})).join("\n")+"\n");

// the stand-in model: "run please" asks for bash once, "slow" waits, else an echo
const calls: any[] = [];
const model = Bun.serve({ port: 0, hostname: "127.0.0.1", async fetch(req) {
  const body: any = await req.json();
  const msgs = body.messages ?? [];
  const last = msgs[msgs.length - 1] ?? {};
  const user = [...msgs].reverse().find((m: any) => m.role === "user");
  const utext = typeof user?.content === "string" ? user.content : (user?.content ?? []).map((c: any) => c.text ?? "").join("");
  calls.push({ n: msgs.length, roles: msgs.map((m: any) => m.role), tools: (body.tools ?? []).map((t: any) => t.function?.name), last, utext });
  if (utext.includes("slow")) await sleep(30000);
  const base = { id: "c" + calls.length, object: "chat.completion.chunk", created: 1, model: body.model };
  const chunks: any[] = [];
  if (utext.includes("run please") && last.role !== "tool") {
    chunks.push({ ...base, choices: [{ index: 0, delta: { role: "assistant", tool_calls: [{ index: 0, id: "call_" + calls.length, type: "function", function: { name: "bash", arguments: JSON.stringify({ command: "echo e2e > ran.txt" }) } }] } }] });
    chunks.push({ ...base, choices: [{ index: 0, delta: {}, finish_reason: "tool_calls" }] });
  } else {
    const said = last.role === "tool" ? "DONE: " + JSON.stringify(last.content).slice(0, 80) : "ECHO: " + utext.slice(0, 60);
    chunks.push({ ...base, choices: [{ index: 0, delta: { role: "assistant", content: said } }] });
    chunks.push({ ...base, choices: [{ index: 0, delta: {}, finish_reason: "stop" }] });
  }
  const enc = new TextEncoder();
  return new Response(new ReadableStream({ start(c) { for (const ch of chunks) c.enqueue(enc.encode("data: " + JSON.stringify(ch) + "\n\n")); c.enqueue(enc.encode("data: [DONE]\n\n")); c.close(); } }),
    { headers: { "content-type": "text/event-stream" } });
} });
writeFileSync(join(piagent, "models.json"), JSON.stringify({ providers: { standin: { baseUrl: `http://127.0.0.1:${model.port}/v1`, api: "openai-completions", apiKey: "x",
  models: [{ id: "fake-1", name: "Fake One", reasoning: false }] } } }));
writeFileSync(join(piagent, "settings.json"), "{}");

function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(50);
  }
};
const olines = () => (existsSync(omplog) ? readFileSync(omplog, "utf8").split("\n").filter(Boolean) : []);
let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) {fail++;throw Error(what);}
  checks.push(what);
};

const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_COPY:"1", BACKPLANE_TELEMETRY:"off", BACKPLANE_LOCK_DIR:join(home,"locks"),
    OMP_LOG: omplog, OMP_STATE: join(root, "omp-sessions"), BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: Bun.file(join(out,"service.stdout.log")),
  stderr: Bun.file(join(out,"service.stderr.log")),
});
try {
  for (let i = 0; i < 200; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
  const seen: any[] = [];
  const info: Record<string, string> = {};
  const token=readFileSync(join(home,"token"),"utf8").trim();
  if(!/^[0-9a-f]{32}$/.test(token))throw Error("Invalid isolated service credential");
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`,{headers:{Authorization:"Bearer "+token}});
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(c);
    if (o?.info) Object.assign(info, o.info);
  };
  await new Promise((r, j) => {const timer=setTimeout(()=>j(Error("Authenticated isolated WebSocket deadline")),5000); ws!.onopen=()=>{clearTimeout(timer);r(true)};ws!.onerror=()=>{clearTimeout(timer);j(Error("Authenticated isolated WebSocket error"))}; });
  let id = 0;
  const send = (m: string, p: object) => { ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))); return id; };
  const models = () => { try { return JSON.parse(info.models ?? "{}"); } catch { return {}; } };
  const turns = (th: string) => seen.filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => String(x.state));
  const said = (th: string) => seen.filter((x) => x.$ === "MessagePosted" && x.thread === th).map((x) => String(x.text ?? "")).join("\n");
  const acts = (th: string) => seen.filter((x) => x.$ === "ActivityLogged" && x.thread === th).map((x) => String(x.summary ?? x.text ?? x.detail ?? ""));
  const ended = (th: string, n: number) => until(40000, () => turns(th).filter((s) => ["completed", "failed", "interrupted"].includes(s)).length >= n ? true : undefined);
  const nextAsk = (k: number) => until(20000, () => seen.filter((x) => x.$ === "AskOpened")[k]);
  const asks = () => seen.filter((x) => x.$ === "AskOpened").length;

  send("project.add", { path: proj });
  const pc = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated" && x.root === proj) ?? seen.find((x) => x.$ === "ProjectCreated" && x.id !== "scratch"));
  if(!pc)throw Error("Synthetic project was not created");
  const thread = async (provider: string, model: string) => {
    const n = seen.filter((x) => x.$ === "ThreadCreated" && x.provider === provider).length;
    send("thread.create", { project: pc.id, title: provider, env: "local", provider, model });
    const t = (await until(10000, () => seen.filter((x) => x.$ === "ThreadCreated" && x.provider === provider)[n]))?.id as string;
    send("thread.modes", { thread: t, runtime: "approval-required" });
    await sleep(300);
    return t;
  };

  // ---------------- Pi ----------------
  if (!realPi) throw Error("Real Pi unavailable; Pi runtime remains unverified");
  else {
    await until(60000, () => (models().pi ?? []).length > 0 ? true : undefined);
    check((models().pi ?? []).includes("standin/fake-1"), `pi models: listed (${JSON.stringify(models().pi)})`);
    check(String(models().about?.pi?.cli ?? "").length > 0, `pi models: its version (${models().about?.pi?.cli})`);
    check(JSON.stringify(models().about?.pi?.efforts?.["standin/fake-1"]) === "[]", "pi models: a model that does not think offers no effort");

    const th = await thread("pi", "standin/fake-1");
    send("turn.start", { thread: th, text: "hello pi", msg: "p-1" });
    await ended(th, 1);
    check(turns(th).at(-1) === "completed", `pi turn: completes (${turns(th).join(",")})`);
    check(said(th).includes("ECHO: "), "pi turn: the reply reaches the thread");
    check(existsSync(join(home, "pi/backplane.js")) && readFileSync(join(home, "pi/backplane.js"), "utf8").includes("backplane:approval"), "pi turn: Backplane's extension is written");
    check(calls.some((c) => (c.tools ?? []).some((t: string) => String(t).startsWith("mcp__backplane__"))), `pi turn: Backplane's MCP tools reach the model (${calls.at(-1)?.tools?.length} tools)`);

    const k1 = asks();
    send("turn.start", { thread: th, text: "run please", msg: "p-2" });
    const a1 = await nextAsk(k1);
    check(!!a1 && a1.kind === "approval" && a1.tool === "bash" && String(a1.detail).includes("echo e2e > ran.txt"), `pi approval: asks with the command (${a1?.kind} ${a1?.tool}: ${a1?.detail})`);
    if (a1) send("ask.answer", { id: a1.id, answer: "accept" });
    await ended(th, 2);
    check(turns(th).at(-1) === "completed" && existsSync(join(proj, "ran.txt")), "pi approval: accepted, the command ran and the turn completed");
    check(acts(th).some((s) => s.includes("echo e2e > ran.txt")), "pi approval: the call is logged by its command");
    check(calls.some((c) => c.roles.length > 4), `pi resume: the conversation goes on in one session (${calls.map((c) => c.n).join(",")})`);

    rmSync(join(proj, "ran.txt"), { force: true });
    const k2 = asks();
    send("turn.start", { thread: th, text: "run please", msg: "p-3" });
    const a2 = await nextAsk(k2);
    if (a2) send("ask.answer", { id: a2.id, answer: "decline" });
    await ended(th, 3);
    check(!!a2 && !existsSync(join(proj, "ran.txt")), "pi decline: the command did not run");
    check(calls.some((c) => JSON.stringify(c.last).includes("declined")), "pi decline: the model hears it was declined");

    // lost: the session file goes, the next turn says so once
    const sdir = join(piagent, "sessions");
    const files: string[] = [];
    const walk = (d: string) => { if (!existsSync(d)) return; for (const f of readdirSync(d, { withFileTypes: true })) { const p = join(d, f.name); if (f.isDirectory()) walk(p); else files.push(p); } };
    walk(sdir);
    const mine = files.filter((f) => f.includes("bp-"));
    check(mine.length === 1, `pi session: one Pi session for the thread (${mine.length})`);
    for (const f of mine) rmSync(f);
    send("turn.start", { thread: th, text: "after loss", msg: "p-4" });
    await ended(th, 4);
    check(acts(th).filter((s) => s.includes("Pi had no saved conversation")).length === 1, "pi lost: said once");
    check(turns(th).at(-1) === "completed", "pi lost: the turn still completes");

    send("turn.start", { thread: th, text: "slow", msg: "p-5" });
    await until(10000, () => turns(th).at(-1) === "running" ? true : undefined);
    await sleep(1500);
    send("turn.interrupt", { thread: th });
    await until(10000, () => turns(th).at(-1) === "interrupted" ? true : undefined);
    check(turns(th).at(-1) === "interrupted", `pi stop: interrupted (${turns(th).slice(-2).join(",")})`);
  }

  // ---------------- Oh My Pi (stand-in) ----------------
  await until(60000, () => (models().omp ?? []).length > 0 ? true : undefined);
  check((models().omp ?? []).includes("standin/fake-omp"), `omp models: listed (${JSON.stringify(models().omp)})`);
  check(String(models().about?.omp?.labels?.["standin/fake-omp"] ?? "").includes("Fake OMP"), "omp models: with their names");
  check(JSON.stringify(models().about?.omp?.efforts?.["standin/fake-omp"]) === '["off","auto","low","high"]', `omp models: its thinking levels (${JSON.stringify(models().about?.omp?.efforts?.["standin/fake-omp"])})`);

  const ot = await thread("omp", "standin/fake-omp");
  send("setting.set", { key: "effort." + ot, value: "high" });
  await sleep(300);
  let mark = olines().length;
  send("turn.start", { thread: ot, text: "hello omp", msg: "o-1" });
  await ended(ot, 1);
  let t = olines().slice(mark);
  check(turns(ot).at(-1) === "completed" && said(ot).includes("ECHO: hello omp"), `omp turn: completes with the reply (${turns(ot).join(",")})`);
  const args = JSON.parse((t.find((l) => l.startsWith("ARGS ")) ?? "ARGS []").slice(5));
  const arg = (f: string) => args[args.indexOf(f) + 1];
  check(arg("--mode") === "rpc-ui" && arg("--approval-mode") === "always-ask", `omp turn: RPC with tool dialogs, supervised is always-ask (${args.slice(0, 4).join(" ")})`);
  check(String(arg("--append-system-prompt")).length > 100, "omp turn: Backplane's instructions as the system prompt");
  check(arg("--model") === "standin/fake-omp" && arg("--thinking") === "high", "omp turn: the thread's model and thinking level");
  check(String(arg("--session-dir")).includes("/omp/sessions/") && args.includes("--continue"), "omp turn: the thread's own session folder");
  const reqs = t.filter((l) => l.startsWith("REQ ")).map((l) => l.split(" ")[1]);
  check(reqs.join(",") === "set_event_filter,get_state,prompt,get_session_stats", `omp turn: the RPC exchange (${reqs.join(",")})`);
  check(t.some((l) => l.startsWith("REQ prompt") && !l.includes("backplane_instructions")), "omp turn: the prompt is the text alone");
  check(t.some((l) => l.startsWith("ENV ") && l.includes("/mcp/")), "omp turn: the thread's Backplane MCP for the extension");
  check(t.some((l) => l.startsWith("TOOLS ") && l.includes("mcp__backplane__thread_list")), "omp turn: Backplane's tools registered by the extension");
  check(acts(ot).some((s) => s.includes("ls notes")), "omp turn: its tool call is logged");
  await until(5000, () => seen.some((x) => x.$ === "TurnUsage" && x.thread === ot) ? true : undefined);
  const tu = seen.find((x) => x.$ === "TurnUsage" && x.thread === ot);
  check(!!tu && tu.provider === "omp" && tu.input === 1000 && tu.output === 20 && String(tu.known).includes("c") && tu.cost === 2000000, `omp usage: the turn's tokens and cost (${JSON.stringify(tu)})`);
  check(seen.some((x) => x.$ === "UsageSet" && x.thread === ot && x.used === 1020), "omp usage: its context fill");

  mark = olines().length;
  send("turn.start", { thread: ot, text: "again", msg: "o-2" });
  await ended(ot, 2);
  t = olines().slice(mark);
  const st = (t.find((l) => l.startsWith("STATE ")) ?? "STATE - 0").split(" ");
  check(Number(st[2]) > 0 && st[1] === seen.filter((x) => x.$ === "SessionSet" && x.id === ot).at(-1)?.session, `omp resume: the session goes on (${st.slice(1).join(" ")})`);
  const tus = seen.filter((x) => x.$ === "TurnUsage" && x.thread === ot);
  check(tus.length === 2 && tus[1].input === 1000 && tus[1].cost === 2000000, `omp usage: the second turn counts its own share (${tus.map((x) => x.input).join(",")})`);

  const k3 = asks();
  send("turn.start", { thread: ot, text: "perm please", msg: "o-3" });
  const p1 = await nextAsk(k3);
  check(!!p1 && p1.kind === "approval" && p1.tool === "bash" && String(p1.detail).includes("rm -rf junk"), `omp approval: reaches the person (${p1?.tool}: ${p1?.detail})`);
  if (p1) send("ask.answer", { id: p1.id, answer: "accept" });
  await ended(ot, 3);
  check(olines().some((l) => l.startsWith("UI") && l.includes('"value":"Approve"')), "omp approval: accept approves once");

  const k4 = asks();
  send("turn.start", { thread: ot, text: "write please", msg: "o-4" });
  const e1 = await nextAsk(k4);
  check(!!e1 && e1.kind === "approval" && e1.tool === "write" && String(e1.detail).includes("notes/a.txt"), `omp write approval: reaches the person (${e1?.tool}: ${e1?.detail})`);
  if (e1) send("ask.answer", { id: e1.id, answer: "decline" });
  await ended(ot, 4);
  check(olines().some((l) => l.startsWith("UI") && l.includes('"value":"Deny"')), "omp write approval: decline answers Deny");

  const k5 = asks();
  send("turn.start", { thread: ot, text: "ask please", msg: "o-5" });
  const q = await nextAsk(k5);
  check(!!q && q.kind === "input" && String(q.detail).includes("Which colour?") && String(q.options).includes("Blue"), `omp question: reaches the person (${q?.kind}: ${q?.detail} / ${q?.options})`);
  if (q) send("ask.answer", { id: q.id, answer: "Blue" });
  await ended(ot, 5);
  check(olines().some((l) => l.startsWith("UI") && l.includes('"value":"Blue"')), "omp question: the answer goes back");

  const k6 = asks();
  send("turn.start", { thread: ot, text: "bp tool", msg: "o-6" });
  await ended(ot, 6);
  check(asks() === k6 && olines().some((l) => l.startsWith("BP ") && !l.includes("no such tool") && !l.includes('"isError":true')), "omp: Backplane's own tool runs through the extension, unasked");
  check(acts(ot).some((s) => s.includes("backplane thread_list")), "omp: a Backplane tool reached through omp's xd:// is logged by name");

  const odir = String(arg("--session-dir"));
  rmSync(odir, { recursive: true, force: true });
  send("turn.start", { thread: ot, text: "after loss", msg: "o-7" });
  await ended(ot, 7);
  check(acts(ot).filter((s) => s.includes("Oh My Pi had no saved conversation")).length === 1 && turns(ot).at(-1) === "completed", "omp lost: starts afresh, said once, completes");

  writeFileSync(join(out,"RESULT.json"),JSON.stringify({state:"passed",source:process.env.PI_OMP_SOURCE,checks,realPi:true,ompStandin:true,scope:"Actual service runner with real Pi against an isolated local model and OMP RPC stand-in. No user profile, credentials, real model provider or GUI."},null,2)+"\n");
} catch(e:any) {
  writeFileSync(join(out,"FAIL.json"),JSON.stringify({state:"failed",source:process.env.PI_OMP_SOURCE,checks,error:e.stack,home:root,scope:"Isolated synthetic runner; completed prefix retained; no real provider or user UI"},null,2)+"\n");
  console.error(e.stack);fail++;
} finally {
  ws?.close();
  proc.kill();
  await proc.exited;
  model.stop(true);
}

if (process.env.KEEP) console.log("kept " + root);
else rmSync(root, { recursive: true, force: true });
console.log(fail ? `${fail} FAILED` : "all ok");
process.exit(fail ? 1 : 0);
