// A lone surrogate typed into a draft or a message (a pasted half emoji)
// goes from the web page to a hub and its log without throwing and without an
// undecodable line. The page builds its requests with wire.js's json() and
// encodes them with app.bend's Cbor.encode (wire_out_json); json() makes each
// lone surrogate U+FFFD, so the frame is valid UTF-8 and the hub, its log and
// a restarted hub all read the same text.
//
//   bun test/tools/lone_surrogate_e2e.ts HUB MODULE
// HUB: a hub binary (scripts/build-js.sh src/server/main.bend ...)
// MODULE: bend test/json_native_test.bend -o MODULE (absolute path)
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { json, learnJson, plain } from "../../src/web/wire.js";
import { ThinViews } from "../../src/web/thin.js";

const [bin, mod] = process.argv.slice(2);
if (!bin || !mod) throw new Error("usage: lone_surrogate_e2e.ts HUB MODULE");
const F = (await import(pathToFileURL(resolve(mod)).href)).default;
learnJson(F.json_tags());
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const toList = (u8: Uint8Array) => { let xs: any = { $: "Nil" }; for (let i = u8.length - 1; i >= 0; i -= 1) xs = { $: "Con", head: u8[i], tail: xs }; return xs; };
const fromList = (xs: any) => { const out: number[] = []; for (let x = xs; x && x.$ === "Con"; x = x.tail) out.push(x.head); return new Uint8Array(out); };
// what the page sends (host.js: App.wire_out_json(Wire.json(request)))
const encode = (v: unknown) => fromList(F.wire_out_json(json(v)));
const decode = (u8: Uint8Array) => plain(F.wire_in(toList(u8)));

let fail = 0;
const check = (ok: boolean, what: string, more: unknown = "") => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}${ok ? "" : ": " + JSON.stringify(more)}`);
  if (!ok) fail += 1;
};
const until = async <T>(ms: number, f: () => T | undefined | null | false): Promise<T | undefined> => {
  const end = Date.now() + ms;
  for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(50); }
};

const DRAFT = "draft \ud83d half \udc00 end";        // a high and a low alone
const MESSAGE = "msg \ud800\ud800 pair 😀 tail \udbff"; // two highs, a real pair, a trailing high
const FIX = (s: string) => s.replace(/[\ud800-\udfff]/gu, "�");

// the frame carries no UTF-8 of a surrogate (ED A0..BF ..: what Bend's
// encoder writes for one), only U+FFFD (EF BF BD)
{
  const bytes = encode({ m: "turn.start", p: { text: MESSAGE } });
  const surrogate = bytes.some((b, i) => b === 0xed && (bytes[i + 1] & 0xe0) === 0xa0);
  const fffd = bytes.some((b, i) => b === 0xef && bytes[i + 1] === 0xbf && bytes[i + 2] === 0xbd);
  check(!surrogate && fffd, "the frame has U+FFFD and no encoded surrogate");
  check((decode(bytes) as any)?.p?.text === FIX(MESSAGE), "the hub's decoder reads U+FFFD where the page had a lone surrogate", decode(bytes));
}

const root = mkdtempSync(join(tmpdir(), "bp-lone-"));
const home = join(root, "home"), fake = join(root, "agents"), proj = join(root, "proj");
for (const d of [home, fake, proj]) mkdirSync(d, { recursive: true });
for (const n of ["claude", "codex", "grok", "muse", "opencode"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
const freePort = () => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };
const port = freePort();
let proc: ReturnType<typeof Bun.spawn> | null = null;
const start = async () => {
  proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0",
      BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fake, BACKPLANE_KISTACK_SYNC: "0", PATH: `${fake}:/usr/bin:/bin` },
    stdout: Bun.file(join(root, "hub.stdout")), stderr: Bun.file(join(root, "hub.stderr")),
  });
  for (let i = 0; i < 300; i += 1) { try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) return; } catch {} await sleep(100); }
  throw new Error("the hub did not start");
};
const stop = async () => { if (proc) { proc.kill(); await proc.exited; proc = null; } };

type Conn = { ws: WebSocket; seen: any[]; frames: any[]; n: number };
const connect = async (q = ""): Promise<Conn> => {
  const c: Conn = { ws: new WebSocket(`ws://127.0.0.1:${port}/ws${q}`), seen: [], frames: [], n: 0 };
  c.ws.binaryType = "arraybuffer";
  c.ws.onmessage = (e) => {
    if (typeof e.data === "string") return;
    const o: any = decode(new Uint8Array(e.data as ArrayBuffer));
    c.frames.push(o);
    for (const x of o?.items ?? []) c.seen.push(x);
  };
  await new Promise((r, j) => { c.ws.onopen = r; c.ws.onerror = j; });
  return c;
};
const rpc = async (c: Conn, m: string, p: object) => {
  const id = ++c.n;
  c.ws.send(encode({ id, m, p }));
  return await until(30000, () => c.frames.find((f) => f?.t === "reply" && Number(f.id) === id));
};
const strings = (v: any, out: string[] = []): string[] => { if (typeof v === "string") out.push(v); else if (v && typeof v === "object") for (const x of Object.values(v)) strings(x, out); return out; };

try {
  await start();
  const c = await connect();
  await rpc(c, "project.add", { path: proj });
  const pc = await until(10000, () => c.seen.find((x) => x.$ === "ProjectCreated"));
  await rpc(c, "thread.create", { project: pc?.id, title: "lone " + "\udc01" });
  const th = await until(10000, () => c.seen.find((x) => x.$ === "ThreadCreated"));
  check(!!th, "thread.create with a lone surrogate in its title", c.frames.slice(-3));
  const sent = await rpc(c, "turn.start", { thread: th?.id, text: MESSAGE, msg: "m-lone", mode: "queue" });
  check(!!sent, "turn.start with lone surrogates is answered", sent);
  const posted = await until(15000, () => c.seen.find((x) => x.$ === "MessagePosted" && x.thread === th?.id && x.role === "user"));
  check(posted?.text === FIX(MESSAGE), "the message is posted with U+FFFD in place", posted?.text);

  // a thin view (the web page's) whose session carries the draft
  const v = await connect("?view=1&frontend=1");
  const greet: any = await until(10000, () => v.frames.find((f) => typeof f?.generation === "string" && typeof f?.epoch === "string") ?? v.frames.find((f) => f?.greeting)?.greeting);
  const views = new ThinViews({ now: () => performance.now(), send: (_h: any, r: any) => r, command: (x: any) => x });
  check(!!greet && views.connect("e2e", greet).connected, "thin view greeting");
  const session: any = { sel: th?.id, rid: 1, drafts: { [th?.id]: DRAFT }, open: {}, info: {}, adding: false, panel: false };
  const call: any = views.select("e2e", { kind: "thread", id: th?.id }, session, "lone-surrogate");
  check(!call.failed && call.cmds.length === 1, "the production thin client opens the view", call);
  v.ws.send(encode(call.cmds[0]));
  const reply: any = await until(20000, () => v.frames.find((f) => f?.t === "reply" && Number(f.id) === call.id));
  check(!!reply?.ok, "the hub answers view.open with the draft in the session", reply && { ok: reply.ok, error: reply.error });
  const texts = strings(reply ?? {});
  check(texts.some((t) => t.includes(FIX(MESSAGE))), "the view shows the message as the hub stored it");
  check(texts.some((t) => t.includes(FIX(DRAFT))), "the view shows the draft with U+FFFD in place");
  check(texts.every((t) => t.isWellFormed()), "nothing in the view reply has a lone surrogate");
  check(!!(await fetch(`http://127.0.0.1:${port}/hello`).then((r) => r.ok).catch(() => false)), "the hub still answers");
  v.ws.close(); c.ws.close();

  // the log: every line is JSON with no lone surrogate, and holds the text
  await sleep(500);
  const lines = readFileSync(join(home, "events.jsonl"), "utf8").split("\n").filter(Boolean);
  const bad = lines.filter((l) => { try { JSON.parse(l); return !l.isWellFormed(); } catch { return true; } });
  check(lines.length > 0 && bad.length === 0, `every log line decodes (${lines.length} lines)`, bad.slice(0, 2));
  check(lines.some((l) => { try { return JSON.parse(l).text === FIX(MESSAGE); } catch { return false; } }), "the log keeps the message with U+FFFD");
  check(lines.every((l) => F.show(F.parse(l)) === F.show(json(JSON.parse(l)))), "Bend's parser (the hub reading its log) reads every line without throwing");

  // a restarted hub reads its log back and still has the message
  await stop();
  await start();
  const c2 = await connect();
  const again = await until(15000, () => c2.seen.find((x) => x.$ === "MessagePosted" && x.text === FIX(MESSAGE)));
  check(!!again, "a restarted hub reads the log back with the message");
  c2.ws.close();
} finally {
  await stop();
  if (fail === 0) rmSync(root, { recursive: true, force: true });
  else console.log(`kept ${root}`);
}
console.log(fail ? `${fail} FAILED` : "lone_surrogate_e2e: all ok");
process.exit(fail ? 1 : 0);
