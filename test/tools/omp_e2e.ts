// oh-my-pi end to end, against a stand-in agent (test/tools/omp_standin.ts,
// answering as `omp acp` does; no model, no omp login). A headless hub on a
// temporary home, with the stand-in as `omp` first on PATH:
//   stream       an omp thread's turn streams its reply and completes
//   supervised   form Allow writes; ACP Allow cannot select allow_always
//   deny         Deny leaves no file, and the turn completes
//   full access  a write in yolo asks nothing and creates the file
//
//   bun test/tools/omp_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
type WireApi = { encode(s: string): Uint8Array; decode(b: Uint8Array): string };
const W: WireApi = (globalThis as unknown as { Wire: WireApi }).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-omp-"));
const fake = join(root, "bin");
const log = join(root, "omp.log");
mkdirSync(fake);
for (const c of ["claude", "codex", "grok", "muse", "agy"]) { writeFileSync(join(fake, c), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, c), 0o755); }
// omp: `acp` is the stand-in; `models --json` lists one chat model; anything else is the version
writeFileSync(join(fake, "omp"), `#!/bin/sh\ncase "$1" in\n  acp) exec ${process.execPath} ${resolve("test/tools/omp_standin.ts")} "$@" ;;\n  models) echo '{"models":[{"kind":"chat","selector":"anthropic/claude-haiku-5-5"}]}' ;;\n  *) echo "omp 18.8.8" ;;\nesac\n`);
chmodSync(join(fake, "omp"), 0o755);
const home = join(root, "home");
const proj = join(root, "proj");
mkdirSync(proj);
mkdirSync(home);
writeFileSync(join(home, "events.jsonl"), "");

const port = (() => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; })();
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(50);
  }
};
let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};

const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", OMP_LOG: log, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: "ignore",
});
try {
  for (let i = 0; i < 200; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
  const seen: Record<string, unknown>[] = [];
  const replies: Record<number, unknown> = {};
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: { items?: Record<string, unknown>[]; t?: string; id?: number } | null = null;
    try { o = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(c);
    if (o?.t === "reply" && o.id !== undefined) replies[o.id] = o;
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => { ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))); return id; };
  const turns = (th: string) => seen.filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => String(x.state));
  const said = (th: string) => seen.filter((x) => x.$ === "MessagePosted" && x.thread === th).map((x) => String(x.text ?? "")).join("\n");
  const asks = () => seen.filter((x) => x.$ === "AskOpened");
  const lines = () => (existsSync(log) ? readFileSync(log, "utf8").split("\n").filter(Boolean) : []);
  const file = join(proj, "note.txt");

  // thread
  send("project.add", { path: proj });
  const pc = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated" && x.root === proj));
  send("thread.create", { project: pc?.id, title: "omp", env: "local", provider: "omp", model: "anthropic/claude-haiku-5-5" });
  const made = await until(10000, () => seen.find((x) => x.$ === "ThreadCreated" && x.provider === "omp"));
  const th = String(made?.id ?? "");
  check(!!made, `thread: created with provider omp (${th})`);
  send("thread.modes", { thread: th, runtime: "approval-required" });
  await sleep(300);

  // stream
  const mark = lines().length;
  send("turn.start", { thread: th, text: "hello there", msg: "m-1" });
  await until(20000, () => ["completed", "failed"].includes(turns(th).at(-1) ?? "") ? true : undefined);
  const t1 = lines().slice(mark);
  check(turns(th).at(-1) === "completed", `stream: the turn completes (${turns(th).join(",")})`);
  check(said(th).includes("hello there") && said(th).includes("ECHO: "), "stream: the reply reaches the thread");
  check(t1.some((l) => l === "REQ session/prompt"), "stream: the prompt went to omp");

  // supervised, Allow
  const a0 = asks().length;
  send("turn.start", { thread: th, text: "write please", msg: "m-2" });
  const ask = await until(20000, () => asks()[a0]);
  check(!!ask && JSON.stringify(ask).includes("note.txt"), `supervised: a write shows an approval card (${String(ask?.tool ?? "")})`);
  check(!!ask && ask.options === "Allow\nDeny", "supervised: card offers Allow and Deny only");
  check(!existsSync(file), "supervised: nothing is written before the answer");
  if (ask) send("ask.answer", { id: ask.id, answer: "Allow" });
  await until(20000, () => turns(th).filter((s) => s === "completed").length >= 2 ? true : undefined);
  check(existsSync(file) && readFileSync(file, "utf8") === "hello", "allow: Allow creates the file");
  check(lines().some((l) => l === "ANSWER {\"action\":\"accept\",\"content\":{\"value\":\"Approve\"}}"), "allow: the form reply is Approve");
  check(turns(th).at(-1) === "completed", `allow: the turn completes (${turns(th).join(",")})`);

  // supervised, Deny
  rmSync(file, { force: true });
  const a1 = asks().length;
  send("turn.start", { thread: th, text: "write again", msg: "m-3" });
  const ask2 = await until(20000, () => asks()[a1]);
  check(!!ask2, "deny: a second write asks again");
  if (ask2) send("ask.answer", { id: ask2.id, answer: "Deny" });
  await until(20000, () => turns(th).filter((s) => s === "completed").length >= 3 ? true : undefined);
  check(!existsSync(file), "deny: Deny leaves no file");
  check(turns(th).filter((s) => s === "completed").length >= 3, "deny: the turn still completes");
  check(lines().includes('ANSWER {"action":"decline"}'), "deny: the form is declined");

  // a one-time OMP permission must not select allow_always when allow_once is absent
  const a2 = asks().length;
  send("turn.start", { thread: th, text: "write ACP permission", msg: "m-4" });
  const ask3 = await until(20000, () => asks()[a2]);
  check(!!ask3 && ask3.options === "Allow\nDeny", "permission: the ACP request uses the one-time Allow/Deny card");
  check(!existsSync(file), "permission: no write happens before the answer");
  if (ask3) send("ask.answer", { id: ask3.id, answer: "Allow" });
  await until(20000, () => turns(th).filter((s) => s === "completed").length >= 4 ? true : undefined);
  check(!existsSync(file), "permission: Allow cannot select allow_always");
  check(lines().includes('PERMISSION {"outcome":{"outcome":"cancelled"}}'), "permission: the request is cancelled without allow_once");
  check(turns(th).at(-1) === "completed", `permission: the turn completes (${turns(th).join(",")})`);



  // full access
  send("thread.modes", { thread: th, runtime: "full-access" });
  await sleep(300);
  rmSync(file, { force: true });
  const a3 = asks().length;
  send("turn.start", { thread: th, text: "write yolo", msg: "m-5" });
  await until(20000, () => turns(th).filter((s) => s === "completed").length >= 5 ? true : undefined);
  check(asks().length === a3, "full access: no approval card");
  check(existsSync(file) && readFileSync(file, "utf8") === "hello", "full access: the file is written");
} finally {
  proc.kill();
  await proc.exited;
}

rmSync(root, { recursive: true, force: true });
console.log(fail ? `${fail} FAILED` : "all ok");
process.exit(fail ? 1 : 0);
