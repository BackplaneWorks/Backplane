// The JS runtime on Bun's own event loop (loop.c's twin answers "c").
//
// Bend's JS runtime runs every computation in one synchronous loop that
// waits on descriptors with select() through bun:ffi into libc, so Bun's
// event loop never turns (no socket callback, timer or Promise fires) and
// a host without libc.so.6/libSystem (Windows) cannot wait at all. This
// file replaces io_run and io_exit at load (they are the runtime's own
// top-level functions; effect files run before main is called):
//
// - io_run is async: it runs ready computations and, after ~4 ms of work or
//   when none is ready, awaits Bun's loop (setImmediate, or a wake from a
//   settled park plus a timer for the nearest deadline). It never blocks.
// - io_park_promise(p, k, then): an effect parks k on a Promise and
//   returns undefined; when p settles, then(value) or then(undefined, err)
//   is k's value (undefined: then parked k again).
// - io_park_on(fd, out, k, more, at) keeps its meaning. A handle id
//   (>= BP_BASE, see BP_H below) parks on the handle's readiness; a real
//   descriptor (POSIX only) parks as before and is polled with poll()
//   between turns, or a blocking poll() when nothing waits on Bun (a
//   hang-up wakes a parked writer, which select() missed). Deadlines are
//   timers.
// - BP_H: Bun-backed handles (TCP from Bun.listen/Bun.connect here; proc,
//   pty and relay kinds from other effect files) behind one interface, so
//   the Base TCP/Socket/Listener effects and host.bend's Sock/Fd/Net
//   effects take a handle where they took a descriptor. A descriptor still
//   reaches the effect it always did.
// - Base File.read*, Process.run and errno text without libc.
//
// On Linux and macOS the old loop stays the default until L1's load gate
// passes on this one; BACKPLANE_JS_LOOP=bun turns it on there. Windows has
// no old loop and always runs this one. globalThis.BEND_ASYNC says which.

const BP_WIN = process.platform === "win32";
const BP_ASYNC = BP_WIN || process.env.BACKPLANE_JS_LOOP === "bun";
const BP_BASE = 0x100000;
const bp_sync_park = io_park_on;
const bp_sync_strerror = io_strerror;
globalThis.BEND_ASYNC = BP_ASYNC;

// Errno
// =====

// errno numbers as the C twin would see them: the host's own on POSIX,
// Linux's on Windows (Bend code shows them, never branches on a Windows one)
const BP_LINUX_ERRNO = { EPERM: 1, ENOENT: 2, EINTR: 4, EIO: 5, EBADF: 9,
  EAGAIN: 11, ENOMEM: 12, EACCES: 13, EEXIST: 17, ENOTDIR: 20, EISDIR: 21,
  EINVAL: 22, EMFILE: 24, ENOSPC: 28, EPIPE: 32, ENOSYS: 38, ENOTEMPTY: 39,
  ENOTSOCK: 88, EADDRINUSE: 98, EADDRNOTAVAIL: 99, ENETUNREACH: 101,
  ECONNABORTED: 103, ECONNRESET: 104, ENOTCONN: 107, ETIMEDOUT: 110,
  ECONNREFUSED: 111, EHOSTUNREACH: 113, EFBIG: 27, EOVERFLOW: 75 };
const BP_ERRNO = BP_WIN ? BP_LINUX_ERRNO
  : { ...BP_LINUX_ERRNO, ...require("os").constants.errno };
const BP_TEXT = { 1: "Operation not permitted", 2: "No such file or directory",
  4: "Interrupted system call", 5: "Input/output error", 9: "Bad file descriptor",
  11: "Resource temporarily unavailable", 13: "Permission denied",
  17: "File exists", 22: "Invalid argument", 32: "Broken pipe",
  38: "Function not implemented", 98: "Address already in use",
  99: "Cannot assign requested address", 104: "Connection reset by peer",
  107: "Transport endpoint is not connected", 110: "Connection timed out",
  111: "Connection refused" };

function bp_errno(e) {
  if (e !== null && typeof e === "object") {
    if (typeof e.code === "string" && BP_ERRNO[e.code] !== undefined) {
      return BP_ERRNO[e.code];
    }
    if (typeof e.errno === "number" && e.errno !== 0 && !BP_WIN) {
      return Math.abs(e.errno);
    }
  }
  return BP_ERRNO.EIO;
}

if (BP_WIN) {
  io_strerror = (code) => BP_TEXT[code] ?? "errno " + code;
}

// Handles
// =======

// A handle is an object kept under one or more ids (Sock.dup adds one); it
// is closed when its last id is. Readiness: read(u8) answers n > 0 bytes,
// 0 at the end, or -errno (-EAGAIN: nothing yet); write(u8) answers the
// bytes taken or -errno (-EAGAIN: full); ready(out) whether that would not
// be -EAGAIN; a parked computation is woken by wake(out). recv(max) and
// send(u8) are the Promise forms.
class BpHandle {
  constructor(kind) {
    this.kind = kind;
    this.refs = 0;
    this.parked = [[], []];
  }
  ready(out) {
    return true;
  }
  wake(out) {
    const ws = this.parked[out ? 1 : 0];
    this.parked[out ? 1 : 0] = [];
    for (const w of ws) {
      w.fire();
    }
  }
  wakeAll() {
    this.wake(false);
    this.wake(true);
  }
  peer() {
    return "";
  }
  shutdown(how) {
  }
  close() {
  }
  recv(max) {
    const b = new Uint8Array(Math.max(1, Number(max)));
    return new Promise((res, rej) => {
      const go = () => {
        const n = this.read(b);
        if (n === -BP_ERRNO.EAGAIN) {
          this.parked[0].push({ fire: go });
        } else if (n < 0) {
          rej(Object.assign(new Error("recv"), { errno: -n }));
        } else {
          res(b.slice(0, n));
        }
      };
      go();
    });
  }
  send(u8) {
    let at = 0;
    return new Promise((res, rej) => {
      const go = () => {
        while (at < u8.length) {
          const n = this.write(u8.subarray(at));
          if (n === -BP_ERRNO.EAGAIN) {
            this.parked[1].push({ fire: go });
            return;
          }
          if (n < 0) {
            rej(Object.assign(new Error("send"), { errno: -n }));
            return;
          }
          at += n;
        }
        res();
      };
      go();
    });
  }
}

const BP_H = globalThis.BP_H ?? (globalThis.BP_H = {
  next: BP_BASE,
  rows: new Map(),
  add(h) {
    const id = this.next;
    this.next += 1;
    h.refs += 1;
    this.rows.set(id, h);
    return id;
  },
  get(id) {
    return typeof id === "number" && id >= BP_BASE ? this.rows.get(id) : undefined;
  },
  is(id) {
    return typeof id === "number" && id >= BP_BASE;
  },
  dup(id) {
    const h = this.get(id);
    return h === undefined ? -1 : this.add(h);
  },
  close(id) {
    const h = this.rows.get(id);
    if (h !== undefined) {
      this.rows.delete(id);
      h.refs -= 1;
      if (h.refs === 0) {
        h.close();
      }
    }
  },
  Handle: BpHandle,
});
globalThis.bp_h_add = (h) => BP_H.add(h);
globalThis.bp_h_get = (id) => BP_H.get(id);

// The byte inbox every stream kind shares: chunks a source pushed, taken by
// read(); above HI bytes the source is paused, below LO resumed.
const BP_HI = 4 << 20;
const BP_LO = 1 << 20;

class BpStream extends BpHandle {
  constructor(kind) {
    super(kind);
    this.q = [];
    this.qsize = 0;
    this.eof = false;
    this.err = 0;
    this.rshut = false;
    this.paused = false;
  }
  feed(d) {
    if (this.rshut || d.length === 0) {
      return;
    }
    this.q.push(new Uint8Array(d));
    this.qsize += d.length;
    if (this.qsize > BP_HI && !this.paused) {
      this.paused = true;
      this.pause();
    }
    this.wake(false);
  }
  finish(err) {
    this.eof = true;
    if (err && this.err === 0) {
      this.err = err;
    }
    this.wakeAll();
  }
  pause() {
  }
  resume() {
  }
  ready(out) {
    return out ? this.writable() : this.qsize > 0 || this.eof || this.rshut;
  }
  writable() {
    return true;
  }
  read(b) {
    if (this.qsize === 0) {
      return this.rshut || this.eof && this.err === 0 ? 0
        : this.eof ? -this.err : -BP_ERRNO.EAGAIN;
    }
    let n = 0;
    while (n < b.length && this.q.length > 0) {
      const c = this.q[0];
      const take = Math.min(c.length, b.length - n);
      b.set(c.subarray(0, take), n);
      n += take;
      if (take === c.length) {
        this.q.shift();
      } else {
        this.q[0] = c.subarray(take);
      }
    }
    this.qsize -= n;
    if (this.paused && this.qsize < BP_LO) {
      this.paused = false;
      this.resume();
    }
    return n;
  }
  shutRead() {
    this.rshut = true;
    this.q = [];
    this.qsize = 0;
    if (this.paused) {
      this.paused = false;
      this.resume();
    }
    this.wake(false);
  }
}

// effect files are each their own scope: others reach these through BP_H
BP_H.Stream = BpStream;
BP_H.ERRNO = BP_ERRNO;
BP_H.errno = bp_errno;

// TCP
// ===

class BpTcp extends BpStream {
  constructor(sock) {
    super("tcp");
    this.s = sock;
    this.full = false;
    this.wshut = false;
    this.gone = false;
    const a = String(sock.remoteAddress ?? "");
    this.addr = a.startsWith("::ffff:") && a.includes(".") ? a.slice(7) : a;
  }
  pause() {
    this.s.pause();
  }
  resume() {
    this.s.resume();
  }
  writable() {
    return !this.full || this.gone || this.err !== 0;
  }
  write(b) {
    if (this.err !== 0) {
      return -this.err;
    }
    if (this.gone || this.wshut) {
      return -BP_ERRNO.EPIPE;
    }
    if (this.full) {
      return -BP_ERRNO.EAGAIN;
    }
    const n = this.s.write(b);
    if (n < 0) {
      return -BP_ERRNO.EPIPE;
    }
    if (n < b.length) {
      this.full = true;
    }
    return n > 0 ? n : -BP_ERRNO.EAGAIN;
  }
  drained() {
    this.full = false;
    this.wake(true);
  }
  peer() {
    return this.addr;
  }
  shutdown(how) {
    if (how !== 1) {
      this.shutRead();
    }
    if (how !== 0 && !this.wshut) {
      this.wshut = true;
      if (!this.gone) {
        this.s.shutdown();
      }
    }
    this.wake(true);
  }
  // the last id closed: the bytes already taken still go, then the FIN
  close() {
    this.rshut = true;
    this.q = [];
    if (!this.gone) {
      this.gone = true;
      this.s.end();
    }
    this.wakeAll();
  }
  abort() {
    if (!this.gone) {
      this.gone = true;
      this.s.terminate();
    }
    this.finish(BP_ERRNO.ECONNRESET);
  }
}

// A real descriptor (POSIX: a socketpair end a child got, a pty master)
// read and written through Bun's own streams, so nothing polls it, while
// the descriptor itself stays ours: h.fd is what an exec keeps (adopt.js)
// or Sock.raw duplicates. Reads are pulled only while the inbox is below
// HI; a write is taken whole and the handle is full until Bun flushed it.
class BpFd extends BpStream {
  constructor(fd, kind) {
    super(kind ?? "fd");
    this.fd = fd;
    this.full = false;
    this.wshut = false;
    this.gone = false;
    this.w = null;
    this.flushing = null;
    this.r = Bun.file(fd).stream().getReader();
    this.pull();
  }
  async pull() {
    try {
      for (;;) {
        if (this.paused) {
          await new Promise((res) => {
            this.unpause = res;
          });
          continue;
        }
        const { done, value } = await this.r.read();
        if (done || this.gone) {
          break;
        }
        this.feed(value);
      }
      this.finish(0);
    } catch (e) {
      // a pty whose other side closed reads EIO: the end of the stream
      this.finish(bp_errno(e) === BP_ERRNO.EIO ? 0 : bp_errno(e));
    }
  }
  pause() {
  }
  resume() {
    const f = this.unpause;
    this.unpause = null;
    f?.();
  }
  writable() {
    return !this.full || this.gone || this.err !== 0;
  }
  write(b) {
    if (this.err !== 0) {
      return -this.err;
    }
    if (this.gone || this.wshut) {
      return -BP_ERRNO.EPIPE;
    }
    if (this.full) {
      return -BP_ERRNO.EAGAIN;
    }
    // at most 64 KiB, and none while Bun still holds the last: a send to a
    // reader that does not read waits (as a full socket makes it wait), and
    // what it sent is all there before a shutdown or close (below)
    const c = b.subarray(0, 65536);
    try {
      this.w ??= Bun.file(this.fd).writer();
      this.w.write(c.slice());
      const f = this.w.flush();
      if (f instanceof Promise) {
        this.full = true;
        this.flushing = f.then(() => {
          this.full = false;
          this.flushing = null;
          this.wake(true);
        }, (e) => {
          this.full = false;
          this.flushing = null;
          this.err = bp_errno(e);
          this.wakeAll();
        });
      }
    } catch (e) {
      this.err = bp_errno(e);
      return -this.err;
    }
    return c.length;
  }
  // f once what was written has gone (at once when nothing waits)
  after(f) {
    if (this.flushing) {
      this.flushing.then(f, f);
    } else {
      f();
    }
  }
  shutdown(how) {
    if (how !== 1) {
      this.shutRead();
    }
    if (how !== 0 && !this.wshut) {
      this.wshut = true;
      this.after(() => bp_fd_shutdown(this.fd, 1));
    }
    this.wake(true);
  }
  close() {
    if (!this.gone) {
      this.gone = true;
      this.rshut = true;
      this.q = [];
      this.r.cancel().catch(() => {});
      const fd = this.fd;
      this.after(() => {
        try {
          require("fs").closeSync(fd);
        } catch (_) {
        }
      });
    }
    this.wakeAll();
  }
  abort() {
    if (!this.gone) {
      bp_fd_shutdown(this.fd, 2);
    }
    this.close();
  }
}

// shutdown(2) on a socket descriptor (POSIX only: BpFd is never made on
// Windows)
function bp_fd_shutdown(fd, how) {
  try {
    globalThis.BP_SHUT ??= require("bun:ffi").dlopen(process.platform === "darwin"
      ? "libSystem.dylib" : "libc.so.6",
      { shutdown: { args: ["i32", "i32"], returns: "i32" } }).symbols;
    globalThis.BP_SHUT.shutdown(fd, how);
  } catch (_) {
  }
}
BP_H.Fd = BpFd;
BP_H.fromFd = (fd, kind) => BP_H.add(new BpFd(fd, kind));

// one set of callbacks for every Bun socket, accepted or connected
const BP_SOCK = {
  open(s) {
    if (s.data === undefined || s.data.kind === "listener") {
      const l = s.data;
      s.data = new BpTcp(s);
      if (l !== undefined) {
        l.arrive(s.data);
      }
    }
  },
  data(s, d) {
    s.data.feed(d);
  },
  end(s) {
    s.data.finish(0);
  },
  close(s, e) {
    s.data.gone = true;
    s.data.finish(e ? bp_errno(e) : 0);
  },
  error(s, e) {
    s.data.finish(bp_errno(e));
  },
  drain(s) {
    s.data.drained();
  },
};

class BpListener extends BpHandle {
  constructor() {
    super("listener");
    this.q = [];
    this.server = null;
  }
  arrive(t) {
    this.q.push(t);
    this.wake(false);
  }
  ready(out) {
    return this.q.length > 0 || this.server === null;
  }
  close() {
    if (this.server !== null) {
      this.server.stop();
      this.server = null;
    }
    for (const t of this.q) {
      t.close();
    }
    this.q = [];
    this.wakeAll();
  }
}

// a listener on host:port, as host.c's Net.listen (Bun reuses the address;
// it keeps no backlog of its own, accepted sockets wait in q)
function bp_listen(host, port) {
  const l = new BpListener();
  try {
    l.server = Bun.listen({ hostname: host, port: Number(port),
      allowHalfOpen: true, data: l, socket: BP_SOCK });
  } catch (e) {
    return io_fail(bp_errno(e));
  }
  // accepted sockets start with the listener's data; BP_SOCK.open swaps it
  return io_done(BP_H.add(l));
}

function bp_accept_with(listener, k, at) {
  const l = BP_H.get(listener);
  const go = () => {
    if (l === undefined || l.server === null && l.q.length === 0) {
      return io_tup(listener, io_ready(at, io_fail(BP_ERRNO.EBADF)));
    }
    if (l.q.length > 0) {
      return io_tup(listener, io_ready(at, io_done(BP_H.add(l.q.shift()))));
    }
    if (io_late(at)) {
      return io_tup(listener, { $: "Wait", rest: { $: "Unit" } });
    }
    io_park_on(listener, false, k, go, at);
    return undefined;
  };
  return go();
}

function bp_connect(host, port, k) {
  const p = Bun.connect({ hostname: host, port: Number(port),
    allowHalfOpen: true, socket: BP_SOCK });
  return io_park_promise(p, k, (s, e) =>
    e !== undefined ? io_fail(bp_errno(e)) : io_done(BP_H.add(s.data)));
}

function bp_recv_with(socket, max, k, read, at) {
  if (Number(max) === 0) {
    return io_tup(socket, io_ready(at, io_fail(BP_ERRNO.EINVAL)));
  }
  const h = BP_H.get(socket);
  const b = new Uint8Array(Number(max));
  const go = () => {
    const n = h === undefined ? -BP_ERRNO.EBADF : h.read(b);
    if (n === -BP_ERRNO.EAGAIN) {
      if (io_late(at)) {
        return io_tup(socket, { $: "Wait", rest: { $: "Unit" } });
      }
      io_park_on(socket, false, k, go, at);
      return undefined;
    }
    if (n < 0) {
      return io_tup(socket, io_ready(at, io_fail(-n)));
    }
    // Base's TCP.recv answers Some/None; host.bend's Sock.recv_bytes the
    // bytes, none at the end (read is null there)
    const got = read === null ? io_list(b, n)
      : n === 0 ? { $: "None" } : { $: "Some", value: read(b, n) };
    return io_tup(socket, io_ready(at, io_done(got)));
  };
  return go();
}

// sends all of b; a full handle parks (until at, or for good)
function bp_send_with(socket, b, k, make, at) {
  const h = BP_H.get(socket);
  let off = 0;
  const go = () => {
    while (off < b.length) {
      const part = b.subarray(off);
      const n = h === undefined ? -BP_ERRNO.EBADF : h.write(part);
      if (n === -BP_ERRNO.EAGAIN) {
        if (io_late(at)) {
          return io_tup(socket, { $: "Wait", rest: make(part, part.length) });
        }
        io_park_on(socket, true, k, go, at);
        return undefined;
      }
      if (n < 0) {
        return io_tup(socket, io_ready(at, io_fail(-n, make(part, part.length))));
      }
      off += n;
    }
    return io_tup(socket, io_ready(at, io_done({ $: "Unit" })));
  };
  return go();
}

function bp_send_list(socket, data, k, at) {
  const b = io_unlist(data);
  return b === null ? io_tup(socket, io_ready(at, io_fail(BP_ERRNO.EINVAL, data)))
    : bp_send_with(socket, b, k, io_list, at);
}

// Base TCP/Socket/Listener: a handle here, a descriptor to the old twin
function bp_base(old) {
  // the old twin is taken now: $0eff[key] is about to be this wrapper
  const on = (key, f) => {
    const was = old[key];
    return [key, (x, ...r) => BP_H.is(x) ? f(x, ...r) : was(x, ...r)];
  };
  return Object.fromEntries([
    ["TCP.listen", (host, port) => bp_listen(host, port)],
    ["TCP.connect", (host, port, k) => bp_connect(host, port, k)],
    on("TCP.accept", (l, k) => bp_accept_with(l, k)),
    on("TCP.try_accept", (l, ms, k) => bp_accept_with(l, k, io_until(ms))),
    on("TCP.recv", (s, max, k) => bp_recv_with(s, max, k, io_text)),
    on("TCP.recv_bytes", (s, max, k) => bp_recv_with(s, max, k, io_list)),
    on("TCP.try_recv", (s, max, ms, k) =>
      bp_recv_with(s, max, k, io_text, io_until(ms))),
    on("TCP.try_recv_bytes", (s, max, ms, k) =>
      bp_recv_with(s, max, k, io_list, io_until(ms))),
    on("TCP.send", (s, d, k) => bp_send_with(s, io_bytes(d), k, io_text)),
    on("TCP.send_bytes", (s, d, k) => bp_send_list(s, d, k)),
    on("TCP.try_send", (s, d, ms, k) =>
      bp_send_with(s, io_bytes(d), k, io_text, io_until(ms))),
    on("TCP.try_send_bytes", (s, d, ms, k) => bp_send_list(s, d, k, io_until(ms))),
    on("Socket.close", (s) => (BP_H.close(s), { $: "Unit" })),
    on("Listener.close", (s) => (BP_H.close(s), { $: "Unit" })),
  ].filter(([key]) => key in old));
}

// host.bend's Sock/Fd/Net
// =======================

function bp_list(b) {
  let xs = { $: "Nil" };
  for (let i = b.length - 1; i >= 0; i -= 1) {
    xs = { $: "Con", head: b[i], tail: xs };
  }
  return xs;
}

// Sock.send_until: the deadline renews after each send that made progress;
// a stall for the whole span (or a failure) shuts the socket both ways
function bp_send_until(socket, b, ms, k) {
  const h = BP_H.get(socket);
  const span = Number(ms);
  let until = performance.now() + span;
  let at = 0;
  const go = () => {
    while (at < b.length) {
      const n = h === undefined ? -BP_ERRNO.EBADF : h.write(b.subarray(at));
      if (n === -BP_ERRNO.EAGAIN && performance.now() < until) {
        io_park_on(socket, true, k, go, until);
        return undefined;
      }
      if (n < 0) {
        if (h !== undefined) {
          h.shutdown(2);
        }
        return io_tup(socket, io_fail(n === -BP_ERRNO.EAGAIN ? BP_ERRNO.ETIMEDOUT : -n));
      }
      at += n;
      until = performance.now() + span;
    }
    return io_tup(socket, io_done({ $: "Unit" }));
  };
  return go();
}

function bp_sock_save(socket, path, n, k) {
  const fs = require("fs");
  const h = BP_H.get(socket);
  const b = new Uint8Array(65536);
  let left = Number(n);
  let out;
  try {
    out = fs.openSync(path, "a", 0o600);
  } catch (e) {
    return io_tup(socket, io_fail(bp_errno(e)));
  }
  const go = () => {
    while (left > 0) {
      const got = h === undefined ? -BP_ERRNO.EBADF
        : h.read(b.subarray(0, Math.min(left, b.length)));
      if (got === -BP_ERRNO.EAGAIN) {
        io_park_on(socket, false, k, go);
        return undefined;
      }
      if (got <= 0) {
        fs.closeSync(out);
        return io_tup(socket, io_fail(got === 0 ? BP_ERRNO.EPIPE : -got));
      }
      fs.writeSync(out, b.subarray(0, got));
      left -= got;
    }
    fs.closeSync(out);
    return io_tup(socket, io_done({ $: "Unit" }));
  };
  return go();
}

function bp_host(old) {
  const key = (name) => Object.keys(old).find((x) => x.endsWith("." + name));
  const frame = (name, ...a) => io_unlist(old[key(name)](...a));
  const pairs = [
    ["Net.listen", () => (host, port) => bp_listen(host, port)],
    ["Sock.recv_bytes", () => (s, max, k) => bp_recv_with(s, max, k, null)],
    ["Sock.save", () => bp_sock_save],
    ["Sock.send_bytes", () => (s, d, k) => {
      const b = io_unlist(d);
      return b === null ? io_tup(s, io_fail(BP_ERRNO.EINVAL))
        : bp_send_with(s, b, k, io_list);
    }],
    ["Sock.send_text", () => (s, t, k) => bp_send_with(s, io_bytes(t), k, io_text)],
    ["Sock.send_until", () => (s, d, ms, k) => {
      const b = io_unlist(d);
      return b === null ? io_tup(s, io_fail(BP_ERRNO.EINVAL))
        : bp_send_until(s, b, ms, k);
    }],
    ["Sock.send_cbor", () => (s, text, keys, words, ms, k) =>
      bp_send_until(s, frame("Json.cbor_frame", text, keys, words), ms, k)],
    ["Sock.send_items", () => (s, hd, plain, split, lines, tl, ends, most, bytes,
      keys, words, ms, k) => bp_send_until(s, frame("Json.items_frames", hd,
      plain, split, lines, tl, ends, most, bytes, keys, words), ms, k)],
    // a second id on the same socket, each frame sent at once and a silent
    // peer found by keepalive (Bun has no TCP_USER_TIMEOUT)
    ["Sock.dup", () => (s) => {
      const h = BP_H.get(s);
      if (h === undefined) {
        return io_tup(s, io_fail(BP_ERRNO.EBADF));
      }
      if (h.kind === "tcp" && !h.gone) {
        h.s.setNoDelay(true);
        h.s.setKeepAlive(true, 20000);
      }
      return io_tup(s, io_done(BP_H.add(h)));
    }],
    ["Sock.peer", () => (s) => io_tup(s, BP_H.get(s)?.peer() ?? "")],
    ["Sock.shutdown", () => (s) => (BP_H.get(s)?.shutdown(1), s)],
    ["Sock.shut_read", () => (s) => (BP_H.get(s)?.shutdown(0), s)],
    // a handle answers another id on itself (its only use is the hub's
    // Fd.shut/Fd.close of a client from outside its writer), so Fd.shut
    // aborts the handle: a raw dup of a BpFd's descriptor, shut, left Bun's
    // pending flush parked for good
    ["Sock.raw", () => (s) => io_tup(s, BP_H.get(s) === undefined ? 0 : BP_H.dup(s))],
    ["Fd.shut", () => (s) => {
      const h = BP_H.get(s);
      if (h !== undefined) {
        h.abort?.();
        BP_H.close(s);
      }
      return { $: "Unit" };
    }],
    ["Fd.close", () => (s) => (BP_H.close(s), { $: "Unit" })],
  ];
  const out = {};
  for (const [name, make] of pairs) {
    const k = key(name);
    if (k === undefined) {
      continue;
    }
    const was = old[k];
    const f = make();
    // Net.listen always answers a handle; the rest dispatch on their first
    // argument, a descriptor going to the old twin
    out[k] = name === "Net.listen" ? f
      : (x, ...r) => BP_H.is(x) ? f(x, ...r) : was(x, ...r);
  }
  return out;
}

// Base File and Process
// =====================

function bp_file(old) {
  const fs = require("fs");
  const read = (file, max, offset, pack) => {
    const len = Math.min(Number(max), 2147483647);
    const b = Buffer.alloc(Math.max(len, 1));
    try {
      const n = fs.readSync(file, b, 0, len, offset === null ? null : Number(offset));
      return io_tup(file, io_done(pack(b, n)));
    } catch (e) {
      return io_tup(file, io_fail(bp_errno(e)));
    }
  };
  return Object.fromEntries([
    ["File.read", (f, max) => read(f, max, null, io_text)],
    ["File.read_bytes", (f, max) => read(f, max, null, io_list)],
    ["File.read_at", (f, offset, max) => read(f, max, offset, io_list)],
  ].filter(([key]) => key in old));
}

// Process.run without blocking the loop: Bun.spawn, output capped at
// maxOutput, killed at timeoutMs (ETIMEDOUT), as process_run.js answers
function bp_process_run(program, args, input, maxOutput, timeoutMs, k) {
  const argv = [program];
  for (let xs = args; xs.$ === "Con"; xs = xs.tail) {
    argv.push(xs.head);
  }
  if (maxOutput === 0 || timeoutMs === 0 || argv.some((a) => a.includes("\0"))) {
    return io_fail(BP_ERRNO.EINVAL);
  }
  let child;
  try {
    child = Bun.spawn(argv, { stdin: "pipe", stdout: "pipe", stderr: "pipe" });
  } catch (e) {
    return io_fail(bp_errno(e));
  }
  let size = 0;
  let over = false;
  let late = false;
  const take = async (stream) => {
    const parts = [];
    for await (const c of stream) {
      size += c.length;
      if (size > maxOutput) {
        over = true;
        child.kill(9);
        break;
      }
      parts.push(c);
    }
    return Buffer.concat(parts).toString("utf8");
  };
  const timer = setTimeout(() => {
    late = true;
    child.kill(9);
  }, Number(timeoutMs));
  child.stdin.write(input);
  child.stdin.end();
  const all = Promise.all([take(child.stdout), take(child.stderr), child.exited]);
  return io_park_promise(all, k, (v, e) => {
    clearTimeout(timer);
    if (e !== undefined) {
      return io_fail(bp_errno(e));
    }
    if (over) {
      return io_fail(BP_ERRNO.EFBIG);
    }
    if (late) {
      return io_fail(BP_ERRNO.ETIMEDOUT);
    }
    const sig = child.signalCode === null ? 0
      : require("os").constants.signals[child.signalCode] ?? 0;
    const code = child.exitCode ?? 128 + sig;
    return io_done(io_tup(code, v[0], v[1]));
  });
}

// Loop
// ====

// Wakes the loop when a park settles while it awaits Bun.
function bp_wake() {
  const io = globalThis.BEND_IO;
  if (io.wake !== null) {
    const f = io.wake;
    io.wake = null;
    f();
  }
}

// k resumes with more()'s value; more runs inside the loop, so a throw ends
// the run as any effect's does
function bp_resume(k, more) {
  io_push((_) => {
    const x = more();
    return x === undefined ? undefined : k(x);
  }, null, false);
  bp_wake();
}

function io_park_promise(p, k, then) {
  const io = globalThis.BEND_IO;
  io.pend += 1;
  Promise.resolve(p).then((v) => {
    io.pend -= 1;
    bp_resume(k, () => then(v));
  }, (e) => {
    io.pend -= 1;
    bp_resume(k, () => then(undefined, e ?? new Error("rejected")));
  });
  return undefined;
}
globalThis.io_park_promise = io_park_promise;

// A handle parks until ready(out) (pend counts it: Bun must turn), with a
// deadline as a timed wait whose firing cuts it, and the other way round.
function bp_park_handle(id, out, k, more, at) {
  const io = globalThis.BEND_IO;
  const h = BP_H.get(id);
  if (h === undefined || h.ready(out)) {
    bp_resume(k, more);
    return;
  }
  const w = { done: false };
  io.pend += 1;
  w.fire = () => {
    if (w.done) {
      return;
    }
    w.done = true;
    io.pend -= 1;
    if (w.late !== undefined) {
      const i = io.waits.findIndex((t) => t.more === w.late);
      if (i >= 0) {
        io.waits.splice(i, 1);
      }
    }
    bp_resume(k, more);
  };
  if (at !== undefined) {
    w.late = () => {
      w.done = true;
      io.pend -= 1;
      const ws = h.parked[out ? 1 : 0];
      const i = ws.indexOf(w);
      if (i >= 0) {
        ws.splice(i, 1);
      }
      return more();
    };
    bp_sync_park(undefined, false, k, w.late, at);
  }
  h.parked[out ? 1 : 0].push(w);
}

if (BP_ASYNC) {
  io_park_on = function (fd, out, k, more, at) {
    if (BP_H.is(fd)) {
      bp_park_handle(fd, out, k, more, at);
      return;
    }
    if (fd !== undefined && BP_WIN) {
      throw "bend: a descriptor park on win32 (fd " + fd + ")";
    }
    bp_sync_park(fd, out, k, more, at);
  };
}

// Descriptor waits through poll(), not the runtime's select(): select puts
// a unix socket whose peer hung up (POLLHUP alone) in neither set, so a
// send parked on a full socketpair slept until the child exited. A
// hang-up, an error or a closed descriptor is ready both ways: the
// computation's retry then meets EPIPE/EBADF at once. ms: -1 waits for
// good, 0 not at all. Deadlines are due by the clock.
function bp_poll(io, ms) {
  globalThis.BP_POLL ??= require("bun:ffi").dlopen(process.platform === "darwin"
    ? "libSystem.dylib" : "libc.so.6",
    { poll: { args: ["ptr", "u32", "i32"], returns: "i32" } }).symbols;
  const fds = io.waits.filter((w) => w.fd !== undefined);
  // struct pollfd: int fd; short events; short revents
  const set = new Int16Array(4 * fds.length);
  const view = new Int32Array(set.buffer);
  fds.forEach((w, i) => {
    view[2 * i] = w.fd;
    // POLLIN 1, POLLOUT 4
    set[4 * i + 2] = w.out ? 4 : 1;
  });
  const n = fds.length === 0 ? 0
    : globalThis.BP_POLL.poll(require("bun:ffi").ptr(set), fds.length, ms);
  const ready = new Set();
  if (n > 0) {
    fds.forEach((w, i) => {
      // the asked event, or POLLERR 8 | POLLHUP 16 | POLLNVAL 32
      if (set[4 * i + 3] & (set[4 * i + 2] | 8 | 16 | 32)) {
        ready.add(w);
      }
    });
  }
  const now = performance.now();
  const due = (w) => ready.has(w) || w.at !== undefined && w.at <= now;
  const todo = io.waits;
  io.waits = todo.filter((w) => !due(w));
  for (const w of todo.filter(due)) {
    const x = w.more();
    if (x !== undefined) {
      io_push(w.k, x, false);
    }
  }
}

// Runs what is due without waiting: descriptors through poll() (POSIX),
// deadlines by the clock.
function bp_due(io) {
  if (io.waits.some((w) => w.fd !== undefined)) {
    bp_poll(io, 0);
    return;
  }
  const now = performance.now();
  while (io.waits.length > 0 && io.waits[0].at <= now) {
    const w = io.waits.shift();
    const x = w.more();
    if (x !== undefined) {
      io_push(w.k, x, false);
    }
  }
}

// Awaits Bun until a park settles or the nearest deadline; while
// descriptors wait too, polls them every tick (1 ms, doubling to 8 while
// idle). With nothing parked on Bun, descriptors alone block in poll().
async function bp_idle(io) {
  const fds = io.waits.some((w) => w.fd !== undefined);
  if (fds && io.pend === 0) {
    const soon = io.waits.reduce((m, w) => Math.min(m, w.at ?? Infinity), Infinity);
    bp_poll(io, soon === Infinity ? -1
      : Math.max(0, Math.ceil(soon - performance.now())));
    return;
  }
  const soon = io.waits.reduce((m, w) => Math.min(m, w.at ?? Infinity), Infinity);
  let ms = soon === Infinity ? Infinity : Math.max(0, soon - performance.now());
  if (fds) {
    ms = Math.min(ms, io.tick);
    io.tick = Math.min(io.tick * 2, 8);
  }
  await new Promise((res) => {
    io.wake = res;
    if (ms !== Infinity) {
      io.timer = setTimeout(res, Math.ceil(ms));
    }
  });
  io.wake = null;
  clearTimeout(io.timer);
  bp_due(io);
}

async function bp_run(m) {
  const io = { runs: [], live: 0, waits: [], pend: 0, wake: null,
    timer: undefined, tick: 1, exit: null };
  globalThis.BEND_IO = io;
  Object.assign($0eff, bp_base($0eff), bp_host($0eff), bp_file($0eff));
  if ("Process.run" in $0eff) {
    $0eff["Process.run"] = bp_process_run;
  }
  // a run whose computations all wait on nothing Bun could finish ends as
  // the old loop's deadlock did
  const stuck = () => {
    if (io.exit === null) {
      io_errs("bend: deadlock: every computation waits on a channel");
      process.exit(1);
    }
  };
  process.on("beforeExit", stuck);
  io_push(run_loop(m()), (x) => ({ $: "Emit", value: x }), true);
  let slice = performance.now() + 4;
  for (let n = 0;; n += 1) {
    if (io.runs.length === 0) {
      if (io.live === 0) {
        io.exit = 0;
        return 0;
      }
      if (io.waits.length === 0 && io.pend === 0) {
        io_errs("bend: deadlock: every computation waits on a channel");
        io.exit = 1;
        return 1;
      }
      await bp_idle(io);
      slice = performance.now() + 4;
      continue;
    }
    if ((n & 63) === 0 && performance.now() >= slice) {
      io.tick = 1;
      await new Promise((res) => setImmediate(res));
      bp_due(io);
      slice = performance.now() + 4;
    }
    const s = io.runs.shift();
    let op = s.fun(s.arg);
    while (op !== undefined) {
      if (op.$ === "Emit") {
        io.live -= 1;
        break;
      }
      if (op.$ === "Halt") {
        io_errs(op.message);
        io.exit = op.code;
        return op.code;
      }
      const run = $0eff[op.$];
      if (run === undefined) {
        throw "bend: no effect registers " + op.$;
      }
      const x = run(...op.args, op.kont);
      if (x === undefined) {
        break;
      }
      op = op.kont(x);
    }
  }
}

if (BP_ASYNC) {
  const sync_exit = io_exit;
  io_exit = function (main, show) {
    if (show !== null) {
      return sync_exit(main, show);
    }
    bp_run(main).then((code) => process.exit(code), (e) => {
      io_errs(e instanceof RangeError
        ? "bend: memory fault (machine stack overflow?)" : String(e));
      process.exit(1);
    });
  };
}

function rt_loop() {
  return BP_ASYNC ? "bun" : "sync";
}

io_eff(CID(Rt.loop), rt_loop);
