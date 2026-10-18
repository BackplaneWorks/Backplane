// CPU of the helper and its Chrome while nobody watches an animating page,
// and how many frames it writes: bun tools/browser/test/idle_cpu.ts [SECONDS]
// ($BACKPLANE_BROWSER, else src/main.ts). Then a client shows it (cast) for
// as long: frames must flow again.
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const secs = Number(process.argv[2] ?? 20);
const dir = mkdtempSync(join(tmpdir(), "bp-idle-cpu-"));
const cmd = process.env.BACKPLANE_BROWSER ? [process.env.BACKPLANE_BROWSER] : [process.execPath, resolve(import.meta.dir, process.env.BROWSER_TS ?? "../src/main.ts")];
const p = Bun.spawn([...cmd, "--profile", join(dir, "prof"), "--jpeg-dir", join(dir, "shots"), "--frames", join(dir, "f.rgb")], { stdin: "pipe", stdout: "pipe", stderr: "ignore" });
let frames = 0;
const waits = new Map<number, (o: any) => void>();
(async () => {
  let buf = "";
  for await (const chunk of p.stdout) {
    buf += new TextDecoder().decode(chunk);
    let i;
    while ((i = buf.indexOf("\n")) >= 0) {
      const line = buf.slice(0, i); buf = buf.slice(i + 1);
      try { const o = JSON.parse(line); if (o.event === "frame" && o.page === "anim") frames++; if (o.id && waits.has(o.id)) { waits.get(o.id)!(o); waits.delete(o.id); } } catch {}
    }
  }
})();
let id = 0;
const ask = (o: object) => new Promise<any>((r) => { const n = ++id; waits.set(n, r); p.stdin.write(JSON.stringify({ id: n, ...o }) + "\n"); p.stdin.flush(); });
const html = "<html><body style='margin:0'><canvas id=c width=800 height=600></canvas><script>let t=0;const x=c.getContext('2d');(function f(){t++;x.fillStyle='hsl('+(t%360)+',80%,50%)';x.fillRect(0,0,800,600);x.fillStyle='#000';x.font='60px sans';x.fillText(String(t),50,300);requestAnimationFrame(f)})()</script></body></html>";
const tree = (root: number): number[] => {
  const kids = new Map<number, number[]>();
  for (const d of readdirSync("/proc")) {
    if (!/^\d+$/.test(d)) continue;
    try { const st = readFileSync(`/proc/${d}/stat`, "utf8"); const pp = Number(st.slice(st.lastIndexOf(")") + 2).split(" ")[1]); (kids.get(pp) ?? kids.set(pp, []).get(pp)!).push(Number(d)); } catch {}
  }
  const out = [root]; for (let i = 0; i < out.length; i++) out.push(...(kids.get(out[i]) ?? []));
  return out;
};
const ticks = (pids: number[]) => pids.reduce((a, q) => { try { const f = readFileSync(`/proc/${q}/stat`, "utf8").slice(readFileSync(`/proc/${q}/stat`, "utf8").lastIndexOf(")") + 2).split(" "); return a + Number(f[11]) + Number(f[12]); } catch { return a; } }, 0);
const measure = async (s: number) => { const a = ticks(tree(p.pid)); const f0 = frames; await Bun.sleep(s * 1000); const b = ticks(tree(p.pid)); return { cpu: ((b - a) / 100 / s) * 100, fps: (frames - f0) / s }; };
const r = await ask({ op: "open", page: "anim", url: "data:text/html," + encodeURIComponent(html), width: 800, height: 600 });
if (!r.ok) { console.log("open failed", r.error); process.exit(1); }
await Bun.sleep(3000);
const idle = await measure(secs);
console.log(`unwatched: ${idle.cpu.toFixed(1)}% CPU (helper + Chrome), ${idle.fps.toFixed(1)} frames/s written`);
const lease = setInterval(() => void ask({ op: "cast", page: "anim", ms: 2500 }), 1000);
await ask({ op: "cast", page: "anim", ms: 2500 });
await Bun.sleep(1000);
const shown = await measure(Math.min(secs, 10));
clearInterval(lease);
console.log(`shown:     ${shown.cpu.toFixed(1)}% CPU, ${shown.fps.toFixed(1)} frames/s`);
await Bun.sleep(5000);
const after = await measure(Math.min(secs, 10));
console.log(`after:     ${after.cpu.toFixed(1)}% CPU, ${after.fps.toFixed(1)} frames/s`);
p.stdin.end();
await p.exited;
rmSync(dir, { recursive: true, force: true });
