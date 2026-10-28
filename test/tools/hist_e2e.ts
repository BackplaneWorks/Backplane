// The design history end to end: a headless hub, a stand-in `claude` that
// edits a real KiCad board and schematic between its tool calls, and a
// client over the socket. Checks that each change is logged as a step
// naming the tool call it followed, that a step's version and a
// comparison of two plot like the phones get them (a comparison's items
// on the removed/changed/added layers), that the hub lists what a file
// may be compared with, and that a folder outside git keeps its steps too.
//
//   bun test/tools/hist_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane, WIREDIR to build/wire (bend
// test/wire/index.html -o build/wire). Needs git and KiCad's ecc83 demo
// (/usr/share/kicad/demos/ecc83). KEEP=1 keeps the temp folder.
import { chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const demo = "/usr/share/kicad/demos/ecc83";

const isPlot = (b: Uint8Array) =>
  b.length > 7 && b[0] >= 0xa0 && b[0] <= 0xb7 && b[1] === 1 && b[2] === 0x64 && b[3] === 0x70 && b[4] === 0x6c && b[5] === 0x6f && b[6] === 0x74;
function cbor(b: Uint8Array): any {
  let i = 0;
  const arg = (ai: number) => {
    if (ai < 24) return ai;
    if (ai === 24) return b[i++];
    if (ai === 25) { const v = (b[i] << 8) | b[i + 1]; i += 2; return v; }
    const v = ((b[i] << 24) >>> 0) + (b[i + 1] << 16) + (b[i + 2] << 8) + b[i + 3];
    i += 4;
    return v;
  };
  const item = (): any => {
    const h = b[i++], mt = h >> 5, n = arg(h & 31);
    switch (mt) {
      case 0: return n;
      case 1: return -1 - n;
      case 2: { const v = b.subarray(i, i + n); i += n; return v; }
      case 3: { const v = new TextDecoder().decode(b.subarray(i, i + n)); i += n; return v; }
      case 4: return Array.from({ length: n }, item);
      case 5: {
        const o: any = {};
        for (let k = 0; k < n; k++) { let key = item(); key = key === 1 ? "t" : key === 31 ? "key" : key; o[key] = item(); }
        return o;
      }
    }
    throw new Error(`cbor major ${mt}`);
  };
  return item();
}

const root = mkdtempSync(join(tmpdir(), "bp-hist-"));
const fake = join(root, "bin");
mkdirSync(fake);
// one turn: words, a tool call that moves a footprint and deletes a track,
// words, one that moves a schematic wire, the reply. Each change is made
// before its tool result is reported, and the next waits for the capture.
// It thinks a moment before its first edit, as a model does: the turn's
// first snapshot is taken as it starts, beside the agent, not before it.
writeFileSync(join(fake, "claude"), `#!/bin/sh
say() { printf '%s\\n' "$1"; }
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      say '{"type":"system","subtype":"init","session_id":"s1","cwd":"/tmp","tools":["Bash"],"model":"fake","permissionMode":"default"}'
      sleep 2
      say '{"type":"assistant","message":{"id":"m1","role":"assistant","content":[{"type":"text","text":"I will move R1 clear of the tube and drop its old track."},{"type":"tool_use","id":"tu1","name":"Bash","input":{"command":"python3 place.py"}}]},"session_id":"s1"}'
      sed -i 's/(at 141.605 99.695 90)/(at 151.605 104.695 90)/' ecc83-pp.kicad_pcb
      python3 - <<'PY'
import re
s=open("ecc83-pp.kicad_pcb").read()
i=s.index("(segment")
j=s.index("\\n\\t)", i)+3
open("ecc83-pp.kicad_pcb","w").write(s[:i]+s[j:])
PY
      say '{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu1","content":"moved"}]},"session_id":"s1"}'
      sleep 3
      say '{"type":"assistant","message":{"id":"m2","role":"assistant","content":[{"type":"text","text":"Now the schematic label."},{"type":"tool_use","id":"tu2","name":"Edit","input":{"file_path":"ecc83-pp.kicad_sch"}}]},"session_id":"s1"}'
      python3 - <<'PY'
s=open("ecc83-pp.kicad_sch").read()
i=s.index("(wire")
j=s.index("(xy ", i)+4
k=s.index(" ", j)
s=s[:j]+str(round(float(s[j:k])+5.08, 3))+s[k:]
open("ecc83-pp.kicad_sch","w").write(s)
PY
      say '{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu2","content":"edited"}]},"session_id":"s1"}'
      sleep 3
      say '{"type":"assistant","message":{"id":"m3","role":"assistant","content":[{"type":"text","text":"Done."}]},"session_id":"s1"}'
      say '{"type":"result","subtype":"success","is_error":false,"result":"Done.","session_id":"s1"}' ;;
  esac
done
`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
const home = join(root, "home");
const git = (cwd: string, ...a: string[]) => Bun.spawnSync(["git", "-c", "user.name=t", "-c", "user.email=t@t", ...a], { cwd }).stdout.toString();

function project(name: string, repo: boolean): string {
  const p = join(root, name);
  mkdirSync(p);
  for (const f of ["ecc83-pp.kicad_pcb", "ecc83-pp.kicad_sch", "ecc83-pp.kicad_pro"]) cpSync(join(demo, f), join(p, f));
  if (repo) {
    git(p, "init", "-q");
    git(p, "add", ".");
    git(p, "commit", "-qm", "first");
    git(p, "branch", "-q", "rev-b");
    git(p, "tag", "-a", "v1", "-m", "v1");
  }
  return p;
}

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
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: "ignore",
});
try {
  for (let i = 0; i < 300; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
  const seen: any[] = [];
  const plots: any[] = [];
  const replies = new Map<number, any>();
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const text = typeof e.data === "string";
    if (!text && isPlot(new Uint8Array(e.data as ArrayBuffer))) { plots.push(cbor(new Uint8Array(e.data as ArrayBuffer))); return; }
    let o: any = null;
    try { o = JSON.parse(text ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(c);
    if (o?.t === "reply") replies.set(Number(o.id), o);
    if (o?.t === "plot") plots.push(o);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => { ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))); return id; };

  async function run(name: string, repo: boolean, env: string) {
    const proj = project(name, repo);
    const n0 = seen.length;
    send("project.add", { path: proj });
    const pc = await until(10000, () => seen.slice(n0).find((c) => c.$ === "ProjectCreated"));
    if (!pc) throw new Error("no project");
    send("thread.create", { project: pc.id, title: name, env });
    const th = await until(10000, () => seen.slice(n0).find((c) => c.$ === "ThreadCreated"));
    if (!th) throw new Error("no thread");
    send("turn.start", { thread: th.id, text: "tidy the board" });
    const done = await until(60000, () => seen.slice(n0).find((c) => c.$ === "TurnChanged" && c.thread === th.id && JSON.stringify(c).includes("ompleted")));
    check(!!done, `${name}: the turn ran`);
    await sleep(4000);
    const steps = seen.slice(n0).filter((c) => c.$ === "ActivityLogged" && c.thread === th.id && c.tone === "step").map((c) => ({ id: c.act, rec: JSON.parse(c.summary) }));
    return { proj, th, steps };
  }

  // a repository, the thread in its own worktree
  const a = await run("repo", true, "worktree");
  const steps = a.steps;
  check(steps.length === 3, "three steps: before, the board, the schematic", JSON.stringify(steps.map((s) => s.rec.f)));
  const [s0, s1, s2] = steps;
  check(!!s0 && s0.rec.p === "" && s0.rec.f.length === 2, "the first holds the design as it was");
  check(!!s1 && s1.rec.e === "tu1" && JSON.stringify(s1.rec.f) === JSON.stringify(["M\tecc83-pp.kicad_pcb"]), "the board's change follows its tool call", JSON.stringify(s1?.rec));
  check(!!s2 && s2.rec.e === "tu2" && s2.rec.f.some((f: string) => f.endsWith("kicad_sch")), "the schematic's follows its own", JSON.stringify(s2?.rec));
  check(!!s1 && s1.rec.p === s0?.rec.c && s2?.rec.p === s1?.rec.c, "each step's parent is the step before");
  check(!!s1 && s1.rec.g.endsWith(".git") && !s1.rec.g.includes("/worktrees/"), "steps live in the repository's own git dir", s1?.rec.g);

  // a step's version plots; the comparison with the step before marks changes
  const spec = (s: any, rel: string) => `git\t${s.rec.g}\t${s.rec.c}\t${s.rec.t.length}\t${s.rec.t}/${rel}`;
  if (s0 && s1) {
    const root1 = s1.rec.t;
    plots.length = 0;
    send("kicad.watch", { kind: "board", root: root1, path: spec(s1, "ecc83-pp.kicad_pcb") });
    const p1 = await until(20000, () => plots.find((p) => p.key?.includes(s1.rec.c) && (p.cs || p.none)));
    check(!!p1 && Array.isArray(p1.cs) && p1.cs.length > 10, "a step's board plots", p1?.none);
    check(!!p1 && typeof p1.fam === "string" && p1.fam.endsWith(`|${root1}/ecc83-pp.kicad_pcb`), "its plot names the live file as its family", p1?.fam);
    // the step before, then this one again: only what the step changed is sent whole
    plots.length = 0;
    send("kicad.watch", { kind: "board", root: root1, path: spec(s0, "ecc83-pp.kicad_pcb") });
    await until(20000, () => plots.find((p) => p.key?.includes(s0.rec.c) && p.cs));
    plots.length = 0;
    send("kicad.watch", { kind: "board", root: root1, path: spec(s1, "ecc83-pp.kicad_pcb") });
    const p1b = await until(20000, () => plots.find((p) => p.key?.includes(s1.rec.c) && p.cs));
    const kept = (p1b?.cs ?? []).filter((c: any) => typeof c === "number").length;
    check(!!p1b && kept > (p1b.cs.length * 3) / 4, "stepping to it resends only the chunks it changed", `${kept} of ${p1b?.cs.length} kept`);
    const diff = `diff\n${spec(s0, "ecc83-pp.kicad_pcb")}\n${spec(s1, "ecc83-pp.kicad_pcb")}`;
    plots.length = 0;
    send("kicad.watch", { kind: "board", root: root1, path: diff });
    const p2 = await until(20000, () => plots.find((p) => p.key?.startsWith("0|") && p.key.includes("diff") && (p.cs || p.none)));
    const layers = new Set((p2?.cs ?? []).filter((c: any) => typeof c === "object").map((c: any) => c.l));
    check(!!p2 && layers.has(26) && layers.has(27), "the comparison marks removed and changed", JSON.stringify([...layers]));
    check(!!p2 && !layers.has(28), "and adds nothing it did not add", JSON.stringify([...layers]));
    // a file a commit lacks says so rather than waiting
    plots.length = 0;
    send("kicad.watch", { kind: "board", root: root1, path: spec(s1, "nope.kicad_pcb") });
    const p3 = await until(20000, () => plots.find((p) => p.key?.includes("nope") && (p.cs || p.none)));
    check(!!p3 && typeof p3.none === "string" && p3.none.includes("cannot be read"), "a missing version says so", JSON.stringify(p3));

    // what the file may be compared with
    const rid = send("history.refs", { thread: a.th.id });
    const r = await until(10000, () => replies.get(rid));
    const res = r?.result ?? {};
    check(res.git === s1.rec.g && res.top === s1.rec.t, "refs name the git dir and top", JSON.stringify(res).slice(0, 200));
    check(typeof res.refs === "string" && res.refs.includes("head\tHEAD\t") && res.refs.includes("branch\trev-b\t"), "refs list HEAD and the branches", res.refs);
    check(typeof res.pcb === "string" && res.pcb.endsWith("ecc83-pp.kicad_pcb"), "and the project's board", res.pcb);
    const head = /head\tHEAD\t([0-9a-f]+)/.exec(res.refs ?? "")?.[1];
    if (head) {
      plots.length = 0;
      send("kicad.watch", { kind: "schematic", root: root1, path: `diff\n${spec({ rec: { ...s1.rec, c: head } }, "ecc83-pp.kicad_sch")}\n${root1}/ecc83-pp.kicad_sch` });
      const p4 = await until(20000, () => plots.find((p) => p.key?.startsWith("1|") && (p.cs || p.none)));
      const l4 = new Set((p4?.cs ?? []).filter((c: any) => typeof c === "object").map((c: any) => c.l));
      check(!!p4 && (l4.has(27) || l4.has(26)), "HEAD against the working copy marks the schematic's edit", JSON.stringify([...l4]) + (p4?.none ?? ""));
    }
  }

  // design review (docs/design-review.md): tags listed, any ref typed in
  // resolved, a comparison on some layers only
  if (s0 && s1) {
    const rid = send("history.refs", { thread: a.th.id });
    const res = (await until(10000, () => replies.get(rid)))?.result ?? {};
    check(typeof res.refs === "string" && res.refs.includes("tag\tv1\t"), "refs list the tags", res.refs);
    const v1 = git(a.proj, "rev-parse", "v1^{commit}").trim();
    const q = send("history.resolve", { thread: a.th.id, side: "a", ref: "v1" });
    const rr = (await until(10000, () => replies.get(q)))?.result?.resolved;
    check(!!rr && rr.commit === v1 && rr.side === "a" && rr.ref === "v1" && rr.thread === a.th.id, "a tag typed in resolves to its commit", JSON.stringify(rr));
    check(!!rr && rr.git.endsWith(".git") && rr.top.length > 0, "with the git dir and top a version needs", JSON.stringify(rr));
    const bad = send("history.resolve", { thread: a.th.id, side: "b", ref: "no-such-ref" });
    const rb = await until(10000, () => replies.get(bad));
    check(!!rb && rb.ok === false && /no commit, branch or tag/.test(rb.result?.error ?? ""), "an unknown ref says so", JSON.stringify(rb));
    const dash = send("history.resolve", { thread: a.th.id, side: "b", ref: "--output=/tmp/x" });
    const rd = await until(10000, () => replies.get(dash));
    check(!!rd && rd.ok === false, "a ref like an option is refused", JSON.stringify(rd));
    // the same comparison on a layer nothing changed on marks nothing
    const diff = `diff\n${spec(s0, "ecc83-pp.kicad_pcb")}\n${spec(s1, "ecc83-pp.kicad_pcb")}\n${1 << 30}`;
    plots.length = 0;
    send("kicad.watch", { kind: "board", root: s1.rec.t, path: diff });
    const p5 = await until(20000, () => plots.find((p) => p.key?.startsWith("0|") && p.key.includes("diff") && (p.cs || p.none)));
    const l5 = new Set((p5?.cs ?? []).filter((c: any) => typeof c === "object").map((c: any) => c.l));
    check(!!p5 && !l5.has(26) && !l5.has(27) && !l5.has(28) && l5.size > 0, "a comparison on an untouched layer marks nothing and dims the rest", JSON.stringify([...l5]) + (p5?.none ?? ""));
  }

  // a folder outside git keeps its steps in .backplane/history.git
  const b = await run("plain", false, "local");
  check(b.steps.length === 3, "a folder outside git has its steps too", JSON.stringify(b.steps.map((s) => s.rec.f)));
  check(existsSync(join(b.proj, ".backplane/history.git")), "in its own history repository");
  check(!!b.steps[1] && b.steps[1].rec.g === join(b.proj, ".backplane/history.git"), "which the steps name", b.steps[1]?.rec.g);
  const st = git(b.proj, "--git-dir=.backplane/history.git", "status", "--porcelain");
  check(!st.includes(".backplane"), "and which it never records");
  ws.close();
} finally {
  proc.kill(9);
  await proc.exited;
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
