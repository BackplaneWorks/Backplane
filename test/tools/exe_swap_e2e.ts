// End to end: a hub whose binary was replaced while it runs (an install or
// update renames a new one over it and the restart waits) still plots for
// phones. Linux then reads /proc/self/exe as "<path> (deleted)"; the plot
// children used to be spawned from that string and failed with "No such
// file or directory" (src/core/exe.bend). Starts a headless hub from a copy
// of BINARY on a temporary home and a free port, swaps the copy by rename,
// and watches a schematic and a board as a phone does. Prints "ok ..." /
// "FAIL ...".
//
//   bun test/tools/exe_swap_e2e.ts SCH PCB [BINARY] [WIREDIR]
//
// SCH and PCB are a .kicad_sch and a .kicad_pcb (copied into a temporary
// project); BINARY defaults to build/backplane, WIREDIR to build/wire
// (bend test/wire/index.html -o build/wire).
import { copyFileSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, readlinkSync, renameSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [sch, pcb, bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
if (!sch || !pcb) throw new Error("usage: bun test/tools/exe_swap_e2e.ts SCH PCB [BINARY] [WIREDIR]");
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

// a plot frame: a map whose first key is 1 ("t") and value "plot"
const isPlot = (b: Uint8Array) =>
  b.length > 7 && b[0] >= 0xa0 && b[0] <= 0xb7 && b[1] === 1 && b[2] === 0x64 &&
  b[3] === 0x70 && b[4] === 0x6c && b[5] === 0x6f && b[6] === 0x74;

// the plot's key, whether it says why there is none, and whether it has chunks
// (enough CBOR for a plot's head: text keys and values, ints, arrays, maps)
function head(b: Uint8Array): { key: string; none?: string; cs: number } {
  let i = 0;
  const arg = (ai: number) => {
    if (ai < 24) return ai;
    if (ai === 24) return b[i++];
    if (ai === 25) { const v = (b[i] << 8) | b[i + 1]; i += 2; return v; }
    if (ai === 26) { const v = ((b[i] << 24) >>> 0) + (b[i + 1] << 16) + (b[i + 2] << 8) + b[i + 3]; i += 4; return v; }
    const hi = ((b[i] << 24) >>> 0) + (b[i + 1] << 16) + (b[i + 2] << 8) + b[i + 3];
    const lo = ((b[i + 4] << 24) >>> 0) + (b[i + 5] << 16) + (b[i + 6] << 8) + b[i + 7];
    i += 8;
    return hi * 2 ** 32 + lo;
  };
  const words: Record<number, string> = { 1: "t", 31: "key" };
  const item = (): any => {
    const h = b[i++], mt = h >> 5, n = arg(h & 31);
    switch (mt) {
      case 0: return n;
      case 1: return -1 - n;
      case 2: { const s = b.subarray(i, i + n); i += n; return s; }
      case 3: { const s = new TextDecoder().decode(b.subarray(i, i + n)); i += n; return s; }
      case 4: { const a = []; for (let k = 0; k < n; k++) a.push(item()); return a; }
      case 5: { const o: any = {}; for (let k = 0; k < n; k++) { const kk = item(); o[typeof kk === "number" ? words[kk] ?? kk : kk] = item(); } return o; }
      case 7: return null;
    }
    throw new Error(`cbor major ${mt}`);
  };
  const o = item();
  return { key: o.key, none: o.none, cs: Array.isArray(o.cs) ? o.cs.length : 0 };
}

const dir = mkdtempSync(join(tmpdir(), "bp-exe-swap-"));
const home = join(dir, "home");
const root = join(dir, "proj");
mkdirSync(home);
mkdirSync(root);
copyFileSync(resolve(sch), join(root, "p.kicad_sch"));
copyFileSync(resolve(pcb), join(root, "p.kicad_pcb"));
const exe = join(dir, "backplane");
copyFileSync(resolve(bin), exe);
Bun.spawnSync(["chmod", "+x", exe]);

const port = freePort();
const proc = Bun.spawn([exe, "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, DISPLAY: "", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9" },
  stdout: "ignore",
  stderr: "ignore",
});

const plots: { key: string; none?: string; cs: number }[] = [];
let ws: WebSocket | null = null;
try {
  let token = "";
  for (let k = 0; k < 200 && !token; k++) {
    try { token = readFileSync(join(home, "token"), "utf8").trim(); } catch { await sleep(100); }
  }
  for (let k = 0; k < 100; k++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) break; } catch {}
    await sleep(100);
  }
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const b = new Uint8Array(e.data as ArrayBuffer);
    if (isPlot(b)) plots.push(head(b));
    else {
      const o = JSON.parse(W.decode(b));
      if (o.t === "plot") plots.push({ key: o.key, none: o.none, cs: Array.isArray(o.cs) ? o.cs.length : 0 });
    }
  };
  await new Promise((r) => (ws!.onopen = r));
  let id = 0;
  const watch = (kind: string) => ws!.send(W.encode(JSON.stringify({ id: ++id, m: "kicad.watch", p: { kind, root, path: "" } })));
  const next = async (key: string, ms = 30000) => {
    const end = Date.now() + ms;
    while (Date.now() < end) {
      const i = plots.findIndex((p) => p.key === key);
      if (i >= 0) return plots.splice(i, 1)[0];
      await sleep(50);
    }
    return null;
  };

  // before the swap: a schematic plots
  watch("schematic");
  const a = await next(`1|${root}|`);
  check("schematic plots before the swap", !!a && !a.none && a.cs > 0, a);

  // an install: a new file renamed over the running binary
  copyFileSync(exe, exe + ".new");
  renameSync(exe + ".new", exe);
  const link = readlinkSync(`/proc/${proc.pid}/exe`);
  check("the running hub's binary reads as deleted", link.endsWith(" (deleted)"), link);

  // after it: a fresh read of each kind spawns a child
  watch("board");
  const b = await next(`0|${root}|`);
  check("board plots after the swap", !!b && !b.none && b.cs > 0, b);
  watch("schematic");
  const c = await next(`1|${root}|`);
  check("schematic plots after the swap", !!c && !c.none && c.cs > 0, c);
} finally {
  ws?.close();
  proc.kill();
  await proc.exited;
  rmSync(dir, { recursive: true, force: true });
}
if (failed) process.exit(1);
console.log("all ok");
