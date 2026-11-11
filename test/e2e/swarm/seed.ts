// Seeds swarms on a throwaway hub over data/1, for the phones' swarm screenshots (test/e2e/swarm/android-shots.sh).
// Members are stand-in agents (test/tools/claude_standin.ts, BP_STANDIN_MODE=plain): a member posts on the board when it is told
// `mcpcall:{room_post…}`, so every post here is the hub's own path (claims take locks, a second claim of a file waits, a pause holds).
//   bun seed.ts BASE TOKENFILE STEP     STEP: board (two live swarms with posts) | synthesis (a member posts the synthesis of "Footprint check")
const [base, tokFile, step] = process.argv.slice(2);
const tok = (await Bun.file(tokFile).text()).trim();
const ws = new WebSocket(base.replace(/^http/, "ws") + `/ws?data=1&enc=json&token=${tok}`);
let id = 0; const waiting = new Map<number, (v: any) => void>();
ws.onmessage = (m) => { const o = JSON.parse(String(m.data)); if (o.t === "reply") { const f = waiting.get(o.id); waiting.delete(o.id); f?.(o); } };
await new Promise((r) => (ws.onopen = r));
async function call(m: string, p: any = {}): Promise<any> {
  const n = ++id; const got = new Promise<any>((r) => waiting.set(n, r)); ws.send(JSON.stringify({ id: n, m, p })); const o = await got;
  if (!o.ok) throw new Error(`${m}: ${JSON.stringify(o.error)}`); return o.result;
}
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function members(room: string) { return (await call("swarm.get", { room })).members as any[]; }
async function say(room: string, who: string, args: any) {
  const m = (await members(room)).find((x) => x.name === who); if (!m) throw new Error(`no member ${who}`);
  const before = (await call("swarm.get", { room })).posts.length;
  await call("message.send", { thread: m.thread, text: "mcpcall:" + JSON.stringify({ name: "room_post", arguments: { room, ...args } }) });
  for (let i = 0; i < 40; i++) { await sleep(500); if ((await call("swarm.get", { room })).posts.length > before) return; }
  throw new Error(`${who}'s post did not arrive`);
}
const project = (await call("projects.list", { limit: 10 })).projects[0].id;
const find = async (name: string) => (await call("swarm.list", { project })).swarms.find((s: any) => s.name === name)?.id as string;

if (step === "board") {
  const a = await call("swarm.create", { project, topic: "Encoder debounce", goal: "A debounced rotary encoder driver, with tests, that never skips a detent.",
    members: [{ provider: "claude", model: "opus", role: "designer" }, { provider: "claude", model: "sonnet", role: "critic" }, { provider: "claude", model: "haiku", role: "tester" }],
    seed: "The encoder on the front panel bounces: src/encoder.rs counts two steps per detent at speed." });
  const r = a.room; await sleep(4000);
  await say(r, "designer", { text: "Integrate the A/B lines over 2 ms and step on the integrator's edges: bounce under 1 ms never reaches the state machine.", tags: ["idea"] });
  await say(r, "critic", { text: "@designer 2 ms is fine at hand speed, but a fast spin gives 1.5 ms between edges. Hysteresis on the integrator, not a longer window.", tags: ["critique"] });
  await say(r, "designer", { text: "Taking the integrator and its hysteresis.", tags: ["claim"], files: ["src/debounce.rs"] });
  await say(r, "tester", { text: "I'll write the table test for fast spins against the same module.", tags: ["claim"], files: ["src/debounce.rs", "tests/debounce.rs"] });
  await call("swarm.post", { room: r, text: "Keep the encoder: no hall sensor on this board. Tester, wait for designer's done.", clientId: "seed-1" });
  await say(r, "tester", { text: "What is the fastest spin we must count? I'll use 20 detents per second unless someone knows better.", tags: ["question"] });
  await call("swarm.act", { room: r, action: "pause" }); await sleep(800);
  await say(r, "critic", { text: "20 per second is generous; the old firmware assumed 12.", tags: ["critique"] });
  const b = await call("swarm.create", { project, topic: "Footprint check", goal: "Every footprint on the main board checked against its datasheet.",
    members: [{ provider: "claude", role: "librarian" }, { provider: "claude", model: "haiku", role: "checker" }], seed: "Check the footprints of the main board." });
  await sleep(4000);
  await say(b.room, "librarian", { text: "U3's datasheet gives a 0.5 mm pitch; the footprint has 0.65 mm. Fixing it.", tags: ["claim"], files: ["lib/qfn32.kicad_mod"] });
  await say(b.room, "checker", { text: "Checked the other 41: pads, courtyard and pin 1 match.", tags: ["done"] });
  console.log("seeded", r, b.room);
} else if (step === "synthesis") {
  const r = await find("Footprint check");
  await say(r, "librarian", { text: "Pitch fixed.", tags: ["done"] });
  await say(r, "checker", { text: "Synthesis: 42 footprints checked. U3 (QFN-32) had a 0.65 mm pitch where the datasheet gives 0.5 mm; fixed in lib/qfn32.kicad_mod. Nothing else differs.", tags: ["synthesis"] });
  console.log("synthesis", r);
}
ws.close(); process.exit(0);
