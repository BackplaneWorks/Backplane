// The hub holds no log lines (src/server/storedb.bend): what a joining
// client gets is read from store.db's line table and events.jsonl by
// offset. On a hub of this build (the JS service or the native binary):
//   - a join from 0 gets every line that parses, in order, a phone's push
//     registration without its token, the origin from the oldest line;
//   - lines a commit adds (a lock taken and given back over /lock/*) are
//     in store.db (line rows, meta.applied_off at the file's end) and in
//     the next join; a resume from `since` gets only what follows, a
//     resume from another origin everything;
//   - a restart serves the same; a deleted store.db is made again.
//   bun test/tools/store_lines_e2e.ts [BINARY]
import { Database } from "bun:sqlite";
import { appendFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { cbor, plain } from "../../src/web/wire.js";

const [bin = "build/backplane-serve"] = process.argv.slice(2);
const src = readFileSync(new URL("../../src/core/cbor.bend", import.meta.url).pathname, "utf8");
const list = (name: string) => JSON.parse(src.split(`def Cbor.${name}() -> List<&2, String>:`)[1].split("\n\n")[0].replace(/\s+/g, " ").trim());
const KEYS = list("keys"), WORDS = list("words");
let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const freePort = () => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };

const root = mkdtempSync(join(tmpdir(), "bp-store-lines-"));
const home = join(root, "home"), fake = join(root, "agents");
mkdirSync(home); mkdirSync(fake);
const log = join(home, "events.jsonl");
const lines = [
  '{"$":"ProjectCreated","id":"p1","title":"Pé","root":"/tmp/p1","at":1700000000}',
  '{"$":"ThreadCreated","id":"t1","project":"p1","title":"T 🙂","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1700000001}',
  '{"$":"MessagePosted","thread":"t1","msg":"m1","role":"user","text":"héllo wörld 🙂 ünïcode","at":1700000002}',
  "not json at all",
  '{"$":"FutureChange","x":1,"at":1700000003}',
  '{"$":"DeviceRegistered","token":"fake-token","platform":"ios","kind":"alert","thread":"","env":"dev","bundle":"x","at":1700000004}',
  '{"$":"MessagePosted","thread":"t1","msg":"m2","role":"assistant","text":"' + "x".repeat(5000) + '","at":1700000005}',
  '{"$":"SettingSet","key":"restart.continue","value":"off","at":1700000006}',
];
writeFileSync(log, lines.join("\n") + "\n");
const wire = (l: string) => { const o = JSON.parse(l); return o.$ === "DeviceRegistered" || o.$ === "DeviceDropped" ? { $: o.$, at: o.at } : o; };
const expected = () => readFileSync(log, "utf8").split("\n").filter((l) => { try { JSON.parse(l); return true; } catch { return false; } }).map(wire);

const port = freePort();
const env = { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_AGENTS: fake };
let proc: ReturnType<typeof Bun.spawn> | null = null;
async function start() {
  proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], { env, stdout: "inherit", stderr: "inherit" });
  for (let i = 0; i < 600; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) return; } catch {}
    await sleep(100);
  }
  throw new Error("hub did not answer /hello");
}
async function stop() { if (proc) { proc.kill("SIGTERM"); await proc.exited; proc = null; } }

// every log item a join gets, and the origin it was told
async function join_(query: string): Promise<{ items: any[]; origin: string; since: number }> {
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws${query}`);
  ws.binaryType = "arraybuffer";
  const items: any[] = []; let origin = "", since = -1, done = false;
  ws.onmessage = (e) => {
    const b = new Uint8Array(e.data as ArrayBuffer);
    if (b[1] === 1 && Buffer.from(b.subarray(3, 7)).toString() === "plot") return;
    const m = plain(cbor(b, KEYS, WORDS)) as any;
    if (m.t === "log") { origin = m.origin; since = m.since; items.push(...m.items); if (!m.more) done = true; }
    else if (m.t === "changes" && "boot" in m) { items.push(...m.items); if (m.boot === false) done = true; }
  };
  const end = Date.now() + 30000;
  while (!done && Date.now() < end) await sleep(50);
  ws.close();
  return { items, origin, since };
}
const same = (a: any[], b: any[]) => JSON.stringify(a) === JSON.stringify(b);

try {
  await start();
  let want = expected();
  let j = await join_("?since=0&origin=");
  check(same(j.items, want), `join from 0: ${j.items.length} items, ${want.length} expected`);
  check(j.origin === "1700000000", `origin from the oldest line (${j.origin})`);
  check(!JSON.stringify(j.items).includes("fake-token"), "a push registration goes out without its token");

  const body = JSON.stringify({ name: "store-e2e", holder: "pid:1@test:x", note: "e2e", wait: false });
  for (const op of ["acquire", "release"]) await fetch(`http://127.0.0.1:${port}/lock/${op}`, { method: "POST", headers: { "content-type": "application/json" }, body });
  await sleep(500);
  want = expected();
  check(want.length > j.items.length, `the locks committed lines (${want.length - j.items.length})`);
  const db = new Database(join(home, "store.db"), { readonly: true });
  const n = (db.query("SELECT count(*) AS n FROM line").get() as any).n;
  const off = (db.query("SELECT v FROM meta WHERE k = 'applied_off'").get() as any)?.v;
  const ents = (db.query("SELECT count(*) AS n FROM entry WHERE thread = 't1'").get() as any).n;
  db.close();
  check(n === want.length, `store.db has a line row per line (${n})`);
  check(Number(off) === statSync(log).size, `meta.applied_off is the log's end (${off})`);
  check(ents === 2, `entries in store.db (${ents})`);
  j = await join_("?since=0&origin=");
  check(same(j.items, want), `join after commits: ${j.items.length} items`);
  j = await join_(`?since=3&origin=1700000000`);
  check(same(j.items, want.slice(3)) && j.since === 3, `resume from 3: ${j.items.length} items`);
  j = await join_(`?since=3&origin=999`);
  check(same(j.items, want) && j.since === 0, `another origin: everything (${j.items.length})`);

  await stop(); await start();
  j = await join_("?since=0&origin=");
  check(same(j.items, want), `after a restart: ${j.items.length} items`);
  appendFileSync(log, "torn {\"$\":\"Proj");
  await stop();
  rmSync(join(home, "store.db")); rmSync(join(home, "store.db-wal"), { force: true }); rmSync(join(home, "store.db-shm"), { force: true });
  await start();
  want = expected();
  j = await join_("?since=0&origin=");
  check(same(j.items, want), `store.db made again (a torn tail too): ${j.items.length} items`);
} catch (e) {
  check(false, String(e));
} finally {
  await stop();
  rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all passed");
process.exit(fail ? 1 : 0);
