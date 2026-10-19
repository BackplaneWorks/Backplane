// backplane-kipc: KiCad's IPC API for the hub, as NDJSON on stdin/stdout.
//   {"id","sock","type","message":{...},"timeout"?,"lib"?,"desc"?}
//   -> {"id","ok":true,"type","message":{...}}
//    | {"id","ok":false,"status":"AS_...","error":"..."}
//   {"id","op":"close","sock"} drops a socket's connection.
//   {"id","op":"spawn","key","argv":[...],"memory"?} starts a KiCad server
//   (answers {"id","ok":true}); what it prints and its end come as events
//   {"event":"line","key","text"} and {"event":"exit","key","code","tail"}.
//   The helper never signals a process: servers end by themselves
//   (--parent-pid, --idle-exit, kiapi.backplane.Shutdown).
// --lib <runtime lib dir>, --desc <descriptor set> are the defaults; a
// request may name its own runtime's.
import { decodeResponse, encodeRequest } from "./codec.ts";
import { load, NO_SCHEMA } from "./schema.ts";
import { createInterface } from "node:readline";
import { spawn } from "node:child_process";
import { existsSync, readdirSync, statSync } from "node:fs";
import { basename, dirname, join } from "node:path";

const args = process.argv.slice(2);
const opt = (n: string) => {
  const i = args.indexOf(n);
  return i >= 0 ? (args[i + 1] ?? "") : "";
};
const defLib = opt("--lib");
const defDesc = opt("--desc");
const client = `backplane-${process.pid}`;

type Slot = { w: Worker; token: string; pending: Map<string, (m: { bytes?: Uint8Array; error?: string }) => void> };
const socks = new Map<string, Slot>();

function slot(sock: string): Slot {
  let s = socks.get(sock);
  if (s) return s;
  const w = new Worker(Bun.main.startsWith("/$bunfs") ? "./worker.ts" : new URL("./worker.ts", import.meta.url));
  const pending = new Map<string, (m: { bytes?: Uint8Array; error?: string }) => void>();
  w.onmessage = (ev: MessageEvent) => {
    const d = ev.data as { op: string; id?: string; bytes?: Uint8Array; error?: string };
    if (d.op === "reply" && d.id !== undefined) {
      const f = pending.get(d.id);
      pending.delete(d.id);
      f?.(d);
    }
  };
  s = { w, token: "", pending };
  socks.set(sock, s);
  return s;
}

const out = (o: unknown) => process.stdout.write(JSON.stringify(o) + "\n");
const fail = (id: unknown, error: string, status?: string) => out(status ? { id, ok: false, status, error } : { id, ok: false, error });

const which = (cmd: string) => (process.env.PATH ?? "").split(":").map((d) => `${d}/${cmd}`).find((f) => existsSync(f));

// inside a systemd scope capped at `memory` when this machine has a user manager
function wrap(argv: string[], memory?: string): string[] {
  const bus = process.env.XDG_RUNTIME_DIR ? `${process.env.XDG_RUNTIME_DIR}/bus` : "";
  const sd = which("systemd-run");
  if (!memory || process.platform !== "linux" || !sd || !bus || !existsSync(bus)) return argv;
  return [sd, "--user", "--scope", "--quiet", "-p", `MemoryMax=${memory}`, "--", ...argv];
}

// KiCad may choose api-<pid>.sock when the requested path is occupied.
// Only sockets created by this launch can supply a missing stdout ready line.
function socketsAt(requested: string): Map<string, string> {
  const sockets = new Map<string, string>();
  if (!requested) return sockets;
  try {
    for (const name of readdirSync(dirname(requested))) {
      if (name !== basename(requested) && !/^api-[0-9]+\.sock$/.test(name)) continue;
      const path = join(dirname(requested), name);
      try {
        const stat = statSync(path);
        if (stat.isSocket()) sockets.set(path, `${stat.dev}:${stat.ino}:${stat.birthtimeMs}`);
      } catch { /* the server can replace a stale socket during this scan */ }
    }
  } catch { /* no socket directory yet */ }
  return sockets;
}

function startServer(key: string, argv0: string[], memory?: string) {
  const argv = wrap(argv0, memory);
  const tail: string[] = [];
  const keep = (l: string) => {
    tail.push(l);
    if (tail.length > 12) tail.shift();
  };
  let done = false;
  const end = (code: number, extra?: string) => {
    if (done) return;
    done = true;
    const t = (extra ? [extra, ...tail] : tail).join("\n");
    out({ event: "exit", key, code, tail: t.slice(-400) });
  };
  const socketArg = argv0.indexOf("--socket");
  const at = socketArg >= 0 ? argv0[socketArg + 1] ?? "" : "";
  const before = socketsAt(at);
  const child = spawn(argv[0], argv.slice(1), { stdio: ["ignore", "pipe", "pipe"] });
  child.on("error", (e) => end(127, String(e.message ?? e)));
  // KiCad buffers stdout on a pipe. Report its freshly bound socket,
  // then the hub still requires Ping and a matching GetVersion.
  let said = false;
  if (at) {
    const poll = setInterval(() => {
      if (done || said || child.exitCode !== null) return clearInterval(poll);
      for (const [path, identity] of socketsAt(at)) {
        if (before.get(path) === identity) continue;
        said = true;
        clearInterval(poll);
        out({ event: "line", key, text: `KiCad API server listening at ${path}` });
        break;
      }
    }, 100);
  }
  createInterface({ input: child.stdout! }).on("line", (l) => {
    keep(l);
    const ready = l.startsWith("KiCad API server listening at ");
    if (ready && said) return;
    if (ready) said = true;
    out({ event: "line", key, text: l });
  });
  createInterface({ input: child.stderr! }).on("line", keep);
  child.on("close", (code, signal) => end(code ?? (signal ? 128 : 1)));
}

let seq = 0;
async function handle(line: string) {
  let r: { id?: unknown; op?: string; sock?: string; type?: string; message?: unknown; timeout?: number; lib?: string; desc?: string };
  try {
    r = JSON.parse(line);
  } catch {
    return fail(null, "bad request line");
  }
  const id = r.id ?? null;
  if (r.op !== "spawn" && !r.sock) return fail(id, "no sock");
  if (r.op === "close") {
    const s = socks.get(r.sock!);
    if (s) {
      s.w.postMessage({ op: "close" });
      s.w.terminate();
      socks.delete(r.sock!);
    }
    return out({ id, ok: true });
  }
  if (r.op === "spawn") {
    const o = r as unknown as { key?: string; argv?: string[]; memory?: string };
    if (!o.key || !Array.isArray(o.argv) || !o.argv.length) return fail(id, "spawn needs key and argv");
    startServer(o.key, o.argv, o.memory);
    return out({ id, ok: true });
  }
  if (!r.type) return fail(id, "no type");
  const schema = load(r.desc ?? defDesc);
  if (!schema.knows(r.type)) return fail(id, NO_SCHEMA);
  const s = slot(r.sock!);
  let bytes: Uint8Array;
  try {
    bytes = encodeRequest(schema, r.type, r.message, s.token, client);
  } catch (e) {
    return fail(id, `bad message for ${r.type}: ${(e as Error).message}`, "AS_BAD_REQUEST");
  }
  const key = String(++seq);
  const got = await new Promise<{ bytes?: Uint8Array; error?: string }>((res) => {
    s.pending.set(key, res);
    s.w.postMessage({ op: "req", id: key, sock: r.sock!, lib: r.lib ?? defLib, bytes, timeout: r.timeout ?? 30000 });
  });
  if (got.error !== undefined || !got.bytes) return fail(id, got.error ?? "no reply");
  try {
    const rep = decodeResponse(schema, got.bytes);
    if (rep.token) s.token = rep.token;
    if (rep.ok) out({ id, ok: true, type: rep.type, message: rep.message });
    else fail(id, rep.error, rep.status);
  } catch (e) {
    fail(id, `bad reply: ${(e as Error).message}`);
  }
}

const rl = createInterface({ input: process.stdin });
rl.on("line", (l) => {
  if (l.trim()) void handle(l);
});
// stdin closed: the hub is gone
rl.on("close", () => process.exit(0));
