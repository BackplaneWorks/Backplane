// The secret box end to end, at the provider boundaries (docs/secrets.md).
// A headless hub on a throwaway home (file key) with stand-in claude and
// codex that write down every argument and everything on their stdin, and
// print the stored value back as an agent would:
//   1. secret.set stores SECRET-FIXTURE-E2E-1; the reply carries no value.
//   2. the person pastes it into a Claude thread and a Codex thread: the
//      stand-ins' argv and stdin never hold it (Spawn.agent, Agent.write),
//      and the hub's log, store and frames never hold it (Hub.step.go,
//      Hub.tagged), though the stand-in printed it.
//   3. a reconnect's replay holds it nowhere either.
//   4. untrusted probe output: each stand-in's --version prints the value.
//      It was probed at start, before the value was stored (late-known):
//      no info frame, no replay, and after secret.set no models.cache holds
//      it. After a restart on the same home (now known at the probe, the
//      cache read before the vault opens), the info, the replay and the
//      cache still do not.
//   5. an old unmasked models.cache written before boot: masked before
//      /hello answers, and the first join's info carries it masked.
//
//   bun test/tools/secret_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const VALUE = "SECRET-FIXTURE-E2E-1";

const root = mkdtempSync(join(tmpdir(), "bp-secret-e2e-"));
const fake = join(root, "bin");
const cap = join(root, "captured");
mkdirSync(fake);
mkdirSync(cap);
// each stand-in: its argv and stdin to files; then the value on stdout in
// the provider's shape, and an end
const standin = (name: string, line: string) =>
  `#!/bin/sh\nif [ "$1" = "--version" ]; then echo "9.9.9 (build ${VALUE})"; exit 0; fi\n` +
  `printf '%s\\n' "$@" >> ${cap}/${name}.argv\n` +
  `( cat >> ${cap}/${name}.stdin ) &\n` +
  `printf '%s\\n' '${line}'\nsleep 2\nexit 0\n`;
writeFileSync(join(fake, "claude"), standin("claude", JSON.stringify({ type: "assistant", message: { role: "assistant", content: [{ type: "text", text: `the key is ${VALUE}` }] } })));
writeFileSync(join(fake, "codex"), standin("codex", JSON.stringify({ type: "item.completed", item: { id: "i1", type: "agent_message", text: `the key is ${VALUE}` } })));
for (const n of ["claude", "codex"]) chmodSync(join(fake, n), 0o755);
for (const n of ["grok", "muse"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
const home = join(root, "home");
const proj = join(root, "proj");
mkdirSync(proj);
mkdirSync(home);
writeFileSync(join(home, "events.jsonl"), "");

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
let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};
const port = freePort();
const spawnHub = () => Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
    BACKPLANE_VAULT_KEYRING: "file", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: "ignore",
});
let proc = spawnHub();
const ready = async () => {
  for (let i = 0; i < 200; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
};
type Client = { ws: WebSocket; seen: any[]; frames: string[]; send: (m: string, p: object) => number };
const connect = async (): Promise<Client> => {
  const seen: any[] = [], frames: string[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let text = "";
    try { text = typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer)); } catch {}
    frames.push(text);
    let o: any = null;
    try { o = JSON.parse(text); } catch {}
    if (o?.t === "reply") seen.push({ $: "reply", ...o });
    for (const c of o?.items ?? []) seen.push(c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  return { ws, seen, frames, send: (m, p) => { ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))); return id; } };
};
const read = (f: string) => existsSync(f) ? readFileSync(f).toString("latin1") : "";
try {
  await ready();
  let c = await connect();
  c.send("project.add", { path: proj });
  const pc: any = await until(10000, () => c.seen.find((x) => x.$ === "ProjectCreated"));
  check(!!pc, "a project");

  // 1. the value stored
  const sid = c.send("secret.set", { name: "fixture-e2e", value: VALUE, purpose: "e2e" });
  const rep: any = await until(10000, () => c.seen.find((x) => x.$ === "reply" && x.id === sid));
  check(!!rep && rep.ok === true && String(rep.secrets).includes("fixture-e2e"), "secret.set is answered with the vault's names");
  check(!JSON.stringify(rep ?? {}).includes(VALUE), "the reply holds no value");
  const vault = read(join(home, "secrets", "vault.json"));
  check(vault.includes("fixture-e2e") && !vault.includes(VALUE), "the vault holds it sealed");
  // 4a. probed before it was known: the save masked the cache again
  await sleep(500);
  const cache1 = read(join(home, "models.cache"));
  check(!cache1.includes(VALUE), `after secret.set the models cache does not hold it (${cache1.length} bytes)`);

  // 2. pasted into a Claude and a Codex thread
  for (const [title, provider] of [["claude one", "claude"], ["codex one", "codex"]]) {
    await sleep(1200);
    c.send("thread.create", { project: pc.id, title, env: "local", provider });
    const th: any = await until(10000, () => c.seen.find((x) => x.$ === "ThreadCreated" && x.title === title));
    check(!!th, `a ${provider} thread`);
    c.send("turn.start", { thread: th.id, text: `please use ${VALUE} to log in`, msg: `m-${provider}` });
    await until(15000, () => existsSync(join(cap, `${provider}.argv`)) ? true : undefined);
    await sleep(3500);
    const argv = read(join(cap, `${provider}.argv`)), stdin = read(join(cap, `${provider}.stdin`));
    check(argv.length > 0, `${provider} started (${argv.length} bytes of argv)`);
    check(!argv.includes(VALUE), `${provider}'s arguments never hold the value`);
    check(!stdin.includes(VALUE), `${provider}'s stdin never holds the value`);
    const posted = c.seen.find((x) => x.$ === "MessagePosted" && x.thread === th.id);
    check(!!posted && String(posted.text).includes("please use") && String(posted.text).includes("•"), `the person's message is kept, masked (${String(posted?.text).slice(0, 60)})`);
  }
  check(!c.frames.some((f) => f.includes(VALUE)), "no frame to the client held the value");
  await sleep(1000);
  const log = read(join(home, "events.jsonl"));
  check(log.length > 0 && !log.includes(VALUE), "the event log never holds it");
  const stored = readdirSync(home).filter((f) => f.startsWith("store")).map((f) => read(join(home, f))).join("");
  check(!stored.includes(VALUE), "the store (and its full-text index) never holds it");

  // 3. a reconnect's replay
  c.ws.close();
  c = await connect();
  await sleep(2000);
  check(c.frames.length > 0 && !c.frames.some((f) => f.includes(VALUE)), "a reconnect's replay holds it nowhere");
  check(c.frames.some((f) => f.includes("9.9.9 (build ")), "the replay's info does carry the probe's version, masked");

  // 4b. a restart on the same home: the probe now knows the value; the
  // cache is read before the vault opens
  c.ws.close();
  process.kill(proc.pid, "SIGKILL");
  await proc.exited;
  proc = spawnHub();
  await ready();
  c = await connect();
  await sleep(4000);
  check(c.frames.length > 0 && !c.frames.some((f) => f.includes(VALUE)), "after a restart no info or replay frame holds it");
  c.ws.close();
  c = await connect();
  await sleep(1500);
  check(!c.frames.some((f) => f.includes(VALUE)), "nor a reconnect after the restart's probe");
  check(!read(join(home, "models.cache")).includes(VALUE), "nor the models cache");

  // 5. an old, unmasked models cache on disk (an older hub, or a probe
  // from before the value was stored): masked before the hub serves
  // anything, the very first join included
  c.ws.close();
  process.kill(proc.pid, "SIGKILL");
  await proc.exited;
  writeFileSync(join(home, "models.cache"), JSON.stringify({ about: { claude: { cli: `old (build ${VALUE})` } } }) + "\n" + `old text ${VALUE}\n`);
  proc = spawnHub();
  await ready();
  check(!read(join(home, "models.cache")).includes(VALUE), "an old unmasked cache is masked before the hub answers /hello");
  c = await connect();
  await sleep(300);
  check(c.frames.length > 0 && !c.frames.some((f) => f.includes(VALUE)), "the first join after it holds the value nowhere");
  // (the start's probe may already have replaced the old list)
  check(c.frames.some((f) => f.includes("old (build ") || f.includes("9.9.9 (build ")), "and its info carries the models, masked");
} finally {
  process.kill(proc.pid, "SIGKILL");
  await proc.exited;
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
