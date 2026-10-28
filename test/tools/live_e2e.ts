// Live text across reconnects, end to end (core/live.bend, server.bend's
// B.Live out and Boot.last). A headless hub with the stand-in `claude` in
// mode stream (test/tools/claude_standin.ts); an observer stays connected
// throughout, and clients come and go mid-stream:
//   reconnect    a client that left mid-reply and joins again shows the
//                reply so far exactly (no repeat, no gap), and with the
//                deltas after it, the reply whole
//   completed    a reply that ended while a client was away: no live text
//                for it, the message in the log
//   replaced     a client back during a second reply shows only that one
//
//   bun test/tools/live_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-live-"));
const fake = join(root, "bin");
mkdirSync(fake);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) {
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
    await sleep(30);
  }
};
let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};
const full = (tag: string, n = 40) => Array.from({ length: n }, (_, i) => `${tag}${i} `).join("");

// a client as the shared Ui keeps live text: a snapshot replaces, a delta
// appends, a posted reply or an ended turn clears
type Cl = { ws: WebSocket; live: Map<string, string>; snaps: any[]; changes: any[]; posted: Map<string, string[]>; atPost: Map<string, string[]>; deltas: number };
const port = freePort();
const connect = async (q = ""): Promise<Cl> => {
  const c: Cl = { ws: new WebSocket(`ws://127.0.0.1:${port}/ws${q}`), live: new Map(), snaps: [], changes: [], posted: new Map(), atPost: new Map(), deltas: 0 };
  c.ws.binaryType = "arraybuffer";
  c.ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    if (!o) return;
    if (o.t === "live") {
      c.snaps.push(o);
      const next = new Map<string, string>();
      for (const it of o.items ?? []) next.set(it.thread, (it.trunc ? "…" : "") + it.text);
      c.live = next;
    } else if (o.t === "delta") {
      c.deltas++;
      c.live.set(o.thread, (c.live.get(o.thread) ?? "") + o.text);
    }
    for (const ch of o.items ?? []) {
      if (o.t === "live") break;
      c.changes.push(ch);
      if (ch.$ === "MessagePosted" && String(ch.role).toLowerCase() === "assistant") {
        c.atPost.set(ch.thread, [...(c.atPost.get(ch.thread) ?? []), c.live.get(ch.thread) ?? ""]);
        c.posted.set(ch.thread, [...(c.posted.get(ch.thread) ?? []), ch.text]);
        c.live.set(ch.thread, "");
      }
      if (ch.$ === "TurnChanged" && ["completed", "failed", "interrupted"].includes(String(ch.state).toLowerCase())) c.live.set(ch.thread, "");
    }
  };
  await new Promise((r, j) => { c.ws.onopen = r; c.ws.onerror = j; });
  return c;
};
const close = async (c: Cl) => {
  c.ws.close();
  await sleep(100);
};

const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
    BP_STANDIN_MODE: "stream", BP_STREAM_N: "40", BP_STREAM_MS: "150", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: "ignore",
});
try {
  for (let i = 0; i < 200; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  const obs = await connect();
  let id = 0;
  const send = (m: string, p: object) => obs.ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  send("project.add", { path: proj });
  const pc = await until(10000, () => obs.changes.find((c) => c.$ === "ProjectCreated"));
  if (!pc) throw new Error("no project");
  send("setting.set", { key: "restart.continue", value: "off" });
  send("thread.create", { project: pc.id, title: "live", env: "local", provider: "claude" });
  const th = (await until(10000, () => obs.changes.find((c) => c.$ === "ThreadCreated")))?.id as string;
  if (!th) throw new Error("no thread");
  const posts = () => obs.posted.get(th) ?? [];

  // reconnect
  const a = await connect();
  await until(5000, () => a.snaps.length ? true : undefined);
  send("turn.start", { thread: th, text: "go A", msg: "l-1" });
  await until(10000, () => (a.live.get(th) ?? "").length > 12 ? true : undefined);
  const had = a.live.get(th) ?? "";
  await close(a);
  await sleep(900);
  const b = await connect();
  await until(5000, () => b.snaps.length ? true : undefined);
  const snap = b.live.get(th) ?? "";
  check(full("A").startsWith(snap) && snap.length > had.length, `reconnect: the snapshot is the reply so far (${had.length} then ${snap.length} chars)`);
  check(b.snaps.length === 1, `reconnect: one snapshot (${b.snaps.length})`);
  await until(15000, () => b.posted.get(th)?.length ? true : undefined);
  check((b.atPost.get(th) ?? [])[0] === full("A"), `reconnect: snapshot + deltas = the reply whole (${JSON.stringify((b.atPost.get(th) ?? [])[0]?.slice(-30))})`);
  check((obs.atPost.get(th) ?? [])[0] === full("A"), "reconnect: the observer's own text whole");
  await until(5000, () => /completed/i.test(String(obs.changes.filter((c) => c.$ === "TurnChanged" && c.thread === th).pop()?.state)) ? true : undefined);

  // completed while away
  send("turn.start", { thread: th, text: "go B", msg: "l-2" });
  await until(10000, () => (b.live.get(th) ?? "").length > 6 ? true : undefined);
  await close(b);
  await until(15000, () => posts().length >= 2 ? true : undefined);
  await sleep(300);
  const c = await connect();
  await until(5000, () => c.snaps.length ? true : undefined);
  const items = (c.snaps[0]?.items ?? []).filter((x: any) => x.thread === th);
  check(items.length === 0 && (c.live.get(th) ?? "") === "", `completed: no live text after the reply ended (${JSON.stringify(items).slice(0, 80)})`);
  check(c.changes.some((x) => x.$ === "MessagePosted" && x.thread === th && x.text === full("B")), "completed: the reply is in the log");
  await until(5000, () => /completed/i.test(String(obs.changes.filter((x) => x.$ === "TurnChanged" && x.thread === th).pop()?.state)) ? true : undefined);

  // replaced: two replies back to back, a client back during the second
  send("turn.start", { thread: th, text: "go C", msg: "l-3" });
  await sleep(200);
  send("turn.start", { thread: th, text: "go D", msg: "l-4", mode: "steer" });
  await until(20000, () => posts().length >= 3 && (obs.live.get(th) ?? "").includes("D3 ") ? true : undefined);
  await close(c);
  await sleep(500);
  const d = await connect();
  await until(5000, () => d.snaps.length ? true : undefined);
  const s2 = d.live.get(th) ?? "";
  check(s2.length > 0 && full("D").startsWith(s2) && !s2.includes("C"), `replaced: only the second reply (${JSON.stringify(s2.slice(0, 40))})`);
  await until(20000, () => (d.posted.get(th) ?? []).includes(full("D")) ? true : undefined);
  const iD = (d.posted.get(th) ?? []).lastIndexOf(full("D"));
  check(iD >= 0 && (d.atPost.get(th) ?? [])[iD] === full("D"), `replaced: the second reply whole (${JSON.stringify((d.atPost.get(th) ?? [])[iD]?.slice(-24))})`);
  await close(d);
  await close(obs);
} finally {
  proc.kill(9);
  await proc.exited;
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
