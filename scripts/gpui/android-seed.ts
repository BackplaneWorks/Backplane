// Seed the Android checks' own hub. usage: bun android-seed.ts HOST:PORT HOMEDIR PROJECT_ID THREAD_COUNT [turn THREAD "text"]...
const [hp, home, project, count, ...rest] = process.argv.slice(2);
const tok = (await Bun.file(`${home}/token`).text()).trim();
const ws = new WebSocket(`ws://${hp}/ws?token=${tok}`);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
const hd = (maj: number, n: number) => n < 24 ? [maj + n] : n < 256 ? [maj + 24, n] : n < 65536 ? [maj + 25, n >> 8, n & 255] : [maj + 26, (n >>> 24) & 255, (n >> 16) & 255, (n >> 8) & 255, n & 255];
const txt = (t: string) => { const b = new TextEncoder().encode(t); return [...hd(0x60, b.length), ...b]; };
const val = (v: any): number[] => typeof v === "number" ? hd(0x00, v) : v && typeof v === "object" ? mapOf(v) : txt(String(v));
const mapOf = (o: Record<string, any>): number[] => [...hd(0xa0, Object.keys(o).length), ...Object.entries(o).flatMap(([k, v]) => [...txt(k), ...val(v)])];
let id = 0;
const send = (m: string, p: object) => ws.send(new Uint8Array(mapOf({ id: ++id, m, p })));
await Bun.sleep(1500);
if (rest[0] === "turn") {
  for (let i = 1; i + 1 < rest.length; i += 2) { send("turn.start", { thread: rest[i], text: rest[i + 1], msg: `seed-${Date.now()}-${i}` }); await Bun.sleep(6000); }
} else {
  for (let i = 1; i <= Number(count); i++) { send("thread.create", { project, title: `Seed thread ${i}` }); await Bun.sleep(150); }
  await Bun.sleep(1500);
}
process.exit(0);
