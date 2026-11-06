// Logging TCP proxy: bun android-proxy.ts LISTEN_IP:PORT TARGET_IP:PORT LOGFILE
// Logs each connection, HTTP request lines, and after a websocket upgrade every frame header in both directions.
import { appendFileSync } from "node:fs";
const [lh, th, log] = process.argv.slice(2); const [lip, lport] = lh.split(":"); const [tip, tport] = th.split(":");
const t0 = Date.now(); const L = (s: string) => appendFileSync(log, `${((Date.now() - t0) / 1000).toFixed(3)} ${s}\n`);
let cid = 0;
function frames(buf: Buffer, st: any, tag: string) { // parse ws frames incrementally (server->client unmasked, client->server masked)
  st.buf = Buffer.concat([st.buf, buf]);
  for (;;) { const b = st.buf; if (b.length < 2) return; const op = b[0] & 15, fin = b[0] >> 7, masked = b[1] >> 7; let len = b[1] & 127, off = 2;
    if (len === 126) { if (b.length < 4) return; len = b.readUInt16BE(2); off = 4; } else if (len === 127) { if (b.length < 10) return; len = Number(b.readBigUInt64BE(2)); off = 10; }
    if (masked) off += 4; if (b.length < off + len) { if (!st.waiting || st.waiting !== len) { st.waiting = len; } return; }
    st.n++; L(`${tag} frame#${st.n} op=${op} fin=${fin} len=${len}`); { const pl = Buffer.from(b.subarray(off, off + len)); if (masked) { const m = b.subarray(off - 4, off); for (let i = 0; i < pl.length; i++) pl[i] ^= m[i & 3]; } require("node:fs").writeFileSync(`${log}.${tag.replace(/[^a-z0-9]+/gi, "_")}_${st.n}.bin`, pl); } st.buf = b.subarray(off + len); st.waiting = 0; }
}
Bun.listen({ hostname: lip, port: Number(lport), socket: {
  open(c) { const id = ++cid; (c as any).id = id; (c as any).st = { up: false, s2c: { buf: Buffer.alloc(0), n: 0 }, c2s: { buf: Buffer.alloc(0), n: 0 }, bytes: [0, 0] }; L(`conn#${id} open`);
    Bun.connect({ hostname: tip, port: Number(tport), socket: {
      open(u) { (c as any).up = u; (u as any).c = c; (c as any).q?.forEach((d: any) => u.write(d)); (c as any).q = []; },
      data(u, d) { const s = (c as any).st; s.bytes[1] += d.length; if (!s.ws) { const h = Buffer.from(d).toString("latin1"); if (h.startsWith("HTTP/1.1 101")) { s.ws = true; L(`conn#${id} s2c 101 upgrade`); const i = h.indexOf("\r\n\r\n"); if (i >= 0) frames(Buffer.from(d).subarray(i + 4), s.s2c, `conn#${id} s2c`); } else L(`conn#${id} s2c ${h.split("\r\n")[0]} (${d.length}b)`); } else frames(Buffer.from(d), s.s2c, `conn#${id} s2c`); c.write(d); },
      close() { L(`conn#${id} upstream closed, bytes c2s=${(c as any).st.bytes[0]} s2c=${(c as any).st.bytes[1]}`); c.end(); }, error(_, e) { L(`conn#${id} upstream error ${e}`); } } }).catch((e) => L(`conn#${id} connect fail ${e}`)); },
  data(c, d) { const s = (c as any).st; s.bytes[0] += d.length; if (!s.ws) { const l = Buffer.from(d).toString("latin1").split("\r\n")[0].replace(/token=[0-9a-f]+/g, "token=<r>").replace(/#.*$/, ""); L(`conn#${(c as any).id} c2s ${l}`); } else frames(Buffer.from(d), s.c2s, `conn#${(c as any).id} c2s`); const u = (c as any).up; if (u) u.write(d); else ((c as any).q ??= []).push(d); },
  close(c) { L(`conn#${(c as any).id} client closed, bytes c2s=${(c as any).st.bytes[0]} s2c=${(c as any).st.bytes[1]}`); (c as any).up?.end(); }, error(c, e) { L(`conn#${(c as any).id} client error ${e}`); } } });
L(`proxy ${lh} -> ${th}`); setInterval(() => {}, 1e6);
