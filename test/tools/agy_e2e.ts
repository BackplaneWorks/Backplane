// Google Antigravity end to end, against a stand-in agent
// (test/tools/agy_standin.ts, answering as agy_acp_server 1.3.0 does; no
// model, no Google account). A headless hub on a temporary home:
//   install      Settings' Install runs the real install script on a zip
//                of the stand-in (BACKPLANE_AGY_SOURCE: file://, its size
//                and SHA-256): checked, unpacked, in place; the models info
//                says it is installed. A wrong hash is refused.
//   key          the API key goes to the API keys file, never to the log;
//                Check lists the agent's models for every client
//   turn         an antigravity thread's turn: authenticate, session/new
//                with Backplane's MCP, the mode, the model, the prompt with
//                Backplane's rules; only that key in the agent's
//                environment, GEMINI_HOME its profile; the reply streams
//                and the turn completes
//   resume       the next turn resumes the session, no rules again
//   approval     a supervised turn's permission request reaches the person
//                as an ask; accepting it lets the agent go on
//   google       a Google sign-in: the address reaches the client that
//                asked, a foreign way back is refused, this one's is
//                delivered on the hub's loopback, the account shows signed in
//   no sign-in   after Sign out, a turn that would need Google's page fails
//                saying to sign in, and no browser opens
//
//   bun test/tools/agy_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-agy-"));
const fake = join(root, "bin");
const log = join(root, "agy.log");
mkdirSync(fake);
for (const c of ["claude", "codex", "grok", "muse"]) { writeFileSync(join(fake, c), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, c), 0o755); }
const home = join(root, "home");
const proj = join(root, "proj");
mkdirSync(proj);
mkdirSync(home);
writeFileSync(join(home, "events.jsonl"), "");

// the archive: the stand-in as agy_acp_server.par, and a harness
const pkg = join(root, "pkg");
mkdirSync(pkg);
writeFileSync(join(pkg, "agy_acp_server.par"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/agy_standin.ts")} "$@"\n`);
writeFileSync(join(pkg, "localharness_external"), "#!/bin/sh\nexit 0\n");
const zip = join(root, "agy.zip");
const z = Bun.spawnSync(["python3", "-I", "-c", "import sys,zipfile\nz=zipfile.ZipFile(sys.argv[1],'w')\nfor n in sys.argv[3:]: z.write(sys.argv[2]+'/'+n, n)\nz.close()", zip, pkg, "agy_acp_server.par", "localharness_external"]);
if (z.exitCode !== 0) throw new Error("zip: " + z.stderr.toString());
const sha = new Bun.CryptoHasher("sha256").update(readFileSync(zip)).digest("hex");
const size = statSync(zip).size;

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
const lines = () => (existsSync(log) ? readFileSync(log, "utf8").split("\n").filter(Boolean) : []);
let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};

const start = (src: string, at = home) => {
  const port = freePort();
  const proc = Bun.spawn([resolve(bin), "--home", at, "--port", String(port), "--no-tailscale", "--foreground"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", AGY_LOG: log,
      BACKPLANE_AGY_SOURCE: src, GEMINI_API_KEY: "ambient-must-not-leak", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
    stdout: "ignore",
    stderr: "ignore",
  });
  return { port, proc };
};

const { port, proc } = start(`file://${zip} ${size} ${sha}`);
try {
  for (let i = 0; i < 200; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
  const seen: any[] = [];
  const info: Record<string, string> = {};
  const replies: Record<number, any> = {};
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(c);
    if (o?.info) Object.assign(info, o.info);
    if (o?.t === "reply") replies[o.id] = o;
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => { ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))); return id; };
  const agy = (p: object) => send("bots.voice", { op: "agy", ...p });
  const job = () => { try { return JSON.parse(info["agy.job"] ?? "{}"); } catch { return {}; } };
  const models = () => { try { return JSON.parse(info.models ?? "{}"); } catch { return {}; } };
  const turns = (th: string) => seen.filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => String(x.state));
  const said = (th: string) => seen.filter((x) => x.$ === "MessagePosted" && x.thread === th).map((x) => String(x.text ?? "")).join("\n");

  // install
  agy({ do: "install" });
  await until(60000, () => job().what === "install" && job().state !== "running" ? true : undefined);
  check(job().state === "done", `install: done (${job().state}: ${job().msg})`);
  check(existsSync(join(home, "antigravity/1.3.0/.ok")) && existsSync(join(home, "antigravity/agy.sh")), "install: the agent and its runner are in place");
  await until(10000, () => models().agy?.have === "1.3.0" ? true : undefined);
  check(models().agy?.have === "1.3.0" && models().agy?.pin === "1.3.0", `install: every client hears it is installed (${JSON.stringify(models().agy)})`);

  // key
  send("setting.set", { key: "agy.auth", value: "gemini-api-key" });
  send("bots.voice", { op: "api-add", name: "GEMINI_API_KEY", value: "test-key-123" });
  await sleep(500);
  agy({ do: "signin", method: "gemini-api-key", account: "default" });
  await until(20000, () => job().what === "signin" && ["done", "failed"].includes(job().state) ? true : undefined);
  check(job().state === "done", `key: Check connects (${job().state}: ${job().msg})`);
  await until(10000, () => (models().antigravity ?? []).length > 0 ? true : undefined);
  check((models().antigravity ?? []).includes("gemini-3.1-pro-low"), `key: the agent's models reach the clients (${(models().antigravity ?? []).length})`);
  check(models().about?.antigravity?.labels?.["gemini-3.8-flash-high"]?.includes("Gemini 3.8 Flash"), "key: with their names");
  check(!readFileSync(join(home, "events.jsonl"), "utf8").includes("test-key-123"), "key: never in the log");

  // turn
  send("project.add", { path: proj });
  const pc = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated" && x.root === proj) ?? seen.find((x) => x.$ === "ProjectCreated" && x.id !== "scratch"));
  const n0 = seen.filter((x) => x.$ === "ThreadCreated").length;
  send("thread.create", { project: pc.id, title: "agy", env: "local", provider: "antigravity", model: "gemini-3.1-pro-low" });
  const th = (await until(10000, () => seen.filter((x) => x.$ === "ThreadCreated" && x.provider === "antigravity")[0]))?.id as string;
  send("thread.modes", { thread: th, runtime: "approval-required" });
  await sleep(300);
  const mark = lines().length;
  send("turn.start", { thread: th, text: "hello there", msg: "m-1" });
  await until(20000, () => ["completed", "failed"].includes(turns(th).at(-1) ?? "") ? true : undefined);
  const t1 = lines().slice(mark);
  check(turns(th).at(-1) === "completed", `turn: completes (${turns(th).join(",")})`);
  check(said(th).includes("hello there") && said(th).includes("ECHO: "), "turn: the reply reaches the thread");
  const reqs = t1.filter((l) => l.startsWith("REQ ")).map((l) => l.split(" ")[1]);
  check(reqs.join(",") === "initialize,authenticate,session/new,session/set_mode,session/set_config_option,session/prompt", `turn: the ACP exchange (${reqs.join(",")})`);
  check(t1.some((l) => l.startsWith("REQ session/new") && l.includes("/mcp/")), "turn: Backplane's MCP is in the session");
  check(t1.some((l) => l.startsWith("PROMPT") && l.endsWith("+rules")), "turn: a fresh session gets Backplane's rules");
  check(t1.some((l) => l === "MODE default") && t1.some((l) => l === "MODEL gemini-3.1-pro-low"), "turn: supervised mode and the thread's model");
  const st = t1.find((l) => l.startsWith("START")) ?? "";
  check(st.includes("gemkey=test-key-123") && !st.includes("ambient"), `turn: only the saved key reaches the agent (${st.replace(/home=\S+ tmp=\S+ /, "")})`);
  check(st.includes(`home=${home}/antigravity/profiles/default`) && st.includes("args=--uid=") === (process.platform === "linux"), "turn: its profile as GEMINI_HOME");
  check(t1.some((l) => l.startsWith("SETTINGS") && l.includes('"type":"gemini-api-key"')), "turn: the method's settings.json");
  check(seen.some((x) => x.$ === "ThreadUpdated" || x.$ === "SessionSet" || (x.thread === th && x.session)), "turn: the session is kept");

  // resume
  const mark2 = lines().length;
  send("turn.start", { thread: th, text: "again", msg: "m-2" });
  await until(20000, () => turns(th).filter((s) => s === "completed").length >= 2 ? true : undefined);
  const t2 = lines().slice(mark2);
  check(t2.some((l) => l.startsWith("REQ session/resume")) && !t2.some((l) => l.startsWith("REQ session/new")), "resume: the session goes on");
  check(t2.some((l) => l.startsWith("PROMPT") && !l.endsWith("+rules")), "resume: no rules again");

  // approval
  const asks0 = seen.filter((x) => x.$ === "AskOpened").length;
  send("turn.start", { thread: th, text: "perm please", msg: "m-3" });
  const ask = await until(20000, () => seen.filter((x) => x.$ === "AskOpened")[asks0]);
  check(!!ask && String(ask.detail ?? "").includes("rm -rf junk"), `approval: the request reaches the person (${ask?.tool}: ${ask?.detail})`);
  if (ask) send("ask.answer", { id: ask.id, answer: "accept" });
  await until(20000, () => turns(th).filter((s) => s === "completed").length >= 3 ? true : undefined);
  check(lines().some((l) => l.startsWith("PERMIT") && l.includes('"optionId":"a1"')), "approval: accepting allows once");
  check(turns(th).filter((s) => s === "completed").length >= 3, "approval: the turn goes on and completes");

  // google
  send("setting.set", { key: "agy.auth", value: "oauth-personal" });
  agy({ do: "signin", method: "oauth-personal", account: "default" });
  await until(20000, () => job().state === "waiting" ? true : undefined);
  const url = String(job().url ?? "");
  check(url.startsWith("https://accounts.google.com/o/oauth2/v2/auth?") && url.includes("redirect_uri=http%3A%2F%2F127.0.0.1"), `google: the sign-in address reaches the client (${url.slice(0, 60)})`);
  const back = new URL(decodeURIComponent(new URL(url).searchParams.get("redirect_uri") ?? ""));
  const state = new URL(url).searchParams.get("state");
  const bad = agy({ do: "back", url: `http://127.0.0.1:${back.port}/?state=WRONG&code=x` });
  await until(5000, () => replies[bad]);
  check(replies[bad]?.ok === false, `google: a foreign way back is refused (${replies[bad]?.error})`);
  const good = agy({ do: "back", url: `http://127.0.0.1:${back.port}/?state=${state}&code=abc&scope=s` });
  await until(10000, () => replies[good]);
  check(replies[good]?.ok === true, `google: this sign-in's way back is delivered (${replies[good]?.error ?? ""})`);
  await until(20000, () => job().state === "done" ? true : undefined);
  check(job().state === "done" && lines().includes("SIGNED abc"), `google: signed in (${job().msg})`);
  await until(10000, () => (models().agy?.signed ?? []).includes("default") ? true : undefined);
  check((models().agy?.signed ?? []).includes("default"), "google: the account shows signed in");
  check(!readFileSync(join(home, "events.jsonl"), "utf8").includes("refresh_token"), "google: no token in the log");

  // no sign-in
  agy({ do: "signout", account: "default" });
  await until(10000, () => !(models().agy?.signed ?? []).includes("default") ? true : undefined);
  check(!existsSync(join(home, "antigravity/profiles/default/antigravity-acp/acp_token.json")), "signout: the agent's token is gone");
  const listens = lines().filter((l) => l.startsWith("LISTEN")).length;
  send("turn.start", { thread: th, text: "needs google", msg: "m-4" });
  await until(20000, () => turns(th).at(-1) === "failed" ? true : undefined);
  check(turns(th).at(-1) === "failed", `no sign-in: the turn fails (${turns(th).slice(-2).join(",")})`);
  check(seen.some((x) => x.thread === th && String(x.summary ?? x.text ?? x.message ?? "").includes("not signed in")), "no sign-in: it says to sign in");
  check(lines().filter((l) => l.startsWith("LISTEN")).length === listens + 1, "no sign-in: the agent's page went to Backplane, not a browser");
} finally {
  proc.kill();
  await proc.exited;
}

// a wrong hash is refused, nothing is installed
const home2 = join(root, "home2");
mkdirSync(home2);
writeFileSync(join(home2, "events.jsonl"), "");
const h2 = start(`file://${zip} ${size} ${"0".repeat(64)}`, home2);
try {
  for (let i = 0; i < 200; i++) {
    try { if ((await fetch(`http://127.0.0.1:${h2.port}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
  const info: Record<string, string> = {};
  const ws = new WebSocket(`ws://127.0.0.1:${h2.port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    if (o?.info) Object.assign(info, o.info);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  ws.send(W.encode(JSON.stringify({ id: 1, m: "bots.voice", p: { op: "agy", do: "install" } })));
  const j = () => { try { return JSON.parse(info["agy.job"] ?? "{}"); } catch { return {}; } };
  await until(60000, () => j().state === "failed" || j().state === "done" ? true : undefined);
  check(j().state === "failed" && String(j().msg).includes("SHA-256"), `install: a wrong hash is refused (${j().msg})`);
  check(!existsSync(join(home2, "antigravity/1.3.0")) && !existsSync(join(home2, "antigravity/1.3.0.new")), "install: nothing is unpacked");
} finally {
  h2.proc.kill();
  await h2.proc.exited;
}

if (process.env.KEEP) console.log("kept " + root);
else rmSync(root, { recursive: true, force: true });
console.log(fail ? `${fail} FAILED` : "all ok");
process.exit(fail ? 1 : 0);
