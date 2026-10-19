// End to end: dictation and a Pebble ring's recording through a real hub.
// Starts a local stand-in for OpenAI's realtime transcription socket, a
// fake `pw-record` (a tone, so no real microphone is heard), and a headless
// hub on a temporary home. Keeps a key and a dictionary, dictates into a
// thread (live, level, words, words to clarify, stop, idle), then posts a
// ring's form with a recording to a bot's webhook under its bearer token and
// checks the bot heard Backplane's own words. Prints "ok ..." / "FAIL ...".
//
//   bun test/tools/voice_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane, with build/backplane-voice beside it
// (scripts/build-voice.sh, then copy dist/backplane-voice); WIREDIR to
// build/wire (bend test/wire/index.html -o build/wire). The ring's part
// needs ffmpeg.
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 400)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function until<T>(f: () => T | undefined, ms = 20000): Promise<T | undefined> {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const v = f();
    if (v) return v;
    await sleep(100);
  }
  return undefined;
}
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

// OpenAI's realtime transcription, as far as the helper uses it: every
// committed segment is heard as "open key cad now"
const openai: any[] = [];
const mock = Bun.serve({
  port: 0,
  hostname: "127.0.0.1",
  fetch(req, srv) {
    const u = new URL(req.url);
    if (!(req.headers.get("authorization") ?? "").startsWith("Bearer sk-")) return Response.json({ error: { message: "bad key" } }, { status: 401 });
    if (u.pathname === "/v1/realtime" && srv.upgrade(req, { data: { n: 0 } })) return;
    if (u.pathname === "/v1/models") return Response.json({ data: [] });
    if (u.pathname === "/v1/audio/transcriptions") {
      openai.push({ type: "rest" });
      return Response.json({ text: "open key cad now" });
    }
    return new Response("not found", { status: 404 });
  },
  websocket: {
    message(ws: any, raw) {
      const ev = JSON.parse(String(raw));
      openai.push(ev);
      if (ev.type !== "input_audio_buffer.commit") return;
      const id = `item_${++ws.data.n}`;
      ws.send(JSON.stringify({ type: "input_audio_buffer.committed", item_id: id }));
      ws.send(JSON.stringify({ type: "conversation.item.input_audio_transcription.delta", item_id: id, delta: "open key " }));
      ws.send(JSON.stringify({ type: "conversation.item.input_audio_transcription.completed", item_id: id, transcript: "open key cad now" }));
    },
  },
});

const home = mkdtempSync(join(tmpdir(), "bp-voice-e2e-"));
// a recorder that plays a 440 Hz tone (speech-loud) for 2 s, then quiet
const fake = join(home, "fakebin");
mkdirSync(fake);
writeFileSync(join(fake, "pw-record"), `#!/usr/bin/env python3
import math, struct, sys, time
out = sys.stdout.buffer
for i in range(80):
    loud = i < 20
    out.write(b"".join(struct.pack("<h", int((12000 if loud else 30) * math.sin(2 * math.pi * 440 * (i * 2400 + k) / 24000))) for k in range(2400)))
    out.flush()
    time.sleep(0.1)
`);
chmodSync(join(fake, "pw-record"), 0o755);
// NO_FFMPEG: the hub's helper finds an ffmpeg that fails (a host without
// one), so a recording goes to gpt-transcribe over REST
if (process.env.NO_FFMPEG) {
  writeFileSync(join(fake, "ffmpeg"), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, "ffmpeg"), 0o755);
}

const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, DISPLAY: "", OPENAI_BASE_URL: `http://127.0.0.1:${mock.port}/v1`, BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: "ignore",
});
const seen: any[] = [];
const msgs: any[] = [];
const bad: string[] = [];
let ws: WebSocket;
let n = 0;
const send = (m: string, p: any) => {
  ws.send(W.encode(JSON.stringify({ id: ++n, m, p })));
  return n;
};
const reply = (id: number) => until(() => msgs.find((o) => o.t === "reply" && String(o.id) === String(id)));

try {
  if (!(await until(() => existsSync(join(home, "token")), 10000))) throw new Error("the hub did not start");
  await sleep(300);
  ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any;
    try {
      o = JSON.parse(W.decode(new Uint8Array(e.data)));
    } catch (err) {
      const raw = Buffer.from(new Uint8Array(e.data)).toString("latin1");
      console.log("UNDECODABLE", raw.slice(0, 300).replace(/[^\x20-\x7e]/g, "."));
      return;
    }
    msgs.push(o);
    for (const c of o.items ?? []) seen.push(c);
  };
  await new Promise((r) => (ws.onopen = r));

  const nokey = await reply(send("bots.voice", { op: "key", key: "not a key" }));
  check("a key that is not one is refused", nokey?.ok === false, nokey);
  const k = await reply(send("bots.voice", { op: "key", key: "sk-test-0123456789abcdefWXYZ" }));
  check("key kept, shown masked", k?.voiceKey === "sk-…WXYZ", k);
  check("key file is the owner's only", (Bun.spawnSync(["stat", "-c", "%a", join(home, "secrets/openai.key")]).stdout.toString().trim()) === "600");
  const d = await reply(send("bots.voice", { op: "dict", dict: "KiCad\n  kicad \nSTM32" }));
  check("dictionary tidied", d?.voiceDict === "KiCad\nSTM32", d);
  const st = await reply(send("bots.voice", { op: "status" }));
  check("status lists microphones", typeof st?.voiceDevices === "string" && st.voiceDevices.includes("devices"), st);
  check("the key never leaves masked", !JSON.stringify(msgs).includes("0123456789abcdef"));

  send("project.add", { path: home });
  const proj = await until(() => seen.find((c) => c.$ === "ProjectCreated"));
  send("thread.create", { project: proj.id, title: "A" });
  const th = (await until(() => seen.find((c) => c.$ === "ThreadCreated")))!.id;

  send("bots.voice", { op: "start", thread: th });
  const live = await until(() => msgs.find((o) => o.t === "voice" && o.ev === "live"));
  check("live, with its recorder", live?.recorder === "pw-record" && live?.thread === th, live);
  const lv = await until(() => msgs.find((o) => o.t === "voice" && o.ev === "level" && o.v > 50));
  check("the level shows the tone", !!lv, msgs.filter((o) => o.ev === "level").slice(0, 5));
  const text = await until(() => msgs.find((o) => o.t === "voice" && o.ev === "text"));
  check("words heard", text?.text === "open key cad now", text);
  check("a word to clarify, with a guess", text?.unsure?.[0]?.word === "key cad" && text?.unsure?.[0]?.guess === "KiCad", text?.unsure);
  const upd = openai.find((e) => e.type === "session.update");
  const tr = upd?.session?.audio?.input?.transcription;
  check("GPT-Live-Transcribe, with the dictionary as keywords", tr?.model === "gpt-live-transcribe" && JSON.stringify(tr?.keywords) === '["KiCad","STM32"]', tr);
  check("the device in the helper's file", existsSync(join(home, "voice/live.pid")));
  send("bots.voice", { op: "stop", thread: th });
  const idle = await until(() => msgs.find((o) => o.t === "voice" && o.ev === "idle"));
  check("stopped, then idle", !!idle, msgs.filter((o) => o.t === "voice").map((o) => o.ev));
  check("the pid file is gone", !existsSync(join(home, "voice/live.pid")));
  const again = await reply(send("bots.voice", { op: "stop", thread: th }));
  check("a stop with none running answers", again?.ok === true, again);

  const add = await reply(send("bots.voice", { op: "add", term: "Pebble Index" }));
  check("a clarified term joins", add?.voiceDict === "KiCad\nSTM32\nPebble Index", add);

  // a Pebble ring's voice note with its recording, under the hook's token
  const hasFfmpeg = Bun.spawnSync(["sh", "-c", "command -v ffmpeg"]).exitCode === 0;
  const b = await reply(send("bots.create", { name: "ringbot", provider: "grok" }));
  const bot = b?.bot;
  const h = await reply(send("bots.hook", { bot, name: "ring" }));
  const hook = h?.hook;
  const t = await reply(send("bots.hook_token", { hook, op: "show" }));
  check("a hook and its token", !!hook && !!t?.token, { h, t });
  if (hasFfmpeg && hook && t?.token) {
    const wav = join(home, "note.m4a");
    Bun.spawnSync(["ffmpeg", "-nostdin", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=2", "-c:a", "aac", "-f", "mp4", wav]);
    const fd = new FormData();
    // the ring set to send the recording alone: no transcription part
    if (!process.env.AUDIO_ONLY) fd.set("transcription", "open keycard now");
    fd.set("recordedAt", "1790000000000");
    fd.set("client", "ring");
    fd.set("audio", new Blob([readFileSync(wav)], { type: "audio/mp4" }), "note.m4a");
    const req = new Request("http://x/", { method: "POST", body: fd });
    // (Bun gives a multipart request's content type only before its body is read)
    const ct = req.headers.get("content-type")!;
    const body = new Uint8Array(await req.arrayBuffer());
    const r = await fetch(`http://127.0.0.1:${port}/hook/${hook}`, {
      method: "POST",
      headers: { authorization: `Bearer ${t.token}`, "content-type": ct, "x-audio-size": String(readFileSync(wav).length) },
      body,
    });
    check("the ring's post is taken at once", r.status === 202, r.status);
    const heard = await until(() => seen.find((c) => JSON.stringify(c).includes("open key cad now")), 30000);
    check("the bot gets Backplane's words", !!heard, seen.slice(-5));
    check("with the words to clarify", JSON.stringify(heard ?? "").includes("key cad"), heard);
    if (process.env.NO_FFMPEG) check("without ffmpeg, heard by gpt-transcribe over REST", openai.some((e) => e.type === "rest"));
    check("the recording is not kept", !readdirSync(join(home, "voice")).some((f) => f.startsWith("ring-")), readdirSync(join(home, "voice")));
  } else console.log("skip ring (no ffmpeg)");
  // a binary body for the bot (no form): its bytes become text a client can read
  if (hook && t?.token) {
    const r2 = await fetch(`http://127.0.0.1:${port}/hook/${hook}`, {
      method: "POST",
      headers: { authorization: `Bearer ${t.token}`, "content-type": "application/octet-stream" },
      body: new Uint8Array([98, 105, 110, 247, 191, 191, 191, 237, 160, 128, 0, 255, 10]),
    });
    check("a binary body is taken", r2.status === 202, r2.status);
    await until(() => seen.filter((c) => c.$ === "HookHit").length >= 2, 10000);
    await sleep(500);
  }
  check("every message decodes", bad.length === 0, bad.map((b) => b.replace(/[^\x20-\x7e]/g, ".").slice(0, 300)));
} catch (e) {
  check("ran", false, String(e));
} finally {
  proc.kill();
  await proc.exited;
  mock.stop(true);
  if (!process.env.KEEP) rmSync(home, { recursive: true, force: true });
  else console.log("home " + home);
}
process.exit(failed ? 1 : 0);
