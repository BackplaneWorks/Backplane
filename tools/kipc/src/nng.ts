// nng over bun:ffi: REQ0 (the helper) and REP0 (the test stub). The library
// is the runtime's own libnng, else the system's. Calls block the thread
// they run on, so each socket lives in its own Worker (see worker.ts).
import { dlopen, FFIType, ptr, read, toArrayBuffer } from "bun:ffi";
import { existsSync, readdirSync } from "node:fs";

const NNG_FLAG_ALLOC = 1;
const NNG_ETIMEDOUT = 5;

const symbols = {
  nng_req0_open: { args: [FFIType.ptr], returns: FFIType.i32 },
  nng_rep0_open: { args: [FFIType.ptr], returns: FFIType.i32 },
  nng_close: { args: [FFIType.u32], returns: FFIType.i32 },
  nng_dial: { args: [FFIType.u32, FFIType.ptr, FFIType.ptr, FFIType.i32], returns: FFIType.i32 },
  nng_listen: { args: [FFIType.u32, FFIType.ptr, FFIType.ptr, FFIType.i32], returns: FFIType.i32 },
  nng_send: { args: [FFIType.u32, FFIType.ptr, FFIType.u64, FFIType.i32], returns: FFIType.i32 },
  nng_recv: { args: [FFIType.u32, FFIType.ptr, FFIType.ptr, FFIType.i32], returns: FFIType.i32 },
  nng_free: { args: [FFIType.ptr, FFIType.u64], returns: FFIType.void },
  nng_socket_set_ms: { args: [FFIType.u32, FFIType.ptr, FFIType.i32], returns: FFIType.i32 },
  nng_strerror: { args: [FFIType.i32], returns: FFIType.ptr },
} as const;

const so = process.platform === "darwin" ? "dylib" : "so";

// libnng files under a runtime's lib/, newest-looking name first
function inRuntime(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir)
    .filter((n) => n.startsWith("libnng.") && (n.includes(`.${so}`) || n.endsWith(`.${so}`)))
    .sort()
    .reverse()
    .map((n) => `${dir}/${n}`);
}

export function candidates(lib?: string): string[] {
  const out: string[] = [];
  if (lib) out.push(...inRuntime(lib));
  out.push(process.platform === "darwin" ? "libnng.1.dylib" : "libnng.so.1");
  out.push(process.platform === "darwin" ? "libnng.dylib" : "libnng.so");
  return out;
}

type Lib = ReturnType<typeof dlopen<typeof symbols>>["symbols"];
const loaded = new Map<string, Lib>();

export function load(lib?: string): Lib {
  const key = lib ?? "";
  const hit = loaded.get(key);
  if (hit) return hit;
  let last: unknown = null;
  for (const c of candidates(lib)) {
    try {
      const l = dlopen(c, symbols).symbols;
      loaded.set(key, l);
      return l;
    } catch (e) {
      last = e;
    }
  }
  throw new Error(`libnng not found${lib ? ` (looked in ${lib} and the system)` : ""}: ${last}`);
}

const cstr = (s: string) => Buffer.from(s + "\0");

export class NngError extends Error {
  constructor(public code: number, what: string, public timeout = false) {
    super(what);
  }
}

class Sock {
  id = 0;
  constructor(protected l: Lib, open: "nng_req0_open" | "nng_rep0_open") {
    const b = new Uint32Array(1);
    this.check(l[open](ptr(b)), open);
    this.id = b[0];
  }
  protected check(rv: number, what: string) {
    if (rv === 0) return;
    const s = this.l.nng_strerror(rv);
    // CString is not needed: read bytes up to NUL
    let msg = String(rv);
    if (s) {
      const bytes: number[] = [];
      for (let i = 0; i < 200; i++) {
        const c = read.u8(s as unknown as number, i);
        if (c === 0) break;
        bytes.push(c);
      }
      msg = Buffer.from(bytes).toString();
    }
    throw new NngError(rv, `${what}: ${msg}`, rv === NNG_ETIMEDOUT);
  }
  timeouts(ms: number) {
    this.check(this.l.nng_socket_set_ms(this.id, cstr("recv-timeout"), ms), "set recv timeout");
    this.check(this.l.nng_socket_set_ms(this.id, cstr("send-timeout"), ms), "set send timeout");
  }
  send(bytes: Uint8Array) {
    const b = Buffer.from(bytes);
    this.check(this.l.nng_send(this.id, ptr(b), b.length, 0), "send");
  }
  recv(): Uint8Array {
    const pp = new BigUint64Array(1);
    const sz = new BigUint64Array(1);
    this.check(this.l.nng_recv(this.id, ptr(pp), ptr(sz), NNG_FLAG_ALLOC), "recv");
    const p = Number(pp[0]);
    const n = Number(sz[0]);
    const out = new Uint8Array(toArrayBuffer(p as never, 0, n).slice(0));
    this.l.nng_free(p as never, n);
    return out;
  }
  close() {
    if (this.id) this.l.nng_close(this.id);
    this.id = 0;
  }
}

export class Req extends Sock {
  constructor(l: Lib) {
    super(l, "nng_req0_open");
    // a request is never resent behind the caller's back
    this.check(l.nng_socket_set_ms(this.id, cstr("req:resend-time"), 3600000), "set resend");
  }
  dial(url: string) {
    this.check(this.l.nng_dial(this.id, cstr(url), 0 as never, 0), `dial ${url}`);
  }
  // one request, one reply
  request(bytes: Uint8Array, ms: number): Uint8Array {
    this.timeouts(ms);
    this.send(bytes);
    return this.recv();
  }
}

export class Rep extends Sock {
  constructor(l: Lib) {
    super(l, "nng_rep0_open");
  }
  listen(url: string) {
    this.check(this.l.nng_listen(this.id, cstr(url), 0 as never, 0), `listen ${url}`);
  }
  // null when the wait ended without a request
  next(ms: number): Uint8Array | null {
    this.timeouts(ms);
    try {
      return this.recv();
    } catch (e) {
      if (e instanceof NngError && e.timeout) return null;
      throw e;
    }
  }
}
