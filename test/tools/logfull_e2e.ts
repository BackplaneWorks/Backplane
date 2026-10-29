// A failed append is taken back. Starts a headless hub whose process may
// not grow a file past 96 KB (ulimit -f, SIGXFSZ ignored, so a write that
// crosses it fails), then sets big settings until a write fails. Checks the
// failure is told to the client, the change is not folded, the hub keeps
// serving, and the log on disk is only whole lines, exactly the changes
// the client was shown (no torn fragment, no change written but unfolded).
//
//   bun test/tools/logfull_e2e.ts [BINARY] [WIREDIR]
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-logfull-"));
const home = join(root, "home");
mkdirSync(home);
const logPath = join(home, "events.jsonl");
const limitKb = 96;

function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};

const port = freePort();
const proc = Bun.spawn(
  ["/bin/bash", "-c", `ulimit -f ${limitKb}; trap '' XFSZ; exec "$0" --home "$1" --port "$2" --no-tailscale`, resolve(bin), home, String(port)],
  { env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "" }, stdout: "ignore", stderr: "ignore" },
);
try {
  for (let i = 0; i < 200; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  const items: any[] = [];
  const other: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    if (!o) return;
    for (const c of o.items ?? []) items.push(c);
    if (!o.items) other.push(o);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));

  const big = "v".repeat(10000);
  let failedAt = -1;
  for (let i = 0; i < 30 && failedAt < 0; i++) {
    send("setting.set", { key: `k${i}`, value: big });
    await sleep(400);
    if (other.some((o) => JSON.stringify(o).includes("log write failed") || JSON.stringify(o).includes("could not be written"))) failedAt = i;
  }
  check(failedAt >= 0, `a write failed past the limit (step ${failedAt})`);
  check(!items.some((c) => c.$ === "SettingSet" && c.key === `k${failedAt}`), "the failed change was not folded");
  const size = statSync(logPath).size;
  check(size <= limitKb * 1024, `the log is within the limit (${size})`);
  const text = readFileSync(logPath, "utf8");
  const lines = text.split("\n").filter(Boolean);
  let whole = true;
  for (const l of lines) { try { JSON.parse(l); } catch { whole = false; } }
  check(text.length === 0 || text.endsWith("\n"), "the log ends on a whole line");
  check(whole, "every line of the log is whole");
  const sets = items.filter((c) => c.$ === "SettingSet" && /^k\d+$/.test(c.key)).length;
  const onDisk = lines.filter((l) => /"k\d+"/.test(l)).length;
  check(sets === onDisk, `the log holds exactly what was shown (${sets} shown, ${onDisk} on disk)`);
  check((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200, "the hub keeps serving");
  ws.close();
} finally {
  proc.kill(9);
  await proc.exited;
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
