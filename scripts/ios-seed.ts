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
if (th === "room") { // bun ios-seed.ts HP HOME PROJ room: a room "crew" with every bot, then two posts that mention bots
  const evs = (await Bun.file(`${home}/events.jsonl`).text()).split("\n").map((l) => { try { return JSON.parse(l); } catch { return {}; } });
  const bots = evs.filter((e) => e.$ === "BotSet");
  const names = [...new Map(bots.map((b) => [b.name, b.id])).entries()];
  send("bots.room", { room: "", name: "crew", members: names.map(([, id]) => id).join(",") });
  await Bun.sleep(2500);
  const room = (await Bun.file(`${home}/events.jsonl`).text()).split("\n").map((l) => { try { return JSON.parse(l); } catch { return {}; } }).filter((e) => e.$ === "RoomSet" || e.$ === "RoomCreated").map((e) => e.id).pop();
  console.log("room", room);
  for (const t of [`@${names[0][0]} hello crew`, `@${names[1][0]} and @${names[2][0]} say hi`]) { send("bots.post", { room, text: t }); await Bun.sleep(7000); }
  process.exit(0);
}
if (th === "bulk") { // bun ios-seed.ts HP HOME PROJ bulk FROM TO [TURNS]: threads "bulk i" for i in FROM..TO, a tool turn in the first TURNS of them
  const pid = (await Bun.file(`${home}/events.jsonl`).text()).split("\n").map((l) => { try { return JSON.parse(l); } catch { return {}; } }).find((e) => e.$ === "ProjectCreated")?.id;
  const [from, to, turns = 0] = process.argv.slice(6).map(Number);
  for (let i = from; i <= to; i++) { send("thread.create", { project: pid, title: `bulk ${i}`, env: "local", provider: "claude" }); await Bun.sleep(60); }
  await Bun.sleep(3000);
  const evs = (await Bun.file(`${home}/events.jsonl`).text()).split("\n").map((l) => { try { return JSON.parse(l); } catch { return {}; } }).filter((e) => e.$ === "ThreadCreated" && /^bulk \d+$/.test(e.title));
  const ids = evs.filter((e) => { const n = Number(e.title.slice(5)); return n >= from && n <= to; }).map((e) => e.id);
  for (const t of ids.slice(0, turns)) { send("turn.start", { thread: t, text: `tools:2 hello ${t}`, msg: `b-${t}` }); await Bun.sleep(2500); }
  console.log(ids.join(" ")); process.exit(0);
}
if (th === "new") { // bun ios-seed.ts HP HOME PROJ new N: N threads, each with a short tool turn
  const pid = (await Bun.file(`${home}/events.jsonl`).text()).split("\n").map((l) => { try { return JSON.parse(l); } catch { return {}; } }).find((e) => e.$ === "ProjectCreated")?.id;
  const n = Number(process.argv[6]); const before = new Set<string>();
  for (let i = 0; i < n; i++) {
    send("thread.create", { project: pid, title: `load ${i + 1}`, env: "local", provider: "claude" });
    await Bun.sleep(1200);
    const ids = (await Bun.file(`${home}/events.jsonl`).text()).split("\n").map((l) => { try { return JSON.parse(l); } catch { return {}; } }).filter((e) => e.$ === "ThreadCreated" && e.title === `load ${i + 1}`);
    const t = ids[0]?.id; if (!t) continue; before.add(t);
    send("turn.start", { thread: t, text: `tools:2 load thread ${i + 1}`, msg: `load-${i}` });
    await Bun.sleep(2500);
  }
  console.log([...before].join(" ")); process.exit(0);
}
if (th) { // turns: bun ios-seed.ts HP HOME PROJ THREAD "text" ...
  let n = 0; for (const t of process.argv.slice(6)) { send("turn.start", { thread: th, text: t, msg: `seed-${Date.now()}-${n++}` }); await Bun.sleep(9000); }
  process.exit(0);
}
send("project.add", { path: proj });
await Bun.sleep(3000);
console.log(seen.map((x) => x.$));
process.exit(0);
