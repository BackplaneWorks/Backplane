// Reading while a thread grows, the native window (nui.bend's Hold,
// layout.bend's TL.hold): a hub with its window on a running X server
// (Xvfb), the stand-in claude streaming lines; the window scrolled up with
// the wheel (xpoke), screenshots through a 200-line reply, and a band of
// the timeline compared pixel for pixel with the one before it streamed.
//   bun test/tools/hold_window.ts --bin BACKPLANE --display :77 [--row Y] [--box x0,y0,x1,y1] [--xpoke PATH] [--out DIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

const argv = process.argv.slice(2);
const opt = (name: string, d: string) => {
  const i = argv.indexOf(`--${name}`);
  return i >= 0 ? argv[i + 1] : d;
};
const here = resolve(import.meta.dir, "../..");
const bin = resolve(opt("bin", "build/backplane"));
const port = Number(opt("port", "3802"));
if (port === 3787) throw new Error("3787 is the live hub's port");
const web = opt("web", "");
const wire = opt("wire", "build/wire");
const display = opt("display", ":81");
const xvfbPath = opt("xvfb", "");
const xvfb = xvfbPath ? Bun.spawn([xvfbPath, display, "-screen", "0", "1600x1000x24", "-nolisten", "tcp"], { stdout: "ignore", stderr: "inherit" }) : null;
if (xvfb) await Bun.sleep(600);
const xpoke = opt("xpoke", join(homedir(), ".cache/bp-scroll/xpoke"));
const out = opt("out", mkdtempSync(join(tmpdir(), "bp-holdwin-shots-")));
mkdirSync(out, { recursive: true });
const rowY = Number(opt("row", "0"));
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};

const root = mkdtempSync(join(tmpdir(), "bp-hold-"));
const fake = join(root, "bin");
mkdirSync(fake);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${join(here, "test/tools/claude_standin.ts")} "$@"\n`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse", "agy"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
const home = join(root, "home");
const proj = join(root, "proj");
mkdirSync(proj);
mkdirSync(home);
writeFileSync(join(home, "events.jsonl"), "");
const args = [bin, "--home", home, "--port", String(port), "--no-tailscale", "--foreground"];
if (web) args.push("--web", resolve(web));
const proc = Bun.spawn(args, {
  env: { ...process.env, DISPLAY: display, WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
    BP_STANDIN_MODE: "stream", BP_STREAM_N: "200", BP_STREAM_MS: "15", BP_STREAM_SEP: "\n\n", BACKPLANE_AGENTS: fake, PATH: `${fake}:/usr/bin:/bin` },
  stdout: "ignore",
  stderr: "ignore",
});
const until = async <T>(ms: number, f: () => T | undefined | Promise<T | undefined>) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = await f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(40);
  }
};
try {
  await until(20000, async () => { try { return (await fetch(`http://127.0.0.1:${port}/hello`)).status === 200; } catch { return undefined; } });
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  const changes: any[] = [];
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const ch of o?.items ?? []) if (o.t !== "live") changes.push(ch);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  send("project.add", { path: proj });
  const pc = await until(10000, () => changes.find((c) => c.$ === "ProjectCreated"));
  if (!pc) throw new Error("no project");
  send("setting.set", { key: "restart.continue", value: "off" });
  // no first-run tour over the page
  send("setting.set", { key: "onboard.done", value: "1" });
  send("thread.create", { project: pc.id, title: "hold", env: "local", provider: "claude" });
  const th = (await until(10000, () => changes.find((c) => c.$ === "ThreadCreated")))?.id as string;
  if (!th) throw new Error("no thread");
  const replies = () => changes.filter((c) => c.$ === "MessagePosted" && c.thread === th && String(c.role).toLowerCase() === "assistant").length;
  const turn = async (word: string, n: number) => {
    send("turn.start", { thread: th, text: `go ${word}`, msg: `h-${word}` });
    return until(60000, () => replies() >= n ? true : undefined);
  };
  // history to read: three replies of 200 lines
  for (const [i, w] of ["a", "b", "c"].entries()) {
    if (!(await turn(w, i + 1))) throw new Error(`history ${w} did not land`);
  }


  const poke = (...a: string[]) => Bun.spawnSync([xpoke, ...a], { env: { ...process.env, DISPLAY: display } });
  let k = 0;
  const shot = (name: string) => {
    const f = join(out, `${String(++k).padStart(3, "0")}-${name}.png`);
    const got = Bun.spawnSync(["import", "-window", "root", f], { env: { ...process.env, DISPLAY: display } });
    if (got.exitCode) throw new Error(`screenshot failed: ${got.stderr}`);
    return f;
  };
  // the window's first-run tour, skipped (Escape), then the thread opened
  await sleep(2000);
  shot("start");
  poke("key", "Escape");
  await sleep(500);
  if (rowY) { poke("click", "130", String(rowY)); await sleep(1000); }
  shot("open");
  // scrolled up with the wheel over the timeline
  for (let i = 0; i < 20; i++) { poke("wheel", "800", "400", "up"); await sleep(80); }
  await sleep(1000);
  const a0 = shot("away");
  send("turn.start", { thread: th, text: "go d", msg: "h-d" });
  const during: string[] = [];
  const end = Date.now() + 30000;
  while (Date.now() < end) {
    await sleep(400);
    during.push(shot("stream"));
    if (replies() >= 4) break;
  }
  await sleep(800);
  const a1 = shot("landed");
  console.log(JSON.stringify({ out, a0, a1, during: during.length }));
  check(replies() >= 4 && during.length > 2, "the window received the complete streamed reply");
  // a band of the timeline compared pixel for pixel (python, PIL)
  const cmp = Bun.spawnSync(["python3", "-I", "-c", `
import sys
from PIL import Image, ImageChops
a = Image.open(sys.argv[1]).convert("RGB")
box = tuple(int(v) for v in "${opt("box", "420,200,1050,500")}".split(","))
assert len(a.crop(box).getcolors(1000000)) > 3, "the timeline band contains no text"
failed = 0
for f in sys.argv[2:]:
    b = Image.open(f).convert("RGB")
    d = ImageChops.difference(a.crop(box), b.crop(box)).getbbox()
    print(f, "same" if d is None else "differs %s" % (d,))
    failed += d is not None
sys.exit(1 if failed else 0)
`, a0, ...during, a1]);
  console.log(cmp.stdout.toString(), cmp.stderr.toString());
  check(cmp.exitCode === 0, "every streamed and landed window frame keeps the reading band pixel-identical");
  // the pill (--pill X,Y) back to the end, then a reply followed there
  const pill = opt("pill", "");
  if (pill) {
    const [px, py] = pill.split(",");
    poke("click", px, py);
    await sleep(800);
    const bottom = shot("bottom");
    console.log("pill clicked:", bottom);
    const gone = Bun.spawnSync(["python3", "-I", "-c", `
from PIL import Image
import sys
x, y = ${Number(px)}, ${Number(py)}
a = Image.open(sys.argv[1]).convert("RGB").crop((x-30,y-10,x+30,y+10))
assert len(a.getcolors(10000)) == 1, "pill still drawn at the bottom"
`, bottom]);
    check(gone.exitCode === 0, "the pill disappears after clicking back to the end");
    send("turn.start", { thread: th, text: "go e", msg: "h-e" });
    await sleep(1500);
    console.log("following:", shot("follow-mid"));
    check(!!(await until(30000, () => replies() >= 5 ? true : undefined)), "the followed window reply lands");
    await sleep(800);
    console.log("followed:", shot("follow-end"));
  }
  ws.close();
} finally {
  proc.kill();
  await proc.exited;
  if (xvfb) { xvfb.kill(); await xvfb.exited; }
  rmSync(root, { recursive: true, force: true });
}

console.log(fail ? `${fail} failed` : "all passed");
process.exit(fail ? 1 : 0);
