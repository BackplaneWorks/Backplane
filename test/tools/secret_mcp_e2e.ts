// The secret box's MCP tools through a real hub's gates (docs/secrets.md).
// A stand-in claude writes down its arguments and remains alive behind a bounded release gate;
// the driver calls the captured MCP address while that generation is live,
// then releases the process before checking the next generation.
//   A. a hub on a copied log (BACKPLANE_COPY=1): secret_run is refused and
//      runs nothing; secret_list still answers (names only).
//   B. an ordinary hub: secret_run runs, its output masked; after the
//      thread's next turn (a new process, a new generation) the old
//      process's secret_run is refused as stale (the gate before routing).
//
//   bun test/tools/secret_mcp_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const VALUE = "SECRET-FIXTURE-MCP-1";
const BUL = "•";

const root = mkdtempSync(join(tmpdir(), "bp-secret-mcp-"));
const fake = join(root, "bin");
const argvLog = join(root, "claude.argv");
const ran = join(root, "ran");
const pidLog = join(root, "agent.pid");
const release = () => writeFileSync(join(root, `release-${readFileSync(pidLog, "utf8").trim()}`), "release\n");
mkdirSync(fake);
writeFileSync(join(fake, "claude"), `#!/bin/sh
agent=0
for arg in "$@"; do
  case "$arg" in --mcp-config) agent=1;; esac
done
if [ "$agent" = 0 ]; then printf '%s\\n' 'fixture-version'; exit 0; fi
printf '%s\\n' "$$" > ${pidLog}
printf '%s\\n' "$@" >> ${argvLog}
printf '%s\\n' '---' >> ${argvLog}
cat > /dev/null &
i=0
while [ "$i" -lt 600 ] && [ ! -f "${root}/release-$$" ]; do
  sleep 0.05
  i=$((i+1))
done
exit 0
`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(50);
  }
};
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}
const urls = () => [...(existsSync(argvLog) ? readFileSync(argvLog, "utf8") : "").matchAll(/http:\/\/127\.0\.0\.1:\d+\/mcp\/[^"'\s\\]+/g)].map((m) => m[0]);
let rid = 0;
const call = async (url: string, name: string, args: object) => {
  const r = await fetch(url, { method: "POST", headers: { "content-type": "application/json", accept: "application/json, text/event-stream" },
    body: JSON.stringify({ jsonrpc: "2.0", id: ++rid, method: "tools/call", params: { name, arguments: args } }) });
  const o: any = JSON.parse(await r.text());
  const text = String(o?.result?.content?.[0]?.text ?? o?.error?.message ?? "");
  return { isError: o?.result?.isError === true || !!o?.error, text };
};

const hub = async (copy: boolean) => {
  const home = join(root, copy ? "home-copy" : "home");
  const proj = join(root, copy ? "proj-copy" : "proj");
  mkdirSync(home);
  mkdirSync(proj);
  writeFileSync(join(home, "events.jsonl"), "");
  const port = freePort();
  const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
      BACKPLANE_VAULT_KEYRING: "file", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, ...(copy ? { BACKPLANE_COPY: "1" } : {}) },
    stdout: "ignore",
    stderr: "ignore",
  });
  for (let i = 0; i < 200; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
  const seen: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    if (o?.t === "reply") seen.push({ $: "reply", ...o });
    for (const c of o?.items ?? []) seen.push(c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => { ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))); return id; };
  send("project.add", { path: proj });
  const pc: any = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated"));
  const sid = send("secret.set", { name: "fixture-mcp", value: VALUE, purpose: "e2e" });
  await until(10000, () => seen.find((x) => x.$ === "reply" && x.id === sid));
  await sleep(1200);
  send("thread.create", { project: pc?.id, title: "mcp one", env: "local", provider: "claude" });
  const th: any = await until(10000, () => seen.find((x) => x.$ === "ThreadCreated" && x.title === "mcp one"));
  return { proc, ws, seen, send, th, home };
};
const turn = async (h: any, n: number) => {
  const before = urls().length;
  h.send("turn.start", { thread: h.th?.id, text: `turn ${n}`, msg: `m-${n}-${Date.now()}` });
  const u = await until(15000, () => urls().length > before ? urls().at(-1) : undefined);
  return u;
};
const stop = async (h: any) => { try { h.ws.close(); } catch {} process.kill(h.proc.pid, "SIGKILL"); await h.proc.exited; };

try {
  // A. a copied log
  let h = await hub(true);
  const ua = await turn(h, 1);
  check(!!ua, `the copy hub started the stand-in with its MCP address`);
  if (ua) {
    const r = await call(ua, "secret_run", { command: `touch ${ran}; echo "$X"`, env: { X: "fixture-mcp" } });
    check(r.isError && /copied log/.test(r.text), `copy mode refuses secret_run (${r.text.slice(0, 100)})`);
    await sleep(500);
    check(!existsSync(ran), "and ran nothing");
    const l = await call(ua, "secret_list", {});
    check(!l.isError && l.text.includes("fixture-mcp") && !l.text.includes(VALUE), "secret_list still answers, names only");
  }
  await stop(h);

  // B. an ordinary hub: it runs, masked; an old generation is refused
  h = await hub(false);
  const u1 = await turn(h, 1);
  check(!!u1, "the hub started the stand-in with its MCP address");
  if (u1) {
    const r = await call(u1, "secret_run", { command: `touch ${ran}; echo "value: $X"`, env: { X: "fixture-mcp" } });
    check(!r.isError && r.text.includes('"code":0') && r.text.includes(BUL) && !r.text.includes(VALUE), `secret_run runs, its output masked (${r.text.slice(0, 120)})`);
    check(existsSync(ran), "and ran");
    rmSync(ran, { force: true });
    const mark = h.seen.length;
    release();
    const ended = await until(5000, () => h.seen.slice(mark).find((x: any) => x.$ === "TurnChanged" && x.thread === h.th.id && x.state?.$ !== "Running"));
    if (!ended) throw new Error("released stand-in did not finish its turn");
    const u2 = await turn(h, 2);
    check(!!u2 && u2 !== u1, "the next turn is a new process with a new address");
    const s = await call(u1, "secret_run", { command: `touch ${ran}`, env: { X: "fixture-mcp" } });
    check(s.isError && /stale process/.test(s.text), `the old process's secret_run is refused as stale (${s.text.slice(0, 100)})`);
    await sleep(500);
    check(!existsSync(ran), "and ran nothing");
  }
  await stop(h);
} finally {
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
