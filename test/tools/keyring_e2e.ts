// The login keyring, end to end (core/keyring.bend): the lock state comes
// from BACKPLANE_KEYRING_LOCKED (1 locked, 0 unlocked) instead of the
// Secret Service. Two headless hubs, one each way, with a stand-in `codex`
// (it logs each run, then completes its turn) and the stand-in `claude`
// in mode exit-now (it exits 3 at its first command):
//   locked    the codex turn fails at once saying codex was not started
//             and why, and codex never runs; the claude failure carries
//             the keyring note
//   unlocked  the codex turn runs and completes; the claude failure has
//             no keyring note
//
//   bun test/tools/keyring_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(50); }
};
const freePort = () => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };

async function run(locked: boolean) {
  console.log(`-- keyring ${locked ? "locked" : "unlocked"}`);
  const root = mkdtempSync(join(tmpdir(), "bp-keyring-"));
  const fake = join(root, "bin"), log = join(root, "agents.log"), home = join(root, "home"), proj = join(root, "proj");
  for (const d of [fake, home, proj]) mkdirSync(d);
  writeFileSync(join(home, "events.jsonl"), "");
  writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
  writeFileSync(join(fake, "codex"), `#!/bin/sh
echo "CODEX RAN $*" >> "$FAKE_LOG"
printf '%s\\n' '{"type":"thread.started","thread_id":"cx-1"}'
printf '%s\\n' '{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"DONE"}}'
printf '%s\\n' '{"type":"turn.completed","usage":{}}'
`);
  for (const n of ["claude", "codex"]) chmodSync(join(fake, n), 0o755);
  for (const n of ["grok", "muse"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
  const port = freePort();
  if (port === 3787) throw new Error("refusing the live port");
  const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log,
      BACKPLANE_KEYRING_LOCKED: locked ? "1" : "0", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, TMPDIR: root },
    stdout: "ignore", stderr: Bun.file(join(root, "hub.err")),
  });
  try {
    let up = false;
    for (let i = 0; i < 200 && !up; i++) { try { up = (await fetch(`http://127.0.0.1:${port}/hello`)).status === 200; } catch {} if (!up) await sleep(100); }
    check(up, "the hub is up");
    const seen: any[] = [];
    const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
    ws.binaryType = "arraybuffer";
    ws.onmessage = (e) => {
      let o: any = null;
      try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
      for (const c of o?.items ?? []) seen.push(typeof c === "string" ? JSON.parse(c) : c);
    };
    await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
    let id = 0;
    const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
    send("project.add", { path: proj });
    const pc: any = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated"));
    const made = async (title: string, provider: string) => {
      await sleep(500);
      send("thread.create", { project: pc.id, title, env: "local", provider });
      return (await until(10000, () => seen.find((x) => x.$ === "ThreadCreated" && x.title === title)))?.id as string;
    };
    const turns = (th: string) => seen.filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => x.state);
    const errors = (th: string) => seen.filter((x) => x.thread === th && (x.$ === "ActivityLogged" || x.$ === "TurnChanged")).map((x) => String(x.summary ?? x.error ?? x.message ?? "")).filter(Boolean);
    const cx = await made("cx", "codex");
    const t0 = Date.now();
    send("turn.start", { thread: cx, text: "go", msg: "c-1" });
    const ended = await until(10000, () => (["failed", "completed"].includes(turns(cx).at(-1)) ? turns(cx).at(-1) : undefined));
    const ran = existsSync(log) && readFileSync(log, "utf8").includes("CODEX RAN exec");
    if (locked) {
      check(ended === "failed" && Date.now() - t0 < 5000, `the codex turn fails at once (${turns(cx).join(",")}, ${Date.now() - t0} ms)`);
      check(errors(cx).some((e) => e.includes("codex was not started") && e.includes("keyring is locked")), `saying codex was not started and why (${errors(cx).join(" | ")})`);
      check(!ran, "codex exec never ran (only its --version probe)");
    } else {
      check(ended === "completed" && ran, `the codex turn runs and completes (${turns(cx).join(",")})`);
    }
    const cl = await made("cl", "claude");
    send("turn.start", { thread: cl, text: "mode:exit-now go", msg: "l-1" });
    await until(15000, () => (turns(cl).at(-1) === "failed" ? true : undefined));
    const why = errors(cl).join(" | ");
    check(turns(cl).at(-1) === "failed" && why.includes("(exit 3)"), `the claude failure says its exit (${why})`);
    check(locked ? why.includes("keyring is locked") : !why.includes("keyring"), locked ? "and carries the keyring note" : "and no keyring note");
  } finally {
    try { process.kill(proc.pid, "SIGKILL"); } catch {}
    await proc.exited;
    if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
  }
}

await run(true);
await run(false);
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
