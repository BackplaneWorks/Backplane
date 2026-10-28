// Non-holder uberbot chat regression (persistent-bub, for the backend lead's repair).
// Two synthetic hubs (alpha, beta; --no-tailscale, BACKPLANE_PEERS, stand-in agents,
// throwaway homes). Uberbot "Tub" promoted on alpha, joined on beta, home moved to beta.
// From alpha (the NON-holder) a marked message goes to Tub's chat through the far chat id
// (as the client's Ub.chat does). Checks: beta (holder) got it and the stand-in replied;
// alpha's mirror file holds both; alpha's thin views (web: view=1&frontend=1, native:
// view=1&mobile=1) of bot Tub show the message and the reply, as beta's holder view does.
// Optional (--general): the same for a plain thread of beta opened on alpha by its far id.
//   bun nonholder_chat_e2e.ts BINARY WIREDIR [--baseline] [--general]
// --baseline: today's bug is EXPECTED (alpha's thin rows lack the message); exit 0 when it
// reproduces. Without it the repaired build must show them. On any failure the synthetic
// homes are kept and their path printed. Never uses a live home, credential or window.
// End to end: one uberbot on two machines (core/ubersync.bend, docs/uber.md
// "Sync"). Two headless hubs that pair by themselves (as far_e2e.ts) with a
// short grace (BACKPLANE_UBER_GRACE=30; the holder beats every 15 s). Checks: a bot promoted on alpha is
// offered to beta; beta joins; memory saved on either machine reaches the
// other's replica; a routine made on alpha runs on exactly one machine (the
// holder); alpha stopped, beta takes the lease within the grace and a
// minute and runs the routine; alpha started again steps down (a higher
// term) and does not run it. Agents are stand-ins. Prints "ok ..." /
// "FAIL ..." lines.
//
//   bun test/tools/uber_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane-serve; WIREDIR to build/wire.
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync, chmodSync, mkdirSync } from "node:fs";
import { ThinViews } from '../../src/web/thin.js';
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const args = process.argv.slice(2);
const [bin = "build/backplane-serve", wire = "build/wire"] = args.filter((a) => !a.startsWith("--"));
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 600)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

const root = mkdtempSync(join(tmpdir(), "bp-nonholder-chat-"));
const fake = join(root, "bin");
mkdirSync(fake);
const claude = `#!/bin/sh
n=0
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      n=$((n + 1))
      printf '%s\\n' '{"type":"assistant","message":{"id":"m'"$$-$n"'","content":[{"type":"text","text":"STANDIN-REPLY"}]}}'
      printf '%s\\n' '{"type":"result","is_error":false,"result":"STANDIN-REPLY"}' ;;
  esac
done
`;
for (const n of ["claude", "codex", "grok", "muse"]) {
  writeFileSync(join(fake, n), n === "claude" ? claude : "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}

type Hub = { name: string; home: string; port: number; peers: string; proc: ReturnType<typeof Bun.spawn> | null };
type Client = { ws: WebSocket; seen: any[]; replies: Map<number, any>; n: number; info: any };

function spawn(h: Hub) {
  h.proc = Bun.spawn([resolve(bin), "--home", h.home, "--port", String(h.port), "--no-tailscale"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NAME: h.name, BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: h.peers, BACKPLANE_UBER_GRACE: "30", BACKPLANE_AGENTS: fake, PATH: `${fake}:/usr/bin:/bin` },
    stdout: Bun.file(join(root, `${h.name}.stdout`)),
    stderr: Bun.file(join(root, `${h.name}.stderr`)),
  });
}

async function up(h: Hub) {
  for (let i = 0; i < 200; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${h.port}/hello`);
      if (r.status === 200) return;
    } catch {}
    await sleep(100);
  }
  throw new Error(`${h.name} did not start`);
}

async function connect(h: Hub): Promise<Client> {
  const c: Client = { ws: null as any, seen: [], replies: new Map(), n: 0, info: {} };
  const ws = new WebSocket(`ws://127.0.0.1:${h.port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));
    for (const x of o.items ?? []) c.seen.push(x);
    if (o.t === "reply") c.replies.set(Number(o.id), o);
    if (o.info) c.info = { ...c.info, ...o.info };
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  c.ws = ws;
  return c;
}

async function until<T>(ms: number, f: () => T | undefined | null | false): Promise<T | undefined> {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(50);
  }
}

async function rpc(c: Client, m: string, p: object): Promise<any> {
  const id = ++c.n;
  c.ws.send(W.encode(JSON.stringify({ id, m, p })));
  return await until(30000, () => c.replies.get(id));
}

// the setting value a client's log last gave a key
const setting = (c: Client, key: string) => c.seen.filter((x) => x.$ === "SettingSet" && x.key === key).at(-1)?.value as string | undefined;
// the uberbot's ops a client has, with their bodies parsed
const ops = (c: Client) => c.seen.filter((x) => x.$ === "UberOp").map((x) => ({ ...x, body: JSON.parse(x.op) }));
const leases = (c: Client) => ops(c).filter((o) => o.body.k === "lease");
const routineRuns = (c: Client, thread: string) => c.seen.filter((x) => x.$ === "MessagePosted" && x.thread === thread && String(x.text).startsWith("[routine:"));

// an MCP tools/call from the agent of a thread (the hub's /mcp/<thread>)
let mcpId = 0;
async function mcp(h: Hub, thread: string, name: string, args: object): Promise<{ isError: boolean; text: string }> {
  const r: any = await fetch(`http://127.0.0.1:${h.port}/mcp/${thread}`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: ++mcpId, method: "tools/call", params: { name, arguments: args } }),
  }).then((x) => x.json());
  const text = String(r.result?.content?.[0]?.text ?? "");
  let body = text;
  try { body = JSON.parse(text)?.text ?? text; } catch {}
  return { isError: !!r.result?.isError || !!r.error, text: body };
}


const baseline = args.includes("--baseline"), general = args.includes("--general");
const pa = freePort(), pb = freePort();
const a: Hub = { name: "alpha", home: join(root, "alpha"), port: pa, peers: `http://127.0.0.1:${pb}`, proc: null };
const b: Hub = { name: "beta", home: join(root, "beta"), port: pb, peers: `http://127.0.0.1:${pa}`, proc: null };
const clients: Client[] = [];
const MARK = `NONHOLDER-MARK-${Date.now()}`;
// every string in a JSON value, for "is this text visible in what the view sent"
const strings = (v: any, out: string[] = []): string[] => { if (typeof v === "string") out.push(v); else if (v && typeof v === "object") for (const x of Object.values(v)) strings(x, out); return out; };
// A thin view connection (web or native renderer): greeting, then view.open for a target;
// answers every string of the view reply. Unverified against a live hub: request envelope
// {id, m:"view.open", p:{version:1, hub, generation, epoch, requestRevision, target, viewId:""}}
// as src/web/thin.js builds it; adjust p here if the hub answers "invalid".
async function thinView(h: Hub, mode: "web" | "native", target: { kind: string; id: string }): Promise<{ ok: boolean; texts: string[]; raw: any }> {
  const q = mode === "web" ? "view=1&frontend=1" : "view=1&mobile=1";
  const tok = readFileSync(join(h.home, "token"), "utf8").trim();
  const ws = new WebSocket(`ws://127.0.0.1:${h.port}/ws?${q}&token=${tok}`);
  ws.binaryType = "arraybuffer";
  const frames: any[] = [];
  ws.onmessage = (e) => { try { frames.push(JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)))); } catch {} };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  const greet = await until(10000, () => frames.find((f) => typeof f?.generation === "string" && typeof f?.epoch === "string") ?? frames.find((f) => f?.greeting)?.greeting);
  const g = (greet as any) ?? {};
  if(!g.generation||!g.epoch)throw Error('thin greeting missing');
  const views=new ThinViews({now:()=>performance.now(),send:(_h:any,r:any)=>r,command:(c:any)=>c});
  if(!views.connect('fixture',g).connected)throw Error('production thin receiver rejected greeting');
  const session:any={sel:target.kind==='thread'?target.id:'',rid:1,drafts:{},open:{},info:{},adding:false,panel:false};
  if(target.kind==='bot')session.route={kind:'bot-tab',bot:target.id,tab:'chat'};
  const call=views.select('fixture',target,session,'mirrored-chat-fixture');
  if(call.failed||call.cmds.length!==1)throw Error('production thin open failed');
  ws.send(W.encode(JSON.stringify(call.cmds[0])));
  const reply:any = await until(15000, () => frames.find((f) => f?.t === "reply" && Number(f.id) === call.id));
  const received=reply?views.reply('fixture',reply):null;
  if(reply?.ok&&!received?.current)throw Error('thin receiver rejected current reply');

  ws.close();
  return { ok: !!(reply as any)?.ok, texts: strings(reply ?? {}), raw: reply ?? { frames: frames.slice(0, 3) } };
}
const sees = (texts: string[], s: string) => texts.some((t) => t.includes(s));
let keep = false;
try {
  spawn(a); spawn(b); await up(a); await up(b);
  const ca = await connect(a); const cb = await connect(b); clients.push(ca, cb);
  const inv = await rpc(ca, "bots.invite", { url: `http://127.0.0.1:${pa}` });
  const jn = await rpc(cb, "bots.join", { invite: inv?.invite ?? "" });
  const peer = await until(60000, () => ca.seen.find((c) => c.$ === "PeerSet" && !c.revoked));
  const link = peer?.id as string;
  check("linked by invite", !!inv?.ok && !!jn?.ok && !!link, [inv?.ok, jn?.ok, link]);
  await rpc(ca, "bots.create", { name: "Tub", persona: "a synthetic uberbot" });
  const tubA = await until(10000, () => ca.seen.find((c) => c.$ === "BotSet" && c.name === "Tub"));
  const promoted = await rpc(ca, "bots.uber", { bot: tubA?.id });
  const uid = promoted?.uid as string;
  const offer = await until(180000, () => { const o = setting(cb, "uber.offer"); return o ? JSON.parse(o) : undefined; });
  const joined = await rpc(cb, "bots.uber_join", { uid: offer?.uid ?? uid });
  const tubB = await until(10000, () => cb.seen.find((c) => c.$ === "BotSet" && c.id === joined?.bot));
  check("Tub promoted on alpha and joined on beta", !!promoted?.ok && !!joined?.ok && !!tubB?.thread, [promoted?.ok, joined]);
  const idB = setting(cb, "machine.id");
  const homed = await rpc(cb, "bots.uber_home", {});
  const learned = await until(120000, () => leases(ca).find((o) => o.body.holder === idB));
  check("home moved to beta; alpha learned it", !!homed?.ok && !!learned, learned?.body);
  if(!learned)throw Error("lease transfer did not synchronize");
  // the non-holder's chat is the holder's thread as mirrored (ubersync.bend's Ub.chat)
  const farChat = `${link}~${tubB.thread}`;
  const sent = await rpc(ca, "turn.start", { thread: farChat, text: MARK, msg: `nh-${Date.now()}`, mode: "queue" });
  check("alpha sends to Tub's far chat", !!sent?.ok, sent);
  const onB = await until(60000, () => cb.seen.find((c) => c.$ === "MessagePosted" && c.thread === tubB.thread && String(c.text).includes(MARK)));
  const replyB = await until(60000, () => cb.seen.find((c) => c.$ === "MessagePosted" && c.thread === tubB.thread && c.role === "assistant" && c.text === "STANDIN-REPLY"));
  check("the holder (beta) got it and the stand-in replied", !!onB && !!replyB, [!!onB, !!replyB]);
  const mirror = join(a.home, "far", `${link}.jsonl`);
  const held = await until(60000, () => { try { const t = readFileSync(mirror, "utf8"); return t.includes(MARK) && t.includes("STANDIN-REPLY"); } catch { return false; } });
  check("alpha's mirror file holds the message and the reply", !!held, mirror);
  if(!held)throw Error("mirror did not receive both rows");
  const holderWeb = await thinView(b, "web", { kind: "bot", id: tubB.id });
  check("holder control: beta's web thin view of Tub shows both", holderWeb.ok && sees(holderWeb.texts, MARK) && sees(holderWeb.texts, "STANDIN-REPLY"), holderWeb.raw);
  if(!holderWeb.ok||!sees(holderWeb.texts,MARK)||!sees(holderWeb.texts,"STANDIN-REPLY"))throw Error("holder control failed");
  for (const mode of ["web", "native"] as const) {
    const v = await thinView(a, mode, { kind: "bot", id: tubA.id });
    const visible = v.ok && sees(v.texts, MARK) && sees(v.texts, "STANDIN-REPLY");
    if (baseline) check(`baseline: alpha's ${mode} thin view of Tub LACKS them (the bug reproduces)`, v.ok && !visible, { ok: v.ok, visible });
    else check(`alpha's ${mode} thin view of Tub (non-holder) shows the message and the reply`, visible, { ok: v.ok, sample: v.texts.slice(0, 20) });
  }
  if (general) {
    mkdirSync(join(root,"board"),{recursive:true});
    const pc = await rpc(cb, "project.add", { path: join(root, "board") }).catch(() => null);
    const proj = await until(5000, () => cb.seen.find((c) => c.$ === "ProjectCreated" && String(c.root).endsWith("/board")));
    await rpc(cb, "thread.create", { project: proj?.id, title: "PLAIN FAR" });
    const pt = await until(5000, () => cb.seen.find((c) => c.$ === "ThreadCreated" && c.title === "PLAIN FAR"));
    const G = `PLAIN-MARK-${Date.now()}`;
    await rpc(cb, "turn.start", { thread: pt?.id, text: G, msg: `pl-${Date.now()}`, mode: "queue" });
    await until(60000, () => { try { return readFileSync(mirror, "utf8").includes(G); } catch { return false; } });
    for (const mode of ["web", "native"] as const) {
      const v = await thinView(a, mode, { kind: "thread", id: `${link}~${pt?.id}` });
      const visible = v.ok && sees(v.texts, G);
      check(`general: alpha's ${mode} thin view of a plain far thread ${baseline ? "(observed only)" : "shows its message"}`, baseline ? true : visible, { ok: v.ok, visible });
      if (baseline) console.log(`  general far thread, ${mode}: visible=${visible}`);
    }
  }
} catch (e) { console.log("FAIL threw: " + String(e)); failed = true; }
finally {
  for (const c of clients) { try { c.ws.close(); } catch {} }
  for (const h of [a, b]) { try { h.proc?.kill(); } catch {} }
  if (failed || args.includes("--keep")) console.log("synthetic homes kept: " + root); else rmSync(root, { recursive: true, force: true });
}
console.log(failed ? "nonholder chat: FAIL" : `nonholder chat: ok${baseline ? " (baseline: bug reproduced)" : ""}`);
process.exit(failed ? 1 : 0);
