// Claude's turns by the command ledger, end to end (core/cmds.bend,
// hub.bend's Hub.agent_line.v, server.bend's Hub.line.run). A headless hub
// with the stand-in `claude` (test/tools/claude_standin.ts), one thread per
// mode; a desk client (desk.look on no thread) hears every notify.
//   wake         the turn ends, a background shell ends after it, and
//                Claude wakes by itself: Running again, one "wake" Act, the
//                wake's result completes it, and a notify comes after it
//   stale-first  a stale notification turn's result while the user's
//                message waits never completes the thread
//   stop         Stop shows Interrupted at once; the message sent right
//                after completes, never Failed
//   pair         two messages back to back: Running until the second's end
//   bg           a background shell outlives two turns: Running at the wake
//   noLC         an old CLI (no lifecycles): the old rule
//
//   bun test/tools/wake_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-wake-"));
const fake = join(root, "bin");
const log = join(root, "claude.log");
mkdirSync(fake);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse", "omp"]) {
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
const lines = () => (existsSync(log) ? readFileSync(log, "utf8").split("\n").filter(Boolean) : []);

let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};

const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log,
    BP_WORK_MS: "1500", BP_BG_MS: "2500", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
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
  // every change and notify, with when it came
  const seen: { c: any; t: number }[] = [];
  const notes: { thread: string; t: number }[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const text = typeof e.data === "string";
    let o: any = null;
    try { o = JSON.parse(text ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    if (o?.t === "notify") notes.push({ thread: o.thread ?? "", t: Date.now() });
    for (const c of o?.items ?? []) seen.push({ c, t: Date.now() });
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  const changes = () => seen.map((x) => x.c);
  const turns = (th: string) => seen.filter((x) => x.c.$ === "TurnChanged" && x.c.thread === th).map((x) => { const v = String(x.c.state); return { s: v.charAt(0).toUpperCase() + v.slice(1), t: x.t }; });
  const states = (th: string) => turns(th).map((x) => x.s[0]).join("");
  const last = (th: string) => { const t = turns(th); return t.length ? t[t.length - 1].s : ""; };
  const acts = (th: string, kind: string) => seen.filter((x) => x.c.$ === "ActivityLogged" && x.c.thread === th && x.c.kind === kind);
  const newThread = async (title: string) => {
    const n = changes().filter((c) => c.$ === "ThreadCreated").length;
    send("thread.create", { project: pc.id, title, env: "local", provider: "claude" });
    const th = await until(10000, () => changes().filter((c) => c.$ === "ThreadCreated")[n]);
    if (!th) throw new Error("no thread");
    return th.id as string;
  };

  send("project.add", { path: proj });
  const pc = await until(10000, () => changes().find((c) => c.$ === "ProjectCreated"));
  if (!pc) throw new Error("no project");
  send("setting.set", { key: "restart.continue", value: "off" });
  // a desk that watches nothing: every turn's alert reaches it
  send("desk.look", { thread: "none", focused: true });
  await sleep(300);

  // wake
  const t1 = await newThread("wake");
  send("turn.start", { thread: t1, text: "mode:wake go", msg: "w-1" });
  await until(15000, () => acts(t1, "wake").length ? true : undefined);
  await until(10000, () => (lines().some((l) => l === "WOKE") && /Completed/.test(last(t1)) && states(t1).endsWith("RC") && acts(t1, "wake").length) ? true : undefined);
  await sleep(500);
  const wk = acts(t1, "wake");
  check(wk.length === 1, `wake: one wake Act (${wk.length}: ${wk[0]?.c.summary})`);
  check(/^R+CR+C$/.test(states(t1)), `wake: Running, Completed, Running at the wake, Completed (${states(t1)})`);
  const wakeAt = wk[0]?.t ?? 0;
  const tt1 = turns(t1);
  const runAtWake = tt1.filter((x) => x.t <= wakeAt + 5).pop()?.s;
  check(runAtWake === "Running", `wake: running when the Act came (${runAtWake})`);
  const n1 = notes.filter((n) => n.thread === t1);
  check(n1.length === 1 && n1[0].t >= wakeAt, `wake: no notify while the shell ran, one after the wake (${n1.length})`);

  // stale-first
  const t2 = await newThread("stale");
  send("turn.start", { thread: t2, text: "mode:stale-first go", msg: "w-2" });
  await until(10000, () => lines().some((l) => l === "STALE") ? true : undefined);
  await sleep(1000);
  check(last(t2) === "Running", `stale-first: still running after the stale result (${states(t2)})`);
  await until(10000, () => /Completed/.test(last(t2)) ? true : undefined);
  check(/^R+C$/.test(states(t2)), `stale-first: completed once, at the command's end (${states(t2)})`);
  const msgs2 = changes().filter((c) => c.$ === "MessagePosted" && c.thread === t2 && c.role !== "User").map((c) => c.text);
  check(msgs2.some((t) => t.includes("REPLY")), `stale-first: the real answer posted (${msgs2.join("|")})`);

  // stop, then a message at once
  const t3 = await newThread("stop");
  send("turn.start", { thread: t3, text: "mode:stop go", msg: "w-3" });
  await until(10000, () => lines().some((l) => l.startsWith("USER") && l.includes("mode:stop")) ? true : undefined);
  await sleep(500);
  send("turn.interrupt", { thread: t3 });
  const intAt = Date.now();
  await until(5000, () => /Interrupted/.test(last(t3)) ? true : undefined);
  check(/Interrupted/.test(last(t3)) && Date.now() - intAt < 3000, `stop: Interrupted at once (${states(t3)})`);
  send("turn.start", { thread: t3, text: "after stop", msg: "w-4" });
  await until(25000, () => /Completed/.test(last(t3)) ? true : undefined);
  check(/^R+IR+C$/.test(states(t3)), `stop: the next message runs and completes (${states(t3)})`);
  check(!turns(t3).some((x) => x.s === "Failed"), "stop: never Failed by the stopped command's error");

  // pair
  const t4 = await newThread("pair");
  send("turn.start", { thread: t4, text: "mode:pair one", msg: "w-5" });
  await until(10000, () => lines().some((l) => l.startsWith("USER") && l.includes("mode:pair")) ? true : undefined);
  send("turn.start", { thread: t4, text: "two", msg: "w-6", mode: "steer" });
  await until(10000, () => changes().some((c) => c.$ === "MessagePosted" && c.thread === t4 && c.text === "ONE") ? true : undefined);
  await sleep(300);
  check(last(t4) === "Running", `pair: running after the first result (${states(t4)})`);
  await until(10000, () => /Completed/.test(last(t4)) ? true : undefined);
  const done4 = turns(t4).find((x) => x.s === "Completed")?.t ?? 0;
  const two = seen.find((x) => x.c.$ === "MessagePosted" && x.c.thread === t4 && x.c.text === "TWO")?.t ?? Infinity;
  check(/^R+C$/.test(states(t4)) && done4 >= two, `pair: completed once, after the second (${states(t4)})`);

  // bg: a background shell outlives this turn and the next
  const t5 = await newThread("bg");
  send("turn.start", { thread: t5, text: "mode:bg start", msg: "w-7" });
  await until(10000, () => /Completed/.test(last(t5)) ? true : undefined);
  send("turn.start", { thread: t5, text: "more", msg: "w-8" });
  await until(15000, () => acts(t5, "wake").length && /Completed/.test(last(t5)) && lines().filter((l) => l === "WOKE").length >= 2 ? true : undefined);
  await sleep(300);
  const wk5 = acts(t5, "wake");
  check(wk5.length === 1, `bg: one wake (${wk5.length})`);
  const at5 = turns(t5).filter((x) => x.t <= (wk5[0]?.t ?? 0) + 5).pop()?.s;
  check(at5 === "Running", `bg: running at the wake (${states(t5)})`);
  check(last(t5) === "Completed", `bg: completed after the wake (${states(t5)})`);
  // its row: at work, then ended but owed (Claude's empty open list), and
  // gone only at the notification that woke it
  const work5 = seen.filter((x) => x.c.$ === "WorkSet" && x.c.thread === t5 && x.c.kind === "bg");
  const kinds5 = work5.map((x) => (x.c.rows === "" ? "-" : x.c.rows.includes("\u0001ended:") ? "E" : "R")).join("");
  check(/^R+E+-$/.test(kinds5), `bg: row at work, ended and owed, then gone (${kinds5})`);
  const ended5 = work5.find((x) => x.c.rows.includes("\u0001ended:"))?.t ?? Infinity;
  const gone5 = work5.filter((x) => x.c.rows === "").pop()?.t ?? 0;
  check(ended5 <= (wk5[0]?.t ?? 0) && gone5 >= (wk5[0]?.t ?? 0), "bg: owed until the wake");
  check(!seen.some((x) => x.c.$ === "SettingSet" && String(x.c.key).startsWith("bg.")), "bg: rows go as WorkSet, never as a setting");

  // an old CLI
  const t6 = await newThread("old");
  send("turn.start", { thread: t6, text: "mode:noLC go", msg: "w-9" });
  await until(10000, () => /Completed/.test(last(t6)) ? true : undefined);
  check(/^R+C$/.test(states(t6)), `noLC: the old rule completes the turn (${states(t6)})`);
} finally {
  proc.kill(9);
  await proc.exited;
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
if (fail) process.exit(1);
// The subscriptions increment adds its own throwaway hub after the existing
// background-work wake regression checks above.
await import("./wake_subscription_e2e.ts");
