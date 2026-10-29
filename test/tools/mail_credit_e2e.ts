// Isolate RPC byte credit from mail filing, search, agents and windows.
// bun test/tools/mail_credit_e2e.ts BINARY WIREDIR [--expect-stall] [--out FILE]
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
const [binary, wiredir] = process.argv.slice(2);
for (const f of readdirSync(wiredir).filter((x) => x.endsWith(".js"))) (0, eval)(readFileSync(join(wiredir, f), "utf8"));
const W = (globalThis as any).Wire;
const root = mkdtempSync(join(tmpdir(), "bp-mail-credit-")), home = join(root, "home"); mkdirSync(home);
writeFileSync(join(home, "events.jsonl"), ["mail.on", "restart.continue"].map((key, i) => JSON.stringify({ $: "SettingSet", key, value: i ? "off" : "on", at: 1 })).join("\n") + "\n");
const listener = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
const port = listener.port; listener.stop(true); if (port === 3787) throw new Error("live port");
const hub = Bun.spawn([resolve(binary), "--home", home, "--port", String(port), "--host", "127.0.0.1", "--foreground", "--no-tailscale"], {
  env: { ...process.env, HOME: root, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_PEERS: "", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_LOCK_DIR: join(root, "locks"), BACKPLANE_DEPLOY_LOCKS: join(root, "deploy-locks") }, stdout: Bun.file(join(root, "stdout")), stderr: Bun.file(join(root, "stderr")),
});
const sleep = (n: number) => new Promise((r) => setTimeout(r, n));
const sourceAt = process.argv.indexOf("--server-source-sha");
const hash = (path: string) => new Bun.CryptoHasher("sha256").update(readFileSync(path)).digest("hex");
const result: any = { root, binary: resolve(binary), binary_sha256: hash(resolve(binary)), benchmark_sha256: hash(import.meta.path), server_source_sha256: sourceAt >= 0 ? process.argv[sourceAt + 1] : null, checks: {}, received: 0, offered_text_units: 0, compressed_payload_bytes: 0, websocket_header_bytes: 0, ledger: {} };
function ledger() {
  return { replies: result.received, offered_json_codepoints: result.offered_text_units, cbor_payload_bytes: result.compressed_payload_bytes, websocket_header_bytes: result.websocket_header_bytes, original_refund_frame_bytes: result.compressed_payload_bytes + result.websocket_header_bytes, json_minus_payload: result.offered_text_units - result.compressed_payload_bytes, original_unmatched_credit: result.offered_text_units - result.compressed_payload_bytes - result.websocket_header_bytes };
}
const sockets: WebSocket[] = []; let id = 0;
async function client() {
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`); ws.binaryType = "arraybuffer"; sockets.push(ws);
  const pending = new Map<number, { resolve: (x: any) => void; timer: Timer }>();
  const seen: any[] = [];
  ws.onmessage = (e) => {
    const bytes = new Uint8Array(e.data as ArrayBuffer), json = W.decode(bytes), j = JSON.parse(json);
    if (j.t === "reply") {
      result.received++; result.offered_text_units += Array.from(json).length; result.compressed_payload_bytes += bytes.length;
      // Server frames are unmasked. Conn.put used to refund the entire
      // WebSocket frame, including its 2/4/10-byte header, not just CBOR.
      result.websocket_header_bytes += bytes.length < 126 ? 2 : bytes.length < 65536 ? 4 : 10;
      const p = pending.get(Number(j.id)); if (p) { clearTimeout(p.timer); pending.delete(Number(j.id)); p.resolve(j); }
    }
    for (const x of j.items ?? []) seen.push(typeof x === "string" ? JSON.parse(x) : x);
  };
  await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
  const rpc = (m: string, p: object) => new Promise<any>((resolve, reject) => {
    const n = ++id, timer = setTimeout(() => { pending.delete(n); reject(new Error(`${m} reply timeout`)); }, 15000);
    pending.set(n, { resolve, timer }); ws.send(W.encode(JSON.stringify({ id: n, m, p })));
  });
  return { ws, rpc, seen };
}
async function until(f: () => boolean, ms: number) { const end = performance.now() + ms; while (!f()) { if (performance.now() > end) return false; await sleep(10); } return true; }
let failed = false;
try {
  for (let i = 0; ; i++) { try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) break; } catch {} if (i > 400) throw new Error("startup timeout"); await sleep(25); }
  const a = await client();
  const start = performance.now();
  for (let i = 0; i < 50000; i += 64) await Promise.all(Array.from({ length: Math.min(64, 50000 - i) }, () => a.rpc("mail.list", { n: 0 })));
  result.rpc_wall_ms = performance.now() - start;
  result.ledger.after_50000_load_replies = ledger();
  await a.rpc("setting.set", { key: "credit.final", value: "hé🙂 final" });
  const recovered = await until(() => a.seen.some((x) => x.$ === "SettingSet" && x.key === "credit.final"), 1500);
  result.checks.observer_recovers = recovered;
  // Another recipient exercises pre-encoded broadcasts as well as text RPCs.
  const b = await client();
  result.checks.fresh_connection_has_durable_marker = await until(() => b.seen.some((x) => x.$ === "SettingSet" && x.key === "credit.final"), 3000);
  await b.rpc("setting.set", { key: "credit.broadcast", value: "日本語🙂" });
  result.checks.broadcast_reaches_both = await until(() => [a, b].every((c) => c.seen.some((x) => x.$ === "SettingSet" && x.key === "credit.broadcast")), 1500);
  // Retire exactly this connection with queued work, then prove others live.
  const c = await client();
  for (let i = 0; i < 100; i++) c.ws.send(W.encode(JSON.stringify({ id: ++id, m: "mail.list", p: { n: 0 } })));
  c.ws.close();
  await b.rpc("setting.set", { key: "credit.after-close", value: "ok" });
  result.checks.close_isolated = await until(() => b.seen.some((x) => x.$ === "SettingSet" && x.key === "credit.after-close"), 1500);
  const expectStall = process.argv.includes("--expect-stall");
  failed = !(result.ledger.after_50000_load_replies.original_unmatched_credit > 1048576 && result.checks.fresh_connection_has_durable_marker && result.checks.close_isolated && (expectStall ? !recovered : recovered && result.checks.broadcast_reaches_both));
} catch (e) { result.error = String(e); failed = true; }
finally {
  result.ledger.after_control_replies = ledger();
  result.ledger.scope = "RPC replies only; log/join/boot/delta/broadcast frames excluded. Zero/variable-charge offers are tested in mailcredit_test.bend. Control replies use the same charge/refund rule as load replies.";
  for (const ws of sockets) ws.close();
  if (hub.exitCode === null) hub.kill("SIGTERM"); await hub.exited;
  const at = process.argv.indexOf("--out");
  if (at >= 0) { const path = resolve(process.argv[at + 1]); mkdirSync(dirname(path), { recursive: true }); writeFileSync(path, JSON.stringify(result, null, 2) + "\n"); }
  console.log(JSON.stringify(result));
}
process.exit(failed ? 1 : 0);
