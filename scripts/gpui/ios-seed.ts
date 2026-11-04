// Seed a throwaway hub for the iOS checks: usage: bun ios-seed.ts HOST:PORT HOMEDIR PROJECTDIR
const [hp, home, proj] = process.argv.slice(2);
const tok = (await Bun.file(`${home}/token`).text()).trim();
const ws = new WebSocket(`ws://${hp}/ws?token=${tok}`);
let id = 0;
const seen: any[] = [];
ws.onmessage = (e) => {
  try { const o = JSON.parse(typeof e.data === "string" ? e.data : new TextDecoder().decode(e.data as ArrayBuffer));
    for (const c of o?.items ?? []) seen.push(typeof c === "string" ? JSON.parse(c) : c); console.log("frame", Object.keys(o).join(",")); } catch { const u = new Uint8Array(e.data as ArrayBuffer); if (u.length < 80) console.log("bin", new TextDecoder().decode(u).replace(/[^ -~]/g, ".")); else console.log("binary frame", u.length); }
};
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
// a client frame: CBOR map {id, m, p} with literal text keys; p's values are strings
const hd = (maj: number, n: number) => n < 24 ? [maj + n] : n < 256 ? [maj + 24, n] : [maj + 25, n >> 8, n & 255];
const txt = (t: string) => { const b = new TextEncoder().encode(t); return [...hd(0x60, b.length), ...b]; };
const val = (v: any): number[] => typeof v === "number" ? hd(0x00, v) : txt(String(v));
const mapOf = (o: Record<string, any>): number[] => [...hd(0xa0, Object.keys(o).length), ...Object.entries(o).flatMap(([k, v]) => [...txt(k), ...(v && typeof v === "object" ? mapOf(v) : val(v))])];
const send = (m: string, p: object) => ws.send(new Uint8Array(mapOf({ id: ++id, m, p })));
await Bun.sleep(1500);
const th = process.argv[5];
if (th) { // turns: bun ios-seed.ts HP HOME PROJ THREAD "text" ...
  let n = 0; for (const t of process.argv.slice(6)) { send("turn.start", { thread: th, text: t, msg: `seed-${Date.now()}-${n++}` }); await Bun.sleep(9000); }
  process.exit(0);
}
send("project.add", { path: proj });
await Bun.sleep(3000);
console.log(seen.map((x) => x.$));
process.exit(0);
