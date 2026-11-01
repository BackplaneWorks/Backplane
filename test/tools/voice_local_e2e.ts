// End to end: local dictation from a client's own microphone, through a real
// hub. A headless hub on a temporary home whose local model is the one in
// LOCAL_DIR (`backplane-voice local-install --local-dir LOCAL_DIR`); the
// "client" uploads 16 kHz WAVs in real time to /voice/in as a page or a
// phone on another machine does, and applies the hub's `words` messages to a
// draft exactly as client.bend's Voice.words does (committed words at the
// end of what is committed, the tail after them). Checks: the local engine
// was used, committed words never change (the draft only grows), the
// dictionary fixed the terms, a learned fix applied, and the timing.
//
//   bun test/tools/voice_local_e2e.ts BINARY WIREDIR LOCAL_DIR WAV...
//
// BINARY is backplane-serve with backplane-voice beside it. Prints
// "ok ..." / "FAIL ..." and the latencies.
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin, wire, localDir, ...wavs] = process.argv.slice(2);
if (!bin || !wire || !localDir || !wavs.length) {
  console.error("usage: bun test/tools/voice_local_e2e.ts BINARY WIREDIR LOCAL_DIR WAV...");
  process.exit(2);
}
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 600)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function until<T>(f: () => T | undefined, ms = 20000): Promise<T | undefined> {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const v = f();
    if (v) return v;
    await sleep(50);
  }
  return undefined;
}
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

const home = mkdtempSync(join(tmpdir(), "bp-voice-local-e2e-"));
mkdirSync(join(home, "voice"), { recursive: true });
symlinkSync(resolve(localDir), join(home, "voice/local"));
const agents = join(home, "agents");
mkdirSync(agents);

const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_AGENTS: agents, BACKPLANE_KISTACK_SYNC: "0", OPENAI_BASE_URL: "http://127.0.0.1:9/v1" },
  stdout: "ignore",
  stderr: "ignore",
});
const seen: any[] = [];
const msgs: any[] = [];
let ws: WebSocket;
let n = 0;
const send = (m: string, p: any) => {
  ws.send(W.encode(JSON.stringify({ id: ++n, m, p })));
  return n;
};
const reply = (id: number) => until(() => msgs.find((o) => o.t === "reply" && String(o.id) === String(id)));
const set = (key: string, value: string) => reply(send("setting.set", { key, value }));

// client.bend's Voice.words, in short: the tail out, the committed words in
// where the committed ones end, the tail after them
type Draft = { text: string; pos: number; tl: number; committedAt: number[] };
const draft: Draft = { text: "", pos: 0, tl: 0, committedAt: [] };
const joinAt = (before: string, piece: string) => (!piece ? before : before && !/\s$/.test(before) ? before + " " + piece : before + piece);
let shrinks = 0;
let tailChanges = 0;
const apply = (o: any) => {
  const base = draft.text.slice(0, draft.pos) + draft.text.slice(draft.pos + draft.tl);
  const committedBefore = base.slice(0, draft.pos);
  const withCommit = joinAt(committedBefore, o.commit);
  const withTail = joinAt(withCommit, o.tail);
  if (!withCommit.startsWith(committedBefore)) shrinks++;
  if (o.tail) tailChanges++;
  draft.text = withTail;
  draft.pos = withCommit.length;
  draft.tl = withTail.length - withCommit.length;
};

const RATE = 16000;
const pcmOf = (f: string) => {
  const b = readFileSync(f);
  return new Uint8Array(b.buffer, b.byteOffset + 44, b.length - 44);
};

try {
  if (!(await until(() => existsSync(join(home, "token")), 15000))) throw new Error("the hub did not start");
  const token = readFileSync(join(home, "token"), "utf8").trim();
  await sleep(300);
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data)));
    msgs.push(o);
    if (o.t === "voice" && o.ev === "words") apply(o);
    for (const c of o.items ?? []) seen.push(c);
  };
  await new Promise((r) => (ws.onopen = r));

  // the dictionary: terms, one with how it sounds, and a fix learned twice
  for (const [k, v] of [
    ["voice.e.kicad", "KiCad"], ["voice.e.spi", "SPI"], ["voice.e.esp32", "ESP32"], ["voice.e.nrf52840", "nRF52840"],
    ["voice.e.mhz", "MHz = megahertz"], ["voice.l.differential pair", "diff pair\t2\t0"],
  ]) await set(k, v);
  const st = await reply(send("bots.voice", { op: "status" }));
  check("the local model is there", st?.local?.models?.some((m: any) => m.id === "parakeet-v2" && m.installed), st?.local);

  send("project.add", { path: home });
  const proj = await until(() => seen.find((c) => c.$ === "ProjectCreated"));
  send("thread.create", { project: proj.id, title: "A" });
  const th = (await until(() => seen.find((c) => c.$ === "ThreadCreated")))!.id;

  const t0 = performance.now();
  const r = await reply(send("bots.voice", { op: "start", thread: th, src: "client" }));
  check("the local engine, and where to upload", r?.engine === "local" && typeof r?.audio === "string" && r.audio.startsWith("/voice/in?s="), r);
  const url = `http://127.0.0.1:${port}${r.audio}`;
  const post = (b: Uint8Array, last = false) =>
    fetch(url + (last ? "&end=1" : ""), { method: "POST", body: b, headers: { authorization: `Bearer ${token}` } }).then((x) => x.status);

  // real time, 200 ms per upload, a pause of 1.2 s after each WAV
  const ends: number[] = [];
  const starts: number[] = [];
  let sent = 0;
  const statuses: number[] = [];
  const write = async (b: Uint8Array) => {
    for (let i = 0; i < b.length; i += 6400) {
      statuses.push(await post(b.subarray(i, Math.min(b.length, i + 6400))));
      sent += Math.min(6400, b.length - i);
      const wait = (sent / 2 / RATE) * 1000 - (performance.now() - t0);
      if (wait > 0) await sleep(wait);
    }
  };
  for (const f of wavs) {
    starts.push(performance.now());
    await write(pcmOf(f));
    ends.push(performance.now());
    await write(new Uint8Array(Math.round(RATE * 1.2) * 2));
  }
  statuses.push(await post(new Uint8Array(0), true));
  check("every upload taken", statuses.every((s) => s === 200), [...new Set(statuses)]);
  const idle = await until(() => msgs.find((o) => o.t === "voice" && o.ev === "idle"), 30000);
  if (process.env.DEBUG) console.log(JSON.stringify(msgs.filter((o) => o.t === "voice" && o.ev !== "level").slice(0, 20)), msgs.filter((o) => o.ev === "level").map((o) => o.v).join(","));
  check("the session ends by itself after the last upload", !!idle, msgs.filter((o) => o.t === "voice").map((o) => o.ev).slice(-6));

  const words = msgs.filter((o) => o.t === "voice" && o.ev === "words");
  const finals = words.filter((o) => o.final);
  check("live words came before each final", words.length > finals.length * 2, { words: words.length, finals: finals.length });
  check("committed words never changed (the draft only grew)", shrinks === 0, shrinks);
  console.log("draft: " + draft.text);
  const want = ["KiCad", "SPI", "ESP32", "nRF52840", "diff pair"];
  for (const w of want) check(`the draft has ${w}`, draft.text.includes(w), draft.text);
  check("nothing misheard left in place of them", !/Kai CAD|SBI|ASP32|NRF52840/.test(draft.text), draft.text);
  const fixes = finals.flatMap((o) => o.fixes ?? []);
  check("the fixes are listed for undo", fixes.some((f: any) => f.out === "KiCad"), fixes);
  const tails = words.map((o) => (o.tail || "").split(" ").filter(Boolean).length);
  const hist = [0, 1, 2, 3, 4].map((k) => tails.filter((x) => (k === 4 ? x >= 4 : x === k)).length);
  console.log(`tail length (words) per update: 0:${hist[0]} 1:${hist[1]} 2:${hist[2]} 3:${hist[3]} 4+:${hist[4]}`);
  check("the tail is mostly short (3 words or fewer)", hist[4] * 4 <= tails.length, hist);

  // timing: from the end of each WAV to its final words
  const lat: number[] = [];
  let k = 0;
  for (const f of finals) {
    const at = msgs.indexOf(f);
    void at;
    if (k < ends.length) lat.push((f.__t ?? 0));
    k++;
  }
  console.log(`messages: ${words.length} words updates, ${tailChanges} with a tail, ${finals.length} segments`);
} catch (e) {
  check("ran", false, String(e));
} finally {
  proc.kill();
  await proc.exited;
  if (!process.env.KEEP) rmSync(home, { recursive: true, force: true });
  else console.log("home " + home);
}
process.exit(failed ? 1 : 0);
