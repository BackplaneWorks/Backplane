// A download the person starts on the page (no agent `download` op waiting) is saved by the helper and announced as a `download` event
// (page, name, path, bytes) for the hub to hand to the window: bun test/person-download.ts (BACKPLANE_BROWSER: a built helper, else src/main.ts)
import { readFileSync } from "node:fs";
const body = "person download\n";
const srv = Bun.serve({ port: 0, hostname: "127.0.0.1", fetch(req) {
  const u = new URL(req.url);
  if (u.pathname === "/file.txt") return new Response(body, { headers: { "content-type": "text/plain", "content-disposition": "attachment; filename=\"file.txt\"" } });
  return new Response('<!doctype html><a id=d href="/file.txt" download>get</a>', { headers: { "content-type": "text/html" } });
} });
const cmd = process.env.BACKPLANE_BROWSER ? [process.env.BACKPLANE_BROWSER] : ["bun", new URL("../src/main.ts", import.meta.url).pathname];
const proc = Bun.spawn(cmd, { stdin: "pipe", stdout: "pipe", stderr: "inherit" });
const waiting = new Map<number, (v: any) => void>(); const events: any[] = []; let next = 1;
(async () => { const dec = new TextDecoder(); let buf = "";
  for await (const chunk of proc.stdout) { buf += dec.decode(chunk); let nl;
    while ((nl = buf.indexOf("\n")) >= 0) { const m = JSON.parse(buf.slice(0, nl)); buf = buf.slice(nl + 1); if (m.event) events.push(m); else waiting.get(m.id)?.(m); } } })();
const call = (op: string, a: Record<string, unknown> = {}) => new Promise<any>((res) => { const id = next++; waiting.set(id, res); proc.stdin.write(JSON.stringify({ id, op, ...a }) + "\n"); proc.stdin.flush(); });
let failed = 0; const check = (what: string, ok: boolean, info = "") => { console.log(`${ok ? "PASS" : "FAIL"} ${what}${info ? ": " + info : ""}`); if (!ok) failed++; };
try {
  const o = await call("open", { page: "p1", url: `http://127.0.0.1:${srv.port}/` }); check("the page opened", o.ok !== false, JSON.stringify(o).slice(0, 200));
  await call("click", { page: "p1", selector: "#d" });
  let ev; for (let i = 0; i < 100 && !(ev = events.find((e) => e.event === "download")); i++) await Bun.sleep(100);
  check("a download event came", !!ev, JSON.stringify(ev ?? {}));
  check("it names the page and the file", ev?.page === "p1" && ev?.name === "file.txt");
  check("the file holds what the page sent", !!ev?.path && readFileSync(ev.path, "utf8") === body && ev.bytes === body.length);
} finally { try { proc.stdin.end(); } catch {} proc.kill(); srv.stop(); }
process.exit(failed ? 1 : 0);
