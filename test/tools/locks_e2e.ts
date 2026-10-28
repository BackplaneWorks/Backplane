// A throwaway native hub, stand-in Claude, temporary homes and flock files.
// bun test/tools/locks_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter(f => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
const root = mkdtempSync(join(tmpdir(), "bp-locks-"));
const home = join(root, "home"), proj = join(root, "proj"), fake = join(root, "bin"), log = join(root, "claude.log"), locks = join(root, "locks");
for (const d of [home, proj, fake, locks]) mkdirSync(d);
writeFileSync(join(home, "events.jsonl"), "");
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec '${process.execPath}' '${resolve("test/tools/claude_standin.ts")}' "$@"\n`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
const socket = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
const port = socket.port; socket.stop(true);
if (port === 3787) throw new Error("refusing the live port");
const env = { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_LOCK_DIR: locks, BACKPLANE_LOCK_URL: `http://127.0.0.1:${port}`, BP_WORK_MS: "100", FAKE_LOG: log, BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, TMPDIR: root };
let hub = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground", "--headless"], { env, stdout: "ignore", stderr: Bun.file(join(root, "hub.log")) });
const owned: ReturnType<typeof Bun.spawn>[] = [];
let ws: WebSocket | undefined, fail = 0, seq = 0, crashWorkPid = 0;
const seen: any[] = [];
const check = (ok: boolean, text: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${text}`); if (!ok) fail++; };
const until = async <T>(ms: number, f: () => T | undefined, label = "condition"): Promise<T> => {
  const end = Date.now() + ms;
  for (;;) { const v = f(); if (v) return v; if (Date.now() > end) throw new Error(`timed out: ${label}`); await sleep(50); }
};
const content = (path: string) => existsSync(path) ? readFileSync(path, "utf8") : "";
const spawn = (args: string[]) => { const p = Bun.spawn(args, { env, stdout: "ignore", stderr: "inherit" }); owned.push(p); return p; };
const post = async (op: string, name: string, holder: string) => {
  const r = await fetch(`${env.BACKPLANE_LOCK_URL}/lock/${op}`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ name, holder, note: "e2e" }) });
  if (!r.ok) throw new Error(`${op}: ${r.status}`); return await r.json() as any;
};
const waitHub = async () => {
  for (let i = 0; i < 200; i++) { try { if ((await fetch(`${env.BACKPLANE_LOCK_URL}/hello`)).status === 200) return; } catch {} await sleep(100); }
  throw new Error("hub did not start");
};
const connect = async () => {
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`); ws.binaryType = "arraybuffer";
  ws.onmessage = e => { const o = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data))); seen.push(...(o.items ?? [])); };
  await new Promise((r, j) => { ws!.onopen = r; ws!.onerror = j; });
};
try {
  await waitHub(); await connect();
  const send = (m: string, p: object) => ws!.send(W.encode(JSON.stringify({ id: ++seq, m, p })));
  send("project.add", { path: proj });
  const pc = await until(10000, () => seen.find(x => x.$ === "ProjectCreated" && x.root === proj));
  const made = async (title: string) => { send("thread.create", { project: pc.id, title, env: "local", provider: "claude" }); return (await until(10000, () => seen.find(x => x.$ === "ThreadCreated" && x.title === title))).id as string; };
  const a = await made("Lock A"), b = await made("Lock B");
  const turns = (th: string) => seen.filter(x => x.$ === "TurnChanged" && x.thread === th && x.state === "completed").length;
  for (const th of [a, b]) { const n = turns(th); send("turn.start", { thread: th, text: "mode:plain ready", msg: `test-${++seq}` }); await until(15000, () => turns(th) > n ? true : undefined); }
  const call = async (th: string, name: string, args: object) => {
    const url = await until(10000, () => content(log).split("\n").filter(l => l.startsWith("MCP ") && (l.includes(`/mcp/${th}?`) || l.includes(`/mcp/${th}.`))).at(-1)?.split(" ").slice(2).join(" "));
    const r = await fetch(url, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: ++seq, method: "tools/call", params: { name, arguments: args } }) });
    const o: any = await r.json(); if (o.result?.isError || o.error) throw new Error(JSON.stringify(o));
    return JSON.parse(o.result.content[0].text);
  };
  const wakes = () => seen.filter(x => x.$ === "MessagePosted" && x.thread === b && String(x.text).startsWith("Lock bend is yours"));
  check((await call(a, "lock_acquire", { name: "bend", note: "compile" })).held === true, "A acquires immediately with OS guard");
  const queued = await call(b, "lock_acquire", { name: "bend", note: "next" });
  check(!queued.held && queued.position === 1 && queued.holder === a, "B queues without blocking the tool");
  const before = seen.filter(x => x.$ === "LockChanged").length;
  check((await call(b, "lock_acquire", { name: "bend" })).position === 1, "duplicate acquire preserves queue position");
  await sleep(200); check(seen.filter(x => x.$ === "LockChanged").length === before, "duplicate acquisition appends no event");
  await call(b, "lock_release", { name: "bend" });
  check((await call(a, "lock_list", {})).locks.find((l: any) => l.name === "bend").holder.id === a, "non-holder release changes nothing");
  await call(a, "lock_release", { name: "bend" });
  await until(15000, () => wakes().length ? true : undefined, "queued grant message");
  await sleep(1200); check(wakes().length === 1, "B is woken exactly once on grant");
  check(String(wakes()[0]?.text).includes("Release it with lock_release"), "grant message explains release");
  const marker = join(root, "work-ran");
  const work = await call(a, "work_run", { command: `touch '${marker}'`, note: "locked work", lock: "bend" });
  await sleep(300); check(!existsSync(marker), "work_run does not launch before its grant");
  check((await call(a, "work_list", {})).work.some((w: any) => w.id === work.id && w.state.startsWith("waiting for lock bend")), "work row shows lock queue position");
  const grantCount = seen.filter(x => x.$ === "MessagePosted" && x.thread === a && String(x.text).startsWith("Lock bend is yours")).length;
  await call(b, "lock_release", { name: "bend" });
  await until(15000, () => existsSync(marker) ? true : undefined, "work launch");
  await until(15000, () => seen.some(x => x.$ === "MessagePosted" && x.thread === a && String(x.text).includes('Watched work "locked work" ended: exit 0')) ? true : undefined, "work end wake");
  check(seen.filter(x => x.$ === "MessagePosted" && x.thread === a && String(x.text).startsWith("Lock bend is yours")).length === grantCount, "work grant wakes only on work end");
  check((await call(a, "lock_list", {})).locks.find((l: any) => l.name === "bend").holder === null, "work end releases lock");
  const timedMarker = join(root, "timed-work"), timedRaw = join(root, "timed-raw");
  const timed = await call(a, "work_run", { command: `sleep 6; touch '${timedMarker}'`, lock: "timeout", note: "timed work", timeoutSec: 1 });
  await until(15000, () => seen.some(x => x.$ === "MessagePosted" && x.thread === a && String(x.text).includes("lock retained")) ? true : undefined, "locked timeout notice");
  check((await call(a, "lock_list", {})).locks.find((l: any) => l.name === "timeout").holder?.id === timed.id, "timeout retains a live command's lock");
  const timedFlock = spawn(["flock", join(locks, "bp-timeout.lock"), "sh", "-c", `touch '${timedRaw}'`]);
  await sleep(300); check(!existsSync(timedRaw), "raw flock stays excluded after a work timeout");
  await until(15000, () => existsSync(timedMarker) && existsSync(timedRaw) ? true : undefined, "timed command ends and releases");
  check(await timedFlock.exited === 0, "timed work ultimately releases its OS guard");
  const cancelMarker = join(root, "must-not-run");
  await call(b, "lock_acquire", { name: "bend" });
  const cancelled = await call(a, "work_run", { command: `touch '${cancelMarker}'`, lock: "bend" });
  await call(a, "work_cancel", { id: cancelled.id }); await call(b, "lock_release", { name: "bend" });
  await sleep(800); check(!existsSync(cancelMarker), "cancelled queued work never launches");
  const serial = join(root, "serial");
  const one = spawn([resolve("scripts/bp-lock.sh"), "scripts", "--", "sh", "-c", `echo one-start >> '${serial}'; sleep 1; echo one-end >> '${serial}'`]);
  await until(10000, () => content(serial).includes("one-start") ? true : undefined);
  const two = spawn([resolve("scripts/bp-lock.sh"), "scripts", "--", "sh", "-c", `echo two-start >> '${serial}'; sleep 1; echo two-end >> '${serial}'`]);
  check(await one.exited === 0 && await two.exited === 0, "two bp-lock helpers exit successfully");
  check(content(serial).trim() === "one-start\none-end\ntwo-start\ntwo-end", "two bp-lock helpers serialize through hub FIFO");
  await post("acquire", "interop", "pid:test@e2e");
  const rawMarker = join(root, "raw");
  const raw = spawn(["flock", join(locks, "bp-interop.lock"), "sh", "-c", `touch '${rawMarker}'`]);
  await sleep(400); check(!existsSync(rawMarker), "hub guard excludes raw flock");
  await post("release", "interop", "pid:test@e2e");
  check(await raw.exited === 0 && existsSync(rawMarker), "raw flock proceeds after hub release");
  const rawHeld = join(root, "raw-held");
  const firstRaw = spawn(["flock", join(locks, "bp-reverse.lock"), "sh", "-c", `touch '${rawHeld}'; sleep 2`]);
  await until(10000, () => existsSync(rawHeld) ? true : undefined);
  check(!(await post("acquire", "reverse", "pid:reverse@e2e")).held, "hub queues behind an existing raw flock");
  await firstRaw.exited;
  await sleep(500);
  check((await post("status", "reverse", "pid:reverse@e2e")).held, "OS grant becomes visible when raw flock ends");
  await post("release", "reverse", "pid:reverse@e2e");
  // The record loses a waiting helper: its next status queues it again, visibly, and it is granted in turn.
  await post("acquire", "lost", "pid:front@e2e");
  const lostErr = join(root, "lost.err"), lostMarker = join(root, "lost-ran");
  const lostHelper = Bun.spawn([resolve("scripts/bp-lock.sh"), "lost", "--", "sh", "-c", `touch '${lostMarker}'`], { env, stdout: "ignore", stderr: Bun.file(lostErr) });
  owned.push(lostHelper);
  const waiterOf = async () => (await call(a, "lock_list", {})).locks.find((l: any) => l.name === "lost")?.waiters.find((w: any) => w.note === "bp-lock.sh");
  let party: any; for (let i = 0; i < 100 && !party; i++) { party = await waiterOf(); if (!party) await sleep(100); }
  check(!!party, "bp-lock helper waits in the lock list");
  await post("cancel", "lost", party.id);
  check(!(await waiterOf()), "the record dropped the waiting helper");
  for (let i = 0; i < 60 && !(await waiterOf()); i++) await sleep(100);
  check(!!(await waiterOf()), "the helper's next status queues it again (visible in lock_list)");
  const lostRaw = await post("status", "lost", "pid:ghost@e2e");
  check(lostRaw.requeued === true && lostRaw.held === false && lostRaw.position === 2, "status of an unknown client requeues it behind the others");
  check((await post("renew", "lost", "pid:ghost2@e2e")).requeued === true, "renew of an unknown client requeues it");
  check((await post("status", "lost", "pid:ghost@e2e")).requeued !== true, "a queued client is not requeued again");
  await post("cancel", "lost", "pid:ghost@e2e"); await post("cancel", "lost", "pid:ghost2@e2e");
  await post("release", "lost", "pid:front@e2e");
  check(await lostHelper.exited === 0 && existsSync(lostMarker), "the requeued helper is granted in turn");
  // A raw flock outside the hub holds the file: the lock list names its pid and command.
  const extHeld = join(root, "ext-held");
  const ext = spawn(["flock", join(locks, "bp-ext.lock"), "sh", "-c", `touch '${extHeld}'; sleep 4`]);
  await until(10000, () => existsSync(extHeld) ? true : undefined, "outside holder starts");
  check(!(await post("acquire", "ext", "pid:ext@e2e")).held, "hub queues behind an outside flock holder");
  let outside: any; for (let i = 0; i < 100 && !outside; i++) { outside = (await call(a, "lock_list", {})).locks.find((l: any) => l.name === "ext")?.outside; if (!outside) await sleep(100); }
  check(outside?.pid === ext.pid && String(outside?.cmd).includes("sleep 4"), `lock_list shows the outside holder (${JSON.stringify(outside)})`);
  await ext.exited;
  for (let i = 0; i < 100 && !(await post("status", "ext", "pid:ext@e2e")).held; i++) await sleep(100);
  check((await post("status", "ext", "pid:ext@e2e")).held === true, "the queue proceeds when the outside holder exits");
  check((await call(a, "lock_list", {})).locks.find((l: any) => l.name === "ext")?.outside === undefined, "the outside holder is cleared once the guard is taken");
  await post("release", "ext", "pid:ext@e2e");
  // SIGKILL has no cleanup trap: a stopped renewal must free the lock in 90s.
  const childFile = join(root, "lease-child.pid");
  const killed = spawn([resolve("scripts/bp-lock.sh"), "lease", "--", "sh", "-c", `echo $$ > '${childFile}'; exec sleep 180`]);
  await until(10000, () => existsSync(childFile) ? true : undefined, "lease holder starts");
  const childPid = Number(content(childFile).trim());
  process.kill(killed.pid, "SIGKILL"); await killed.exited;
  process.kill(childPid, "SIGTERM");
  const expiry = join(root, "after-expiry");
  const started = Date.now();
  const next = spawn([resolve("scripts/bp-lock.sh"), "lease", "--", "sh", "-c", `touch '${expiry}'`]);
  await until(120000, () => existsSync(expiry) ? true : undefined, "killed helper lease expires");
  check(await next.exited === 0 && Date.now() - started >= 85000, `killed helper frees lock after lease (${Math.round((Date.now() - started) / 1000)}s)`);
  // A helper waiting while the hub restarts survives it (see the restart below).
  await post("acquire", "survive", "pid:holder@e2e");
  const surviveMarker = join(root, "survive-ran");
  const survivor = Bun.spawn([resolve("scripts/bp-lock.sh"), "survive", "--", "sh", "-c", `touch '${surviveMarker}'`], { env, stdout: "ignore", stderr: "inherit" });
  owned.push(survivor);
  await sleep(28000); // past the first long-poll acquire: the helper is in its own status loop
  // A restart loses queued locked work; it must not become an unguarded launch.
  await call(b, "lock_acquire", { name: "restart" });
  const restartMarker = join(root, "restart-must-not-run");
  await call(a, "work_run", { command: `touch '${restartMarker}'`, lock: "restart" });
  ws?.close(); process.kill(hub.pid, "SIGTERM"); await hub.exited;
  hub = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground", "--headless"], { env, stdout: "ignore", stderr: Bun.file(join(root, "hub-restart.log")) });
  await waitHub(); await sleep(1000);
  check(!existsSync(restartMarker), "restart never starts queued locked work");
  check(!existsSync(surviveMarker), "a helper waiting through the restart has not run");
  check(survivor.exitCode === null, `a helper waiting through the restart is still alive (exit ${survivor.exitCode})`);
  await post("release", "survive", "pid:holder@e2e");
  check(await survivor.exited === 0 && existsSync(surviveMarker), "the helper that waited through the restart is granted");
  check(content(join(home, "events.jsonl")).includes("lost on restart"), "restart records and reports lost locked work");
  const crashPidFile = join(root, "crash-work.pid"), crashRaw = join(root, "crash-raw");
  await call(a, "work_run", { command: `echo $$ > '${crashPidFile}'; exec sleep 180`, lock: "crash-work" });
  await until(10000, () => existsSync(crashPidFile) ? true : undefined, "crash work starts");
  crashWorkPid = Number(content(crashPidFile).trim());
  const afterWorkHub = spawn(["flock", join(locks, "bp-crash-work.lock"), "sh", "-c", `touch '${crashRaw}'`]);
  await sleep(300); check(!existsSync(crashRaw), "running owned work keeps its OS guard");
  const hubGone = join(root, "hub-gone");
  const afterHub = spawn(["flock", join(locks, "bp-restart.lock"), "sh", "-c", `touch '${hubGone}'`]);
  await sleep(300); check(!existsSync(hubGone), "restart restores the retained thread's OS guard");
  process.kill(hub.pid, "SIGKILL"); await hub.exited;
  await until(10000, () => existsSync(hubGone) ? true : undefined, "hub death closes guard pipe");
  check(await afterHub.exited === 0, "captured OS guard dies with a killed hub");
  await until(10000, () => existsSync(crashRaw) ? true : undefined, "crash guard stops the detached work");
  check(await afterWorkHub.exited === 0, "work guard frees only after crash cleanup");
  const alive = (pid: number) => { try { process.kill(pid, 0); return true; } catch { return false; } };
  await until(5000, () => !alive(crashWorkPid) ? true : undefined, "owned command terminated");
  check(!alive(crashWorkPid), "hub SIGKILL stops detached locked work by its captured PID");

  const fallback = spawn([resolve("scripts/bp-lock.sh"), "fallback", "--", "sh", "-c", "exit 7"]);
  check(await fallback.exited === 7, "no-hub fallback preserves command exit status");

} finally {
  if (crashWorkPid) { try { process.kill(crashWorkPid, "SIGTERM"); } catch {} }
  ws?.close();
  for (const p of owned) if (p.exitCode === null) { try { process.kill(p.pid, "SIGTERM"); } catch {} await p.exited; }
  if (hub.exitCode === null) { try { process.kill(hub.pid, "SIGTERM"); } catch (e: any) { if (e.code !== "ESRCH") throw e; } await hub.exited; }
  if (process.env.KEEP) console.log(`artifacts ${root}`); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
