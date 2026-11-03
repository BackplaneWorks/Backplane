// Mentions end to end against a running hub: two projects, three threads,
// two bots and a room (Tofu in it); a message in a thread naming `@Miso`, `>thread` and
// `#room` must wake the bot and post in the room, once. The hub should run
// with no agent CLIs on its PATH (the woken turns then fail at once).
// Usage: bun test/tools/refs_e2e.ts PORT HOME WIREDIR   (WIREDIR from
// `bend test/wire/index.html -o build/wire`)
import { mkdirSync, readdirSync, readFileSync } from "fs";
const hubToken = (dir: string): string => require('node:fs').readFileSync(dir + '/token', 'utf8').trim();
const [port, home, wire] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const fail = (m: string) => { console.error("refs_e2e: " + m); process.exit(1); };
for (const d of ["bend", "board"]) mkdirSync(`${home}/${d}`, { recursive: true });

const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${hubToken(home)}`);
ws.binaryType = "arraybuffer";
const seen: any[] = [];
let n = 0;
const send = (m: string, p: any) => ws.send(W.encode(JSON.stringify({ id: ++n, m, p })));
const all = (k: string) => seen.filter((c) => c.$ === k);
let step = 0;
const done = new Promise<void>((resolve) => {
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data)));
    for (const c of o.items ?? []) seen.push(c);
    if (o.t === "log" && step === 0) { step = 1; send("project.add", { path: `${home}/bend` }); send("project.add", { path: `${home}/board` }); }
    const ps = all("ProjectCreated");
    if (step === 1 && ps.length === 2) {
      step = 2;
      send("thread.create", { project: ps[0].id, title: "Fix perf lag" });
      send("thread.create", { project: ps[0].id, title: "Refs popup" });
      send("thread.create", { project: ps[1].id, title: "Board bringup" });
      send("bots.create", { name: "Miso", provider: "claude", persona: "Keeps the parts library tidy" });
      send("bots.create", { name: "Tofu", provider: "codex" });
    }
    const bots = all("BotSet");
    if (step === 2 && all("ThreadCreated").length >= 5 && bots.length === 2) {
      step = 3;
      send("bots.room", { name: "Hardware talk", members: bots[1].id });
    }
    if (step === 3 && all("RoomSet").length === 1) {
      step = 4;
      const t = all("ThreadCreated").find((c) => c.title === "Refs popup");
      send("turn.start", { thread: t.id, text: "@Miso look at >fix-perf-lag and tell #hardware-talk", msg: "e2e-1" });
      setTimeout(() => send("turn.start", { thread: t.id, text: "@Miso look at >fix-perf-lag and tell #hardware-talk", msg: "e2e-1" }), 300);
      setTimeout(resolve, 1500);
    }
  };
});
ws.onerror = () => fail("websocket failed");
await Promise.race([done, new Promise((_, rej) => setTimeout(() => rej(new Error("timeout at step " + step)), 10000))]).catch((e) => fail(String(e)));
ws.close();
const woke = all("BotWoke"), posted = all("RoomPosted");
const [miso, tofu] = all("BotSet").map((c) => c.id);
if (woke.filter((c) => c.bot === miso).length !== 1) fail(`Miso woken ${woke.length} times, want once`);
if (woke.filter((c) => c.bot === tofu).length !== 1) fail("Tofu (in the room) not woken once");
if (posted.length !== 1 || !String(posted[0].text).includes("[from the thread \"Refs popup\"")) fail("room post missing: " + JSON.stringify(posted));
const inbox = all("MessagePosted").find((c) => String(c.text).startsWith("[the person mentioned you in the thread \"Refs popup\""));
if (!inbox) fail("Miso's inbox message missing");
console.log("refs_e2e: ok (Miso woken once by the mention, Tofu once by the room post)");
process.exit(0);
