// Design review end to end (docs/design-review.md): a headless hub, a
// repository made from KiCad's ecc83 demo (tag v1, then two values
// changed), and a client over the socket. Checks that the hub answers a
// BOM and a BOM change (review.bom, through a `--review bom` child), runs
// the design checks on the working copy and on a commit (review.checks:
// kicad-cli's ERC and DRC, the BOM rules, the checklist, a custom check),
// and ticks a checklist box into the repository (review.tick).
//
//   bun test/tools/review_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane, WIREDIR to build/wire (bend
// test/wire/index.html -o build/wire). Needs git, kicad-cli and the demo
// (/usr/share/kicad/demos/ecc83). KEEP=1 keeps the temp folder.
import { chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const demo = "/usr/share/kicad/demos/ecc83";

const root = mkdtempSync(join(tmpdir(), "bp-review-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse", "omp"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
const home = join(root, "home");
const git = (cwd: string, ...a: string[]) => Bun.spawnSync(["git", "-c", "user.name=t", "-c", "user.email=t@t", ...a], { cwd }).stdout.toString();

const proj = join(root, "board");
mkdirSync(proj);
for (const f of ["ecc83-pp.kicad_pcb", "ecc83-pp.kicad_sch", "ecc83-pp.kicad_pro"]) cpSync(join(demo, f), join(proj, f));
git(proj, "init", "-q", "-b", "main");
git(proj, "add", ".");
git(proj, "commit", "-qm", "rev A");
git(proj, "tag", "-a", "v1", "-m", "v1");
const sch = join(proj, "ecc83-pp.kicad_sch");
writeFileSync(sch, readFileSync(sch, "utf8").replace('(property "Value" "47K"', '(property "Value" "56K"'));
mkdirSync(join(proj, ".backplane"));
writeFileSync(join(proj, ".backplane/checks.json"), JSON.stringify({ checks: [{ name: "says hi", run: "echo hi; exit 3", on: ["manual", "release"] }] }));
git(proj, "add", ".");
git(proj, "commit", "-qm", "rev B: R4 56K");
const head = git(proj, "rev-parse", "HEAD").trim();
const v1 = git(proj, "rev-parse", "v1^{commit}").trim();

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
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
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
  const ask = async (m: string, p: object, ms = 60000) => { const q = send(m, p); return await until(ms, () => replies.get(q)); };

  send("project.add", { path: proj });
  const pc = await until(10000, () => seen.find((c) => c.$ === "ProjectCreated"));
  if (!pc) throw new Error("no project");
  send("thread.create", { project: pc.id, title: "review", env: "local" });
  const th = await until(10000, () => seen.find((c) => c.$ === "ThreadCreated"));
  if (!th) throw new Error("no thread");

  // versions as the clients name them: the thread's git folder and top
  // (its worktree) from history.refs
  const refs = (await ask("history.refs", { thread: th.id }))?.result ?? {};
  const top: string = refs.top ?? "";
  const spec = (c: string, rel: string) => `git\t${refs.git}\t${c}\t${top.length}\t${top}/${rel}`;
  check(!!refs.git && top.length > 0, "the thread's folder", JSON.stringify(refs).slice(0, 200));

  // a BOM, then the change since v1
  const t0 = Date.now();
  const one = (await ask("review.bom", { thread: th.id, a: "", b: "" }))?.result?.report;
  check(!!one && one.page === "bom" && one.thread === th.id, "review.bom answers a report", JSON.stringify(one)?.slice(0, 200));
  check(!!one && /^11 parts in \d+ lines/.test(one.md), "the working copy's BOM: 11 parts", one?.md?.slice(0, 80));
  check(!!one && one.md.includes("| R4 | 1 | 56K |"), "with R4's new value", one?.md);
  check(!!one && !one.md.includes("| MPN |"), "and no empty MPN column", one?.md?.split("\n").slice(2, 4).join(" / "));
  console.log(`     (a BOM in ${Date.now() - t0} ms)`);
  const chg = (await ask("review.bom", { thread: th.id, a: spec(v1, "ecc83-pp.kicad_pcb"), b: "" }))?.result?.report;
  check(!!chg && chg.md.startsWith("0 added, 0 removed, 1 changed"), "the change since v1: one part", chg?.md?.slice(0, 120));
  check(!!chg && chg.md.includes("R4") && chg.md.includes("47K") && chg.md.includes("56K"), "R4 47K → 56K", chg?.md);
  const none = (await ask("review.bom", { thread: th.id, a: spec(head, "ecc83-pp.kicad_sch"), b: "" }))?.result?.report;
  check(!!none && none.md.trim() === "No changes", "HEAD against the working copy: no change", none?.md);

  // the design checks
  const t1 = Date.now();
  const ck = (await ask("review.checks", { thread: th.id, commit: "", trigger: "manual" }, 120000))?.result?.report;
  console.log(`     (checks in ${Date.now() - t1} ms)`);
  check(!!ck && ck.page === "checks" && !ck.err, "review.checks answers a report", JSON.stringify(ck)?.slice(0, 300));
  check(!!ck && /ERC|DRC/.test(ck.md) && /ERC|DRC/.test(ck.line), "with KiCad's ERC and DRC", ck?.line);
  check(!!ck && ck.md.includes("says hi") && /exit|fail/i.test(ck.md), "and the custom check that failed", ck?.md?.slice(-400));
  check(!!ck && ck.gate.includes("Custom: 1 failed"), "which fails the gate", ck?.gate);
  check(!!ck && Array.isArray(ck.tasks) && ck.tasks.length >= 10 && ck.tasks.every((t: any) => !t.done), "the default checklist, nothing ticked", JSON.stringify(ck?.tasks?.slice(0, 2)));
  const atv1 = (await ask("review.checks", { thread: th.id, commit: v1, trigger: "manual" }, 120000))?.result?.report;
  check(!!atv1 && !atv1.err && !atv1.md.includes("says hi"), "checks on v1 run that commit (no custom check yet)", JSON.stringify(atv1)?.slice(0, 300));
  check(readdirSync(tmpdir()).filter((f) => f.startsWith("bp-review.")).length === 0, "and leave no folder behind");

  // a box ticked lands in the repository
  const tk = (await ask("review.tick", { thread: th.id, index: 1 }))?.result?.ticked;
  check(!!tk && tk.tasks[1]?.done === true && tk.tasks[0]?.done === false, "a box ticked", JSON.stringify(tk?.tasks?.slice(0, 2)));
  const file = join(top, ".backplane/checklist.md");
  check(existsSync(file) && readFileSync(file, "utf8").split("\n").filter((l) => l.includes("[x]")).length === 1, "into the thread's .backplane/checklist.md", existsSync(file) ? readFileSync(file, "utf8").slice(0, 200) : "missing");
  const tk2 = (await ask("review.tick", { thread: th.id, index: 1 }))?.result?.ticked;
  check(!!tk2 && tk2.tasks[1]?.done === false, "and cleared again");

  // review comments: the person's over the socket, the agent's through MCP
  const pc1 = await ask("review.comment", { thread: th.id, op: "add", file: "ecc83-pp.kicad_pcb", kind: "board", layer: "F.Cu", x: 141.6, y: 99.7, item: "", text: "Move C1 off the edge" });
  check(!!pc1 && pc1.ok === true, "a person's comment is taken", JSON.stringify(pc1));
  const act1 = await until(10000, () => seen.find((c) => c.$ === "ActivityLogged" && c.thread === th.id && c.tone === "review" && c.kind === "add"));
  check(!!act1 && JSON.parse(act1.summary).text === "Move C1 off the edge" && JSON.parse(act1.summary).a.file === "ecc83-pp.kicad_pcb", "logged as a review Act", JSON.stringify(act1));
  const empty = await ask("review.comment", { thread: th.id, op: "add", text: "  " });
  check(!!empty && empty.ok === false, "an empty comment is refused", JSON.stringify(empty));
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const mcp = async (name: string, args: any) => {
    const r = await fetch(`http://127.0.0.1:${port}/mcp/${th.id}?token=${token}`, { method: "POST", headers: { "content-type": "application/json", accept: "application/json, text/event-stream" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name, arguments: args } }) });
    return await r.text();
  };
  const listed = await mcp("review_comments", {});
  check(listed.includes("Move C1 off the edge") && listed.includes("review_comments"), "the agent reads it", listed.slice(0, 300));
  const made = await mcp("review_comment", { op: "add", parent: act1?.act ?? "", file: "ecc83-pp.kicad_pcb", text: "Moved it 2 mm in" });
  check(made.includes("\"id\"") && !made.includes("isError\":true"), "the agent answers it", made.slice(0, 300));
  const rep = await until(10000, () => seen.find((c) => c.$ === "ActivityLogged" && c.thread === th.id && c.tone === "review" && c.summary.includes("Moved it 2 mm in")));
  check(!!rep && JSON.parse(rep.summary).parent === act1?.act && JSON.parse(rep.summary).by === "claude", "as a reply by the agent", JSON.stringify(rep));
  const res = await ask("review.comment", { thread: th.id, op: "resolve", id: act1?.act ?? "" });
  check(!!res && res.ok === true, "resolved by the person", JSON.stringify(res));
  const after = await mcp("review_comments", {});
  check(after.includes("No open comments") && after.includes("resolved") && after.includes(":1"), "and the agent sees none open", after.slice(0, 300));

  // a release: the checks gate it, forced it is made, never published unasked
  const gated = (await ask("review.release", { thread: th.id, tag: "v2", publish: false, force: false }, 180000))?.result?.report;
  check(!!gated && gated.err.startsWith("No release: the checks fail"), "a release is refused while the checks fail", JSON.stringify(gated)?.slice(0, 300));
  check(!git(top, "tag", "--list", "v2").includes("v2"), "and makes no tag");
  const t2 = Date.now();
  const rel = (await ask("review.release", { thread: th.id, tag: "v2", publish: false, force: true }, 300000))?.result?.report;
  console.log(`     (a release in ${Date.now() - t2} ms)`);
  check(!!rel && rel.page === "release" && !rel.err, "forced, the release is made", JSON.stringify(rel)?.slice(0, 400));
  check(git(top, "tag", "--list", "v2").includes("v2"), "with its tag");
  const rdir = join(top, ".backplane/releases/v2");
  const has = (f: string) => existsSync(join(rdir, f));
  check(has("notes.md") && has("manifest.json"), "notes and a manifest", rdir);
  const outs = existsSync(rdir) ? Bun.spawnSync(["find", rdir, "-type", "f"]).stdout.toString() : "";
  check(/\.gbr|\.g[tb][lso]|-F_Cu/i.test(outs) && /\.drl/.test(outs), "Gerbers and drill files", outs.slice(0, 400));
  check(/cpl\.csv/.test(outs) && /bom\.csv/.test(outs) && /\.step/.test(outs) && /\.pdf/.test(outs), "CPL, BOM, STEP and PDFs", outs.slice(0, 600));
  const notes = has("notes.md") ? readFileSync(join(rdir, "notes.md"), "utf8") : "";
  check(notes.startsWith("# v2") && notes.includes("Changes since v1") && notes.includes("R4"), "notes name the tag before and the BOM change", notes.slice(0, 400));
  check(!!rel && rel.md.includes("**Files**") && !rel.md.includes("Published"), "and nothing is published", rel?.md?.slice(-300));
  const again = (await ask("review.release", { thread: th.id, tag: "v2", publish: false, force: true }, 60000))?.result?.report;
  check(!!again && again.err.includes("tag could not be made"), "a tag that exists is refused", JSON.stringify(again)?.slice(0, 300));
  const badtag = (await ask("review.release", { thread: th.id, tag: "v 3..", publish: false, force: true }))?.result?.report;
  check(!!badtag && badtag.err.includes("not a tag name"), "a bad tag name is refused", JSON.stringify(badtag));

  // a ref typed in that is not there
  const bad = await ask("review.checks", { thread: th.id, commit: "-x", trigger: "manual" });
  check(!!bad && bad.ok === false, "a commit like an option is refused", JSON.stringify(bad));
  ws.close();
} finally {
  proc.kill(9);
  await proc.exited;
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
