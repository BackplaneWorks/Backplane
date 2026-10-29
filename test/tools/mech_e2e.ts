// End to end: the Mechanical page's data. Starts a headless hub on a
// temporary home and a free port, adds a project holding FreeCAD parts
// (STEP files under mech/), and checks: `backplane --render` writes each
// part's four views; the hub lists the parts and renders (mech.list) and
// serves a render (/img); mech.render (the Mech page's Create renders)
// draws a part as the hub's own --render child; an agent's viewer_show with kind mech reaches
// the thread; and a part's 3D model for the phones and the web
// (kicad.watch 3d with its path) stands Z up. Prints "ok ..." / "FAIL ...".
//
//   bun test/tools/mech_e2e.ts PROJECT [BINARY] [WIREDIR]
//
// PROJECT is a folder with mech/*.step (e.g. made with FreeCAD); BINARY
// defaults to build/backplane with backplane-step2glb beside it; WIREDIR
// to build/wire (bend test/wire/index.html -o build/wire).
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [project, bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
if (!project) throw new Error("usage: bun test/tools/mech_e2e.ts PROJECT [BINARY] [WIREDIR]");
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 400)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

async function until<T>(f: () => T | undefined, ms = 20000): Promise<T | undefined> {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const v = f();
    if (v) return v;
    await sleep(100);
  }
  return undefined;
}

const root = resolve(project);
const steps = readdirSync(join(root, "mech")).filter((f) => /\.(step|stp)$/i.test(f)).sort();

// the renders, drawn the way an agent draws them
for (const s of steps) {
  const r = Bun.spawnSync([resolve(bin), "--render", `mech/${s}`], { cwd: root });
  const out = r.stdout.toString().trim().split("\n");
  const stem = s.replace(/\.[^.]+$/, "");
  const views = ["iso", "front", "top", "right"].map((v) => join(root, "mech", "renders", `${stem}.${v}.png`));
  check(`--render draws ${stem}'s four views`, r.exitCode === 0 && views.every((v) => out.includes(v) && existsSync(v)), r.stderr.toString() || out);
  const png = readFileSync(views[0]);
  check(`${stem}'s render is a PNG`, png.subarray(1, 4).toString() === "PNG", png.subarray(0, 8));
}

const home = mkdtempSync(join(tmpdir(), "bp-mech-e2e-"));
const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, DISPLAY: "", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9" },
  stdout: "ignore",
  stderr: "ignore",
});
const seen: any[] = [];
const msgs: any[] = [];
const plots: Uint8Array[] = [];
let ws: WebSocket;
let n = 0;
const send = (m: string, p: any) => { ws.send(W.encode(JSON.stringify({ id: ++n, m, p }))); return n; };

async function mcp(thread: string, name: string, args: any): Promise<string> {
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const r = await fetch(`http://127.0.0.1:${port}/mcp/${thread}?token=${token}`, {
    method: "POST",
    headers: { "content-type": "application/json", accept: "application/json, text/event-stream" },
    body: JSON.stringify({ jsonrpc: "2.0", id: ++n, method: "tools/call", params: { name, arguments: args } }),
  });
  return r.text();
}

// a plot frame's box3 (x0 y0 z0 x1 y1 z1, in the hub's units), read
// from its CBOR without decoding the rest: the key "box3", then an array
function box3(b: Uint8Array): number[] | undefined {
  const i = Buffer.from(b).indexOf(Buffer.from("box3"));
  if (i < 0) return undefined;
  let k = i + 4;
  const len = b[k++] & 31;
  const out: number[] = [];
  for (let j = 0; j < len; j++) {
    const h = b[k++];
    const ai = h & 31;
    let v = ai < 24 ? ai : ai === 24 ? b[k++] : ai === 25 ? (b[k++] << 8) | b[k++] : ((b[k++] << 24) >>> 0) + (b[k++] << 16) + (b[k++] << 8) + b[k++];
    out.push(h >> 5 === 1 ? -1 - v : v);
  }
  return out;
}

try {
  if (!(await until(() => existsSync(join(home, "token")), 10000))) throw new Error("the hub did not start");
  await sleep(300);
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const b = new Uint8Array(e.data);
    // plots are for the viewers; the rest is the client's JSON
    if (b[1] === 1 && Buffer.from(b.subarray(3, 7)).toString() === "plot") { plots.push(b); return; }
    const o = JSON.parse(W.decode(b));
    msgs.push(o);
    for (const c of o.items ?? []) seen.push(c);
  };
  await new Promise((r) => (ws.onopen = r));
  send("project.add", { path: root });
  const proj = await until(() => seen.find((c) => c.$ === "ProjectCreated"));
  send("thread.create", { project: proj.id, title: "Parts" });
  const th = (await until(() => seen.find((c) => c.$ === "ThreadCreated")))!.id;

  const id = send("mech.list", { thread: th });
  const r = await until(() => msgs.find((o) => o.t === "reply" && o.id === id));
  const listed: string = r?.result?.mech ?? "";
  const paths = listed.trim().split("\n").map((l) => l.split(" ").slice(2).join(" "));
  check("mech.list names the thread's project root", r?.result?.root === root, r);
  check("mech.list lists every part", steps.every((s) => paths.includes(`mech/${s}`)), paths);
  check("mech.list lists the renders", paths.filter((p) => p.startsWith("mech/renders/")).length === steps.length * 4, paths);
  check("mech.list lists nothing else", paths.every((p) => /\.(step|stp|glb|png)$/i.test(p)), paths);

  // the Mech page's Create renders: the hub draws a part as its own --render child
  const again = join(root, "mech", "renders", steps[0].replace(/\.[^.]+$/, "") + ".iso.png");
  rmSync(again, { force: true });
  const rid = send("mech.render", { thread: th, path: `mech/${steps[0]}` });
  const rr = await until(() => msgs.find((o) => o.t === "reply" && o.id === rid), 600000);
  check("mech.render draws the part's views", rr?.ok !== false && rr?.result?.error === "" && existsSync(again), rr);
  check("mech.render answers the parts with its renders", (rr?.result?.mech ?? "").includes("mech/renders/"), rr);

  const token = readFileSync(join(home, "token"), "utf8").trim();
  const shot = join(root, "mech", "renders", steps[0].replace(/\.[^.]+$/, "") + ".iso.png");
  const img = await fetch(`http://127.0.0.1:${port}/img?path=${encodeURIComponent(shot)}&v=1&token=${token}`);
  check("the hub serves a render", img.status === 200 && (img.headers.get("content-type") ?? "").includes("png"), img.status);

  await mcp(th, "viewer_show", { path: join(root, "mech", steps[0]), kind: "mech" });
  const v = await until(() => msgs.find((o) => o.t === "view" && o.kind === "mech"));
  check("viewer_show kind mech reaches the thread", v?.thread === th && v?.path?.endsWith(steps[0]), v);

  send("kicad.watch", { kind: "3d", root, path: join(root, "mech", steps[0]) });
  const p = await until(() => plots.find((b) => box3(b)), 60000);
  const b = p ? box3(p)! : [];
  check("a part's 3D model comes for the phones and the web", !!p, plots.length);
  // the test bracket: 60 wide, 40 deep, 30 tall; Z up is its height
  const w = b[3] - b[0], d = b[4] - b[1], h = b[5] - b[2];
  if (steps[0] === "bracket.step") check("the part stands Z up", Math.abs(h / w - 0.5) < 0.01 && Math.abs(d / w - 2 / 3) < 0.01, b);
} catch (e) {
  check("run", false, String(e));
} finally {
  proc.kill();
  await proc.exited;
  rmSync(home, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
