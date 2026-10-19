// Scratch threads end to end: a headless hub on a fresh home makes the
// Scratch project, a scratch thread gets its own git folder and its first
// turn runs there, and /give hands it to a thread and to a new thread in a
// project, both with the conversation queued.
//
//   bun test/tools/scratch_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane, WIREDIR to build/wire (bend
// test/wire/index.html -o build/wire). KEEP=1 keeps the temp folder.
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-scratch-"));
const fake = join(root, "bin");
mkdirSync(fake);
// a turn that notes where it ran, then answers
writeFileSync(join(fake, "claude"), `#!/bin/sh
say() { printf '%s\\n' "$1"; }
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      pwd > "${root}/ran-in"
      say '{"type":"system","subtype":"init","session_id":"s1","cwd":"/tmp","tools":[],"model":"fake","permissionMode":"default"}'
      say '{"type":"assistant","message":{"id":"m1","role":"assistant","content":[{"type":"text","text":"A buck converter it is."}]},"session_id":"s1"}'
      say '{"type":"result","subtype":"success","is_error":false,"result":"A buck converter it is.","session_id":"s1"}' ;;
  esac
done
`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse", "omp"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
const home = join(root, "home");
const proj = join(root, "board");
mkdirSync(proj);

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
    await sleep(100);
  }
};

let fail = 0;
const check = (ok: boolean, what: string, more = "") => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}${ok || !more ? "" : ": " + more}`);
  if (!ok) fail++;
};

const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: "ignore",
});
try {
  for (let i = 0; i < 300; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
  const seen: any[] = [];
  const replies = new Map<number, any>();
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const text = typeof e.data === "string";
    let o: any = null;
    try { o = JSON.parse(text ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(c);
    if (o?.t === "reply") replies.set(Number(o.id), o);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => { ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))); return id; };

  const sp = await until(10000, () => seen.find((c) => c.$ === "ProjectCreated" && c.id === "scratch"));
  check(!!sp && sp.root === join(home, "scratch"), "a fresh hub makes Scratch", JSON.stringify(sp));

  let n0 = seen.length;
  send("thread.create", { project: "scratch", title: "New thread" });
  const th = await until(10000, () => seen.slice(n0).find((c) => c.$ === "ThreadCreated"));
  const dir = join(home, "scratch", th?.id ?? "?");
  check(!!th && th.worktree === dir, "a scratch thread works in its own folder", JSON.stringify(th));
  check(!!(await until(5000, () => existsSync(join(dir, ".git")) || undefined)), "the folder is a git repository");

  send("turn.start", { thread: th.id, text: "which regulator for 12V to 3V3?", msg: "u1" });
  const done = await until(30000, () => seen.find((c) => c.$ === "TurnChanged" && c.thread === th.id && JSON.stringify(c).includes("ompleted")));
  check(!!done, "its turn ran");
  const ran = existsSync(join(root, "ran-in")) ? readFileSync(join(root, "ran-in"), "utf8").trim() : "";
  check(ran === dir, "in its folder", ran);

  n0 = seen.length;
  send("project.add", { path: proj });
  const pc = await until(10000, () => seen.slice(n0).find((c) => c.$ === "ProjectCreated"));
  send("thread.create", { project: pc.id, title: "Power" });
  const t2 = await until(10000, () => seen.slice(n0).find((c) => c.$ === "ThreadCreated"));
  await sleep(500);

  n0 = seen.length;
  const g = send("bots.give", { thread: th.id, text: "/give >power %board keep the part choice" });
  const r = await until(10000, () => replies.get(g));
  check(!!r && r.ok !== false && !r.error, "give is accepted", JSON.stringify(r));
  await sleep(500);
  const got = seen.slice(n0);
  const q2 = got.find((c) => c.$ === "TurnQueued" && c.thread === t2.id);
  check(!!q2 && q2.text.includes("A buck converter it is.") && q2.text.includes("keep the part choice"), "the thread has the conversation and the note", JSON.stringify(q2)?.slice(0, 200));
  const t3 = got.find((c) => c.$ === "ThreadCreated" && c.project === pc.id);
  check(!!t3 && !!got.find((c) => c.$ === "TurnQueued" && c.thread === t3.id), "the project gets a new thread with it queued");
  check(got.filter((c) => c.$ === "ActivityLogged" && c.thread === th.id && String(c.summary).startsWith("Given to")).length === 2, "the scratch thread says where it went");

  const bad = send("bots.give", { thread: th.id, text: "/give nowhere" });
  const rb = await until(10000, () => replies.get(bad));
  check(!!rb && JSON.stringify(rb).includes("name a thread"), "naming nothing is refused", JSON.stringify(rb));
  ws.close();
} finally {
  proc.kill();
  await proc.exited;
  if (!process.env.KEEP) rmSync(root, { recursive: true, force: true });
  else console.log(root);
}
process.exit(fail ? 1 : 0);
