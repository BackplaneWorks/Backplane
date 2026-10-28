// End to end: a new project thread's worktree. The hub's own jobs report
// back to it as client 0 (server.bend GitJob.say: thread.worktree,
// thread.local, thread.note); a hub with no client 0 (headless: the JS
// service, backplane-serve) dropped them from bb783cc13 on (Hub.rpc.client),
// so a thread never got its worktree and its first turn waited out
// Worktree.wait() and ran in the project folder. On one headless hub whose
// only client is this test's: a thread in a git project gets WorktreeSet
// (a path under .backplane/worktrees) and its agent starts there; one in a
// plain folder gets WorktreeSkipped and its agent starts in the folder;
// both answer, well inside the 120 s wait. Prints "ok ..." / "FAIL ...".
//
//   bun test/tools/worktree_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane-serve, WIREDIR to build/wire.
import { mkdtempSync, readdirSync, readFileSync, writeFileSync, chmodSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const [bin = "build/backplane-serve", wire = "build/wire"] = process.argv.slice(2);
let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => { console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 600)}`); if (!ok) failed = true; };
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const root = mkdtempSync(join(tmpdir(), "bp-ord-"));
const fake = join(root, "bin"); mkdirSync(fake);
writeFileSync(join(fake, "claude"), `#!/bin/sh
case "$*" in *--version*) echo "2.0.0"; exit 0 ;; esac
echo "$(pwd)" >> ${root}/claude.log
while IFS= read -r line; do case "$line" in *'"type":"user"'*)
printf '%s\\n' '{"type":"assistant","message":{"id":"m1","content":[{"type":"text","text":"pong"}]}}'
printf '%s\\n' '{"type":"result","is_error":false,"result":"pong"}' ;; esac; done
`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const port = s.port; s.stop(true);
const home = join(root, "home");
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, HOME: join(root, "fh"), DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_AGENTS: fake, PATH: `${fake}:/usr/bin:/bin` },
  stdout: "ignore", stderr: Bun.file(join(root, "hub.err")) });
try {
  for (let i = 0; i < 300; i++) { try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) break; } catch {} await sleep(100); }
  const seen: any[] = []; const replies = new Map<number, any>(); let n = 0;
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`); ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => { const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer))); for (const x of o.items ?? []) seen.push(x); if (o.t === "reply") replies.set(Number(o.id), o); };
  await new Promise((r) => (ws.onopen = r));
  const rpc = async (m: string, p: object) => { const id = ++n; ws.send(W.encode(JSON.stringify({ id, m, p }))); for (let i = 0; i < 200 && !replies.has(id); i++) await sleep(50); return replies.get(id); };
  const one = async (name: string, git: boolean) => {
    const pp = join(root, name); mkdirSync(pp);
    if (git) Bun.spawnSync(["sh", "-c", `cd ${pp} && git init -q && git -c user.email=a@b -c user.name=a commit -q --allow-empty -m init`]);
    await rpc("project.add", { path: pp });
    for (let i = 0; i < 100 && !seen.find((c) => c.$ === "ProjectCreated" && c.root === pp); i++) await sleep(50);
    const pc = seen.find((c) => c.$ === "ProjectCreated" && c.root === pp);
    const title = `WT ${name}`;
    await rpc("thread.create", { project: pc?.id, title });
    for (let i = 0; i < 100 && !seen.find((c) => c.$ === "ThreadCreated" && c.title === title); i++) await sleep(50);
    const th = seen.find((c) => c.$ === "ThreadCreated" && c.title === title)?.id;
    const t0 = Date.now();
    await rpc("turn.start", { thread: th, text: "hello", msg: `${name}-m1`, mode: "queue" });
    const want = git ? "WorktreeSet" : "WorktreeSkipped";
    let wt: any;
    for (let i = 0; i < 600 && !wt; i++) { wt = seen.find((c) => c.$ === want && c.id === th); if (!wt) await sleep(50); }
    check(`${name}: the job's ${want} reaches the hub`, !!wt, seen.filter((c) => c.thread === th || c.id === th).map((c) => c.$));
    if (git) check(`${name}: the worktree is under .backplane/worktrees`, !!wt && String(wt.worktree).includes("/.backplane/worktrees/"), wt);
    let pong: any;
    for (let i = 0; i < 600 && !pong; i++) { pong = seen.find((c) => c.$ === "MessagePosted" && c.thread === th && c.text === "pong"); if (!pong) await sleep(50); }
    check(`${name}: the turn starts and answers within 30 s (not the 120 s wait)`, !!pong && Date.now() - t0 < 30000, { ms: Date.now() - t0 });
    let cwds: string[] = [];
    try { cwds = readFileSync(join(root, "claude.log"), "utf8").trim().split("\n"); } catch {}
    const where = git ? wt?.worktree : pp;
    check(`${name}: the agent runs in ${git ? "the worktree" : "the folder"}`, !!where && cwds.includes(where), { where, cwds });
  };
  await one("repo", true);
  await one("plain", false);
} finally { proc.kill(); await proc.exited; }
console.log(failed ? "worktree e2e: FAIL" : "worktree e2e: ok");
process.exit(failed ? 1 : 0);
