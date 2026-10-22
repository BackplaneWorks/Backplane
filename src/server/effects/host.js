// Backplane host effects: the JS twin of host.c.
//
// Bend's JS runtime is one synchronous loop that waits on descriptors with
// select() (through bun:ffi); Bun's own event loop never runs, so nothing
// here uses a Promise, a callback or a blocking call. Children start with
// posix_spawnp and are waited for on a pidfd the loop polls, sockets park
// on the loop when full, and everything else is a plain syscall.

// libc through bun:ffi: the symbols every libc has, then the optional ones
// one at a time (a missing one is absent, never a failed load)
function host_libc() {
  if (globalThis.BACKPLANE_LIBC === undefined) {
    const ffi = require("bun:ffi");
    const mac = process.platform === "darwin";
    const lib = mac ? "libSystem.dylib" : "libc.so.6";
    const T = { i: "i32", u: "u32", h: "i16", U: "u64", I: "i64", p: "ptr" };
    const spec = (list) => Object.fromEntries(list.split(" ").map((x) => {
      const [name, args, ret] = x.split(/[:>]/);
      return [name, { args: [...args].map((a) => T[a]), returns: T[ret] }];
    }));
    const c = ffi.dlopen(lib, spec("dup:i>i shutdown:ii>i close:i>i"
      + " write:ipU>I socketpair:iiip>i pipe:p>i kill:ii>i"
      + " waitpid:ipi>i waitid:iupi>i getpeername:ipp>i inet_ntop:ippu>p"
      + " execve:ppp>i setenv:ppi>i unsetenv:p>i"
      + " posix_spawnp:pppppp>i posix_spawn:pppppp>i"
      + " posix_spawn_file_actions_init:p>i posix_spawn_file_actions_destroy:p>i"
      + " posix_spawn_file_actions_adddup2:pii>i posix_spawn_file_actions_addclose:pi>i"
      + " posix_spawn_file_actions_addopen:pipiu>i"
      + " posix_spawnattr_init:p>i posix_spawnattr_destroy:p>i"
      + " posix_spawnattr_setflags:ph>i posix_spawnattr_setsigdefault:pp>i"
      + " posix_spawnattr_setsigmask:pp>i sigemptyset:p>i sigaddset:pi>i sigfillset:p>i")).symbols;
    const opt = (one) => {
      try {
        return Object.values(ffi.dlopen(lib, spec(one)).symbols)[0];
      } catch (_) {
        return undefined;
      }
    };
    c.addchdir = opt("posix_spawn_file_actions_addchdir_np:pp>i");
    c.addclosefrom = opt("posix_spawn_file_actions_addclosefrom_np:pi>i");
    c.pidfd_open = mac ? undefined : opt("pidfd_open:iu>i");
    c.mac = mac;
    c.ptr = ffi.ptr;
    c.read_ptr = ffi.read;
    globalThis.BACKPLANE_LIBC = c;
  }
  return globalThis.BACKPLANE_LIBC;
}

// a String as a NUL-terminated buffer (null when it holds a NUL)
function host_cstr(s) {
  return s.includes("\0") ? null : Buffer.from(s + "\0", "utf8");
}

// strings as a NULL-terminated char* vector; bufs keeps them alive
function host_vec(strs) {
  const bufs = strs.map(host_cstr);
  if (bufs.some((b) => b === null)) {
    return null;
  }
  const c = host_libc();
  const vec = new BigUint64Array(bufs.length + 1);
  bufs.forEach((b, i) => { vec[i] = BigInt(c.ptr(b)); });
  return { bufs, vec };
}

// this process's environment, as the C runtime's environ is (Env.put
// writes process.env); extra names override
function host_envp(extra) {
  const env = { ...process.env, ...(extra ?? {}) };
  return Object.entries(env).filter(([k, v]) => v !== undefined && k !== ""
    && !k.includes("=")).map(([k, v]) => k + "=" + v)
    .filter((kv) => !kv.includes("\0"));
}

function host_eilseq() {
  return process.platform === "darwin" ? 92 : 84;
}

function host_list_js(xs) {
  const out = [];
  for (; xs.$ === "Con"; xs = xs.tail) {
    out.push(xs.head);
  }
  return out;
}

function host_errno(e) {
  return Math.abs(e?.errno ?? 5);
}

function clock_epoch() {
  return Math.floor(Date.now() / 1000) >>> 0;
}

function host_list(xs) {
  let out = { $: "Nil" };
  for (let i = xs.length; i > 0; i -= 1) {
    out = { $: "Con", head: xs[i - 1], tail: out };
  }
  return out;
}

// Socket handles may also wrap a pty master (see pty.c): recv and send
// answer ENOTSOCK there, so they fall back to read and write. A pty whose
// other side has closed reads EIO, which means end of stream.
function host_read(fd, p, n) {
  const sys = io_sys();
  let r = Number(sys.recv(fd, p, BigInt(n), 0));
  if (r < 0 && sys.errno() === (sys.mac ? 38 : 88)) {
    r = Number(sys.read(fd, p, BigInt(n)));
    if (r < 0 && sys.errno() === 5) {
      r = 0;
    }
  }
  return r;
}

function host_write(fd, p, n) {
  const sys = io_sys();
  let r = Number(sys.send(fd, p, BigInt(n), sys.mac ? 0 : 0x4000));
  if (r < 0 && sys.errno() === (sys.mac ? 38 : 88)) {
    r = Number(host_libc().write(fd, p, BigInt(n)));
  }
  return r;
}

function sock_recv_bytes(socket, max, k) {
  const sys = io_sys();
  const b = new Uint8Array(Math.max(Number(max), 1));
  const again = sys.mac ? 35 : 11;
  const go = () => {
    const n = host_read(socket, sys.ptr(b), b.length);
    if (n < 0) {
      const code = sys.errno();
      if (code === again) {
        io_park_on(socket, false, k, go);
        return undefined;
      }
      return io_tup(socket, io_fail(code));
    }
    return io_tup(socket, io_done(host_list(Array.from(b.subarray(0, n)))));
  };
  // park before the first read, as the C twin does: a socket with a steady
  // backlog must not keep the loop from polling other connections and timers
  io_park_on(socket, false, k, go);
  return undefined;
}

// the next n bytes of the socket appended to the file at path
function sock_save(socket, path, n, k) {
  const sys = io_sys();
  const fs = require("fs");
  const b = new Uint8Array(65536);
  const again = sys.mac ? 35 : 11;
  let left = Number(n);
  let out;
  try {
    out = fs.openSync(path, "a", 0o600);
  } catch (err) {
    return io_tup(socket, io_fail(host_errno(err)));
  }
  const go = () => {
    while (left > 0) {
      const got = host_read(socket, sys.ptr(b), Math.min(left, b.length));
      if (got < 0 && sys.errno() === again) {
        io_park_on(socket, false, k, go);
        return undefined;
      }
      if (got <= 0) {
        const code = got === 0 ? 32 : sys.errno();
        fs.closeSync(out);
        return io_tup(socket, io_fail(code));
      }
      // a write that fails (a full disk) is the save's failure, as in C,
      // never a throw out of the loop
      try {
        let at = 0;
        while (at < got) {
          at += fs.writeSync(out, b.subarray(at, got));
        }
      } catch (err) {
        try {
          fs.closeSync(out);
        } catch (_) {
        }
        return io_tup(socket, io_fail(host_errno(err)));
      }
      left -= got;
    }
    fs.closeSync(out);
    return io_tup(socket, io_done({ $: "Unit" }));
  };
  if (left <= 0) return go();
  io_park_on(socket, false, k, go);
  return undefined;
}

// Sends all of b; a full socket parks until writable (never a spin), as
// host.c's host_send_more does. A parked send also tries again each second:
// the loop waits with select(), which never reports a socketpair whose
// connection was shut (Fd.shut) as writable (only POLLHUP, which C's poll
// sees), so the send would wait for the peer to exit; the retry fails with
// EPIPE at once.
function host_park_out(socket, k, go, until) {
  const tick = performance.now() + 1000;
  io_park_on(socket, true, k, go, until === undefined ? tick : Math.min(until, tick));
}

function host_send(socket, b, k) {
  const sys = io_sys();
  const again = sys.mac ? 35 : 11;
  let at = 0;
  const go = () => {
    while (at < b.length) {
      const n = host_write(socket, sys.ptr(b, at), b.length - at);
      if (n < 0) {
        const code = sys.errno();
        if (code === again) {
          host_park_out(socket, k, go);
          return undefined;
        }
        return io_tup(socket, io_fail(code));
      }
      at += n;
    }
    return io_tup(socket, io_done({ $: "Unit" }));
  };
  return go();
}

function sock_send_bytes(socket, data, k) {
  const bytes = [];
  for (let xs = data; xs.$ === "Con"; xs = xs.tail) {
    bytes.push(xs.head);
  }
  if (bytes.some((x) => x > 255)) {
    return io_tup(socket, io_fail(22));
  }
  return host_send(socket, Uint8Array.from(bytes), k);
}

// Sock.send_until: a deadline renewed after every send that made progress;
// a full socket parks on the socket and the clock (never a spin), and a
// stall for the whole span (or a failure) shuts the socket both ways
function sock_send_until(socket, data, ms, k) {
  const bytes = [];
  for (let xs = data; xs.$ === "Con"; xs = xs.tail) {
    bytes.push(xs.head);
  }
  if (bytes.some((x) => x > 255)) {
    return io_tup(socket, io_fail(22));
  }
  const sys = io_sys();
  const again = sys.mac ? 35 : 11;
  const b = Uint8Array.from(bytes);
  const span = Number(ms);
  let until = performance.now() + span;
  let at = 0;
  const go = () => {
    while (at < b.length) {
      const n = host_write(socket, sys.ptr(b, at), b.length - at);
      if (n < 0) {
        const code = sys.errno();
        if (code === again && performance.now() < until) {
          host_park_out(socket, k, go, until);
          return undefined;
        }
        host_libc().shutdown(socket, 2);
        return io_tup(socket, io_fail(code === again ? (sys.mac ? 60 : 110) : code));
      }
      at += n;
      until = performance.now() + span;
    }
    return io_tup(socket, io_done({ $: "Unit" }));
  };
  return go();
}

function sock_send_text(socket, text, k) {
  return host_send(socket, io_bytes(text), k);
}

// A second handle on the same socket (close-on-exec), each frame sent at
// once (TCP_NODELAY) and a silent peer found by keepalive and
// TCP_USER_TIMEOUT, as host.c's Sock.dup; a socketpair refuses the
// options, which changes nothing.
function sock_dup(socket) {
  const sys = io_sys();
  const opt = (level, name, v) => {
    const x = new Int32Array([v]);
    sys.setsockopt(socket, level, name, sys.ptr(x), 4);
  };
  opt(6, 1, 1);
  opt(sys.mac ? 0xffff : 1, sys.mac ? 8 : 9, 1);
  if (!sys.mac) {
    opt(6, 4, 20);
    opt(6, 5, 5);
    opt(6, 6, 3);
    opt(6, 18, 30000);
  }
  const got = sys.fcntl(socket, sys.mac ? 67 : 1030, 0);
  return io_tup(socket, got < 0 ? io_fail(sys.errno()) : io_done(got));
}

// The peer's IP address as text ("" when it has none, e.g. a socketpair).
function sock_peer(socket) {
  const c = host_libc();
  const a = new Uint8Array(128);
  const n = new Uint32Array([128]);
  const out = new Uint8Array(64);
  if (c.getpeername(socket, c.ptr(a), c.ptr(n)) === 0) {
    const fam = c.mac ? a[1] : a[0] | a[1] << 8;
    if (fam === 2) {
      c.inet_ntop(2, c.ptr(a, 4), c.ptr(out), 64);
    } else if (fam === (c.mac ? 30 : 10)) {
      c.inet_ntop(fam, c.ptr(a, 8), c.ptr(out), 64);
    }
  }
  const end = out.indexOf(0);
  return io_tup(socket, Buffer.from(out.subarray(0, end < 0 ? 0 : end)).toString("latin1"));
}

function sock_shutdown(socket) {
  host_libc().shutdown(socket, 1);
  return socket;
}

// Windows: the package's own POSIX runtime (portable Cygwin: sh.exe, bash.exe,
// env.exe, cygwin1.dll) sits in runtime\bin beside the hub's executable
// (resources\backplane-service\runtime\bin). It goes first on the hub's PATH
// at load, however the hub was started (the app, a shortcut, a shell), so
// Proc.run's sh/env, the default SHELL (/bin/bash, bp_win_cmd) and every
// child's environment find it. Nothing changes when it is not there.
(() => {
  if (process.platform !== "win32") {
    return;
  }
  try {
    const path = require("path");
    const bin = path.join(path.dirname(process.execPath), "runtime", "bin");
    if (!require("fs").statSync(bin).isDirectory()) {
      return;
    }
    // runtime\ is the Cygwin root: its /tmp is runtime\tmp, which bash
    // checks and a ZIP does not keep empty (service.cjs makes it too, for a
    // hub the app starts; this is for one started any other way)
    try {
      require("fs").mkdirSync(path.join(path.dirname(process.execPath), "runtime", "tmp"), { recursive: true });
    } catch (_) {
    }
    const key = Object.keys(process.env).find((k) => k.toUpperCase() === "PATH") ?? "PATH";
    const was = process.env[key] ?? "";
    if (!was.toLowerCase().split(";").includes(bin.toLowerCase())) {
      process.env[key] = was === "" ? bin : bin + ";" + was;
    }
  } catch (_) {
  }
})();

// Proc
// ====

// A child starts with a soft stack limit of 8 MiB at most. The hub runs with
// a large one (LimitSTACK=4G: Bun's JS stack is bounded by it, and Bend's
// deep walks need it), which children would inherit, and glibc sizes every
// thread's stack from it: each agent's threads would reserve gigabytes.
// The soft limit is lowered around the spawn and put back; the hub's own
// stack is untouched (lowering a limit unmaps nothing, and the loop runs
// spawns from a shallow stack). Hard limit unchanged.
function host_child_stack(spawn) {
  let lib;
  try {
    lib = globalThis.BACKPLANE_RLIMIT ?? (globalThis.BACKPLANE_RLIMIT = require("bun:ffi").dlopen(
      process.platform === "darwin" ? "libSystem.dylib" : "libc.so.6", {
        getrlimit: { args: ["i32", "ptr"], returns: "i32" },
        setrlimit: { args: ["i32", "ptr"], returns: "i32" },
      }).symbols);
  } catch (_) {
    return spawn();
  }
  const ptr = require("bun:ffi").ptr;
  const was = new BigUint64Array(2);
  const want = 8n << 20n;
  // RLIMIT_STACK is 3 on Linux and macOS; RLIM_INFINITY is all ones
  if (lib.getrlimit(3, ptr(was)) !== 0 || was[0] <= want) {
    return spawn();
  }
  const low = new BigUint64Array([want, was[1]]);
  const lowered = lib.setrlimit(3, ptr(low)) === 0;
  try {
    return spawn();
  } finally {
    if (lowered) {
      lib.setrlimit(3, ptr(was));
    }
  }
}

// Children on Bun's own APIs
// --------------------------
// Where the runtime runs on Bun's event loop (loop.js: BEND_ASYNC, always on
// Windows), children start with Bun.spawn and their stdin and stdout are one
// BP_H handle (a BpStream), as host.c's socketpair end is one Socket:
// reading its stdout (with stderr when err is 1), writing its stdin (held
// back while Bun's pipe is full), Sock.shutdown ending its stdin, closing
// both. Proc.wait parks on the child's exit (io_park_promise). The libc
// backend below stays for the old loop (BACKPLANE_JS_LOOP=sync, POSIX).
// Chosen at each call: effect files load in no fixed order.
function bp_proc_bun() {
  return (process.platform === "win32" || process.env.BACKPLANE_JS_PROC === "bun")
    && globalThis.BP_H?.Stream !== undefined;
}

// On POSIX the libc spawn stays (a real socketpair end: an exec in place
// keeps it, adopt.js hands it on); on Bun's loop that end is a BP_H handle
// over the descriptor (loop.js's BpFd: read and written through Bun's own
// streams, nothing polled), and Proc.wait waits on SIGCHLD.
function bp_loop_bun() {
  return globalThis.BEND_ASYNC === true && globalThis.BP_H?.fromFd !== undefined;
}

// Proc.wait on Bun's loop: the waited pids, reaped (waitpid WNOHANG) at
// each SIGCHLD, which Bun delivers to process.on (several exits may share
// one signal: every pending pid is tried). Bun reaps only its own children.
// Under the sync loop the signal never fires (Bun's loop does not turn):
// there Proc.wait parks on a pidfd as before.
function bp_chld() {
  if (globalThis.BP_CHLD === undefined) {
    // a signal listener and a pending Promise do not keep Bun's loop alive:
    // while any pid is waited for, a timer does (sweeping once a second
    // too, should a signal be missed)
    const g = { want: new Map(), timer: null };
    g.sweep = () => {
      for (const [pid, done] of g.want) {
        const code = host_reap(pid);
        if (code !== undefined) {
          g.want.delete(pid);
          for (const f of done) {
            f(code);
          }
        }
      }
      if (g.want.size === 0 && g.timer !== null) {
        clearInterval(g.timer);
        g.timer = null;
      } else if (g.want.size > 0 && g.timer === null) {
        g.timer = setInterval(g.sweep, 1000);
      }
    };
    process.on("SIGCHLD", g.sweep);
    globalThis.BP_CHLD = g;
  }
  return globalThis.BP_CHLD;
}

function bp_chld_wait(pid, k) {
  const now = host_reap(pid);
  if (now !== undefined) {
    return now;
  }
  const g = bp_chld();
  const p = new Promise((res) => {
    const list = g.want.get(pid) ?? [];
    list.push(res);
    g.want.set(pid, list);
  });
  // an exit between the reap above and the listener: tried once more
  g.sweep();
  return io_park_promise(p, k, (code) => code);
}

// the children Bun started, by pid, until Proc.wait has answered
function bp_procs() {
  return globalThis.BP_PROCS ?? (globalThis.BP_PROCS = new Map());
}

function bp_proc_errno(e) {
  return globalThis.BP_H?.errno ? globalThis.BP_H.errno(e) : Math.abs(e?.errno ?? 5);
}

// a child's stdio as one handle; open counts the streams still feeding it.
// Made at first use: loop.js's BpStream may load after this file.
function bp_proc_class() {
  if (globalThis.BP_PROC_CLASS !== undefined) {
    return globalThis.BP_PROC_CLASS;
  }
  // node:child_process's streams, not Bun.spawn's FileSink: a FileSink write
  // to a stdin the child closed never fails to the caller (Bun prints the
  // EPIPE later, flush answers 0, the send reads as sent) and, on Windows,
  // its held flush never settles (the hub's loop ended as a deadlock). A
  // node Writable reports it to the write's callback.
  globalThis.BP_PROC_CLASS = class BpProc extends globalThis.BP_H.Stream {
    constructor(child, streams) {
      super("proc");
      this.child = child;
      this.sink = child.stdin;
      this.wshut = false;
      this.full = false;
      this.streams = streams.filter((x) => x !== null && x !== undefined);
      this.open = this.streams.length;
      if (this.open === 0) {
        this.finish(0);
      }
      for (const r of this.streams) {
        r.on("data", (d) => this.feed(new Uint8Array(d.buffer, d.byteOffset, d.byteLength)));
        r.once("end", () => this.ended());
        r.once("error", () => this.ended());
      }
      if (this.sink) {
        this.sink.on("error", () => this.broken());
      }
    }
    ended() {
      this.open -= 1;
      if (this.open === 0) {
        this.finish(0);
      }
    }
    broken() {
      this.wshut = true;
      this.full = false;
      this.wake(true);
    }
    pause() {
      for (const r of this.streams) {
        r.pause();
      }
    }
    resume() {
      for (const r of this.streams) {
        r.resume();
      }
    }
    writable() {
      return !this.full || this.wshut;
    }
    // at most 64 KiB at a time, none while the stream holds more than its
    // high-water mark (it says so by write answering false; drain clears it):
    // a send to a child that does not read waits, as a full socketpair makes
    // it wait in C; a write that fails (EPIPE) fails the send
    write(b) {
      if (this.wshut || !this.sink) {
        return -globalThis.BP_H.ERRNO.EPIPE;
      }
      if (this.full) {
        return -globalThis.BP_H.ERRNO.EAGAIN;
      }
      const c = Buffer.from(b.subarray(0, 65536));
      let ok;
      try {
        ok = this.sink.write(c, (e) => {
          if (e) {
            this.broken();
          }
        });
      } catch (_) {
        this.broken();
        return -globalThis.BP_H.ERRNO.EPIPE;
      }
      if (!ok) {
        this.full = true;
        this.sink.once("drain", () => {
          this.full = false;
          this.wake(true);
        });
      }
      return c.length;
    }
    shutdown(how) {
      if (how !== 1) {
        this.shutRead();
      }
      if (how !== 0 && !this.wshut) {
        this.wshut = true;
        try {
          this.sink?.end();
        } catch (_) {
        }
      }
      this.wake(true);
    }
    close() {
      this.shutdown(2);
      for (const r of this.streams) {
        try {
          r.destroy();
        } catch (_) {
        }
      }
      this.wakeAll();
    }
    abort() {
      this.close();
    }
  };
  return globalThis.BP_PROC_CLASS;
}

// Proc.spawn on Bun: cmd (on PATH) with args in cwd ("" keeps ours); err as
// host.c: 0 discards stderr, 1 joins it to stdout, 2 inherits ours, 3
// writes it to the file Proc.errtail reads. The environment is ours
// (Env.put writes process.env).
// On Windows a command named by an absolute POSIX path that is not there
// ("/bin/bash", the hub's default SHELL; "/bin/sh") is the program of that
// name on PATH outside the system's folders: the Windows package ships its
// runtime's bin (bash.exe, sh.exe) there. Anything else is left as it is.
function bp_win_cmd(cmd) {
  if (process.platform !== "win32" || !cmd.startsWith("/") || require("fs").existsSync(cmd)) {
    return cmd;
  }
  const base = cmd.slice(cmd.lastIndexOf("/") + 1);
  if (base === "") {
    return cmd;
  }
  // PATH in order, skipping the system's own folders: C:\Windows\System32
  // has bash.exe, WSL's launcher, which exits at once (66) without a
  // distribution; the package's runtime bin has the real one
  const sys = String(process.env.SystemRoot || process.env.windir || "C:\\Windows").toLowerCase();
  const fs = require("fs");
  for (const dir of String(process.env.PATH ?? "").split(";")) {
    if (dir === "" || dir.toLowerCase().startsWith(sys) || /\\windowsapps\\?$/i.test(dir)) {
      continue;
    }
    for (const name of [base + ".exe", base]) {
      const full = dir.replace(/[\\/]$/, "") + "\\" + name;
      try {
        if (fs.statSync(full).isFile()) {
          return full;
        }
      } catch (_) {
      }
    }
  }
  return cmd;
}

function bp_proc_spawn(cmd, args, cwd, err) {
  cmd = bp_win_cmd(cmd);
  const fs = require("fs");
  const argv = host_self([cmd, ...args]);
  if (argv.some((a) => a.includes("\0")) || cwd.includes("\0")) {
    return io_fail(host_eilseq());
  }
  if (cwd !== "") {
    try {
      if (!fs.statSync(cwd).isDirectory()) {
        return io_fail(globalThis.BP_H.ERRNO.ENOTDIR);
      }
    } catch (e) {
      return io_fail(bp_proc_errno(e));
    }
  }
  const env = {};
  for (const [k, v] of Object.entries(process.env)) {
    if (v !== undefined && k !== "" && !k.includes("=")) {
      env[k] = v;
    }
  }
  // node reports a missing program later, as an event: found here first, so
  // Proc.spawn fails at once with ENOENT as posix_spawnp does
  const named = /[\\/]/.test(argv[0]);
  const found = named ? (fs.existsSync(argv[0]) ? argv[0] : null)
    : Bun.which(argv[0], { PATH: env.PATH ?? env.Path ?? "", cwd: cwd === "" ? undefined : cwd });
  if (found === null) {
    return io_fail(globalThis.BP_H.ERRNO.ENOENT);
  }
  let tmp = null;
  let errfd = null;
  let stderr = "ignore";
  if (err === 1) {
    stderr = "pipe";
  } else if (err === 2) {
    stderr = "inherit";
  } else if (err === 3) {
    const g = globalThis.BACKPLANE_ERR ?? (globalThis.BACKPLANE_ERR = { seq: 0 });
    const dir = host_err_dir();
    if (g.seq === 0) {
      host_err_sweep(dir);
    }
    g.seq += 1;
    tmp = dir + "/spawn." + process.pid + "." + g.seq;
    try {
      errfd = fs.openSync(tmp, "w", 0o600);
      stderr = errfd;
    } catch (_) {
      tmp = null;
    }
  }
  let child;
  try {
    child = require("node:child_process").spawn(found, argv.slice(1), {
      cwd: cwd === "" ? undefined : cwd, env, argv0: argv[0],
      stdio: ["pipe", "pipe", stderr], windowsHide: true });
  } catch (e) {
    child = null;
    if (tmp !== null) {
      try {
        fs.unlinkSync(tmp);
      } catch (_) {
      }
    }
    if (errfd !== null) {
      fs.closeSync(errfd);
    }
    return io_fail(bp_proc_errno(e));
  }
  if (errfd !== null) {
    fs.closeSync(errfd);
  }
  if (child.pid === undefined) {
    if (tmp !== null) {
      try {
        fs.unlinkSync(tmp);
      } catch (_) {
      }
    }
    child.once("error", () => {});
    return io_fail(globalThis.BP_H.ERRNO.ENOENT);
  }
  child.once("error", () => {});
  child.exited = new Promise((res) => child.once("exit", () => res(child.exitCode)));
  if (tmp !== null) {
    try {
      fs.renameSync(tmp, host_err_path(child.pid));
    } catch (_) {
    }
  }
  const h = new (bp_proc_class())(child, err === 1 ? [child.stdout, child.stderr] : [child.stdout]);
  bp_procs().set(child.pid >>> 0, child);
  return io_done(io_tup(child.pid >>> 0, globalThis.BP_H.add(h)));
}

// the code Proc.wait answers for a child that ended: its exit code, 128 +
// the signal that ended it
function bp_proc_code(child) {
  if (child.signalCode !== null && child.signalCode !== undefined) {
    return 128 + (require("os").constants.signals[child.signalCode] ?? 0);
  }
  return (child.exitCode ?? 255) & 0xff;
}

// Proc.wait: parks until the child ends; 255 when it is not ours (or was
// waited for already), as waitpid's ECHILD
function bp_proc_wait(pid, k) {
  const child = bp_procs().get(pid >>> 0);
  if (child === undefined) {
    return 255;
  }
  return io_park_promise(child.exited, k, () => {
    bp_procs().delete(pid >>> 0);
    return bp_proc_code(child);
  });
}

// Proc.exited: the code of a child that ended (Proc.wait still answers it),
// 4294967295 while it runs, 255 when it is not ours
function bp_proc_exited(pid) {
  const child = bp_procs().get(pid >>> 0);
  if (child === undefined) {
    return 255;
  }
  if (child.exitCode === null && (child.signalCode === null || child.signalCode === undefined)) {
    return 4294967295;
  }
  return bp_proc_code(child) >>> 0;
}

// Proc.kill: pid as kill(2) reads it, a negated pid a process group (POSIX;
// on Windows a group's leader is ended alone). Never 0, 1 or -1.
function bp_proc_kill(pid, sig) {
  const n = pid | 0;
  if (n === 0 || n === 1 || n === -1) {
    return;
  }
  const child = bp_procs().get((n < 0 ? -n : n) >>> 0);
  try {
    if (n < 0 && process.platform !== "win32") {
      process.kill(n, sig | 0);
    } else if (child !== undefined) {
      child.kill(sig | 0);
    } else {
      process.kill(n < 0 ? -n : n, sig | 0);
    }
  } catch (_) {
  }
}

// Where an agent's stderr goes (err 3): ${BACKPLANE_ERR_DIR:-${TMPDIR:-/tmp}/backplane-err-<uid>}/<pid>.err
function host_err_dir() {
  const d = process.env.BACKPLANE_ERR_DIR;
  const t = process.env.TMPDIR || require("os").tmpdir();
  const uid = typeof process.getuid === "function" ? process.getuid() : "user";
  const dir = d ? d : t + "/backplane-err-" + uid;
  try {
    require("fs").mkdirSync(dir, { mode: 0o700 });
  } catch (_) {
  }
  return dir;
}

function host_err_path(pid) {
  return host_err_dir() + "/" + (pid >>> 0) + ".err";
}

// files of processes that ended unread (their hub was killed) go, once per
// hub: those whose pid no longer runs
function host_err_sweep(dir) {
  const fs = require("fs");
  let names = [];
  try {
    names = fs.readdirSync(dir);
  } catch (_) {
    return;
  }
  for (const name of names) {
    const m = /^([0-9]+)\.err$/.exec(name);
    if (m === null || Number(m[1]) <= 1) {
      continue;
    }
    let gone = false;
    try {
      process.kill(Number(m[1]), 0);
    } catch (e) {
      gone = e?.code === "ESRCH";
    }
    if (gone) {
      try {
        fs.unlinkSync(dir + "/" + name);
      } catch (_) {
      }
    }
  }
}

// a fresh file for a child's stderr: [fd, its temporary path], or null
function host_err_open() {
  const g = globalThis.BACKPLANE_ERR ?? (globalThis.BACKPLANE_ERR = { seq: 0 });
  const dir = host_err_dir();
  if (g.seq === 0) {
    host_err_sweep(dir);
  }
  g.seq += 1;
  const tmp = dir + "/spawn." + process.pid + "." + g.seq;
  try {
    return [require("fs").openSync(tmp, "w", 0o600), tmp];
  } catch (_) {
    return null;
  }
}

// The descriptors open in this process above 2 (Bun's own included). The
// listing names the descriptor that read it, closed by now: only those
// still open are kept (on macOS a close action on a closed one fails the
// whole posix_spawn with EBADF).
function host_fds() {
  const fs = require("fs");
  try {
    return fs.readdirSync(process.platform === "darwin" ? "/dev/fd" : "/proc/self/fd")
      .map(Number).filter((fd) => fd > 2 && io_sys().fcntl(fd, 1, 0) >= 0);
  } catch (_) {
    return [];
  }
}

// Running this program again. Under `bun x.js` the script is no
// executable of its own: a command that names it (the path Sys.exe_path
// answers, as Exe.child gives it, maybe after env's NAME=value words)
// runs through the bun that runs us, with bun's own flags. A single-file binary
// (`bun build --compile`) runs as itself.
function host_compiled() {
  const m = process.argv[1] ?? "";
  return m.startsWith("/$bunfs/") || m.startsWith("B:/~BUN/") || m.startsWith("B:\\~BUN\\");
}

function host_script() {
  return require("path").resolve(process.argv[1] ?? "");
}

function host_bun() {
  if (process.platform !== "darwin") {
    try {
      const raw = require("fs").readlinkSync("/proc/self/exe");
      return raw.endsWith(" (deleted)") ? "/proc/self/exe" : raw;
    } catch (_) {
    }
  }
  return process.execPath;
}

function host_self(argv) {
  if (host_compiled()) {
    return argv;
  }
  const me = host_script();
  let i = 0;
  if (argv[0] === "env" || argv[0] === "/usr/bin/env" || argv[0] === "/bin/env") {
    i = 1;
    while (i < argv.length && argv[i].includes("=") && !argv[i].startsWith("-")) {
      i += 1;
    }
  }
  if (i < argv.length && argv[i] === me) {
    return [...argv.slice(0, i), host_bun(), ...(process.execArgv ?? []), ...argv.slice(i)];
  }
  return argv;
}

// Starts cmd (searched on PATH) in cwd. Its stdin and stdout are one end of
// a socketpair; we keep the other, non-blocking. stderr: 0 discards it, 1
// joins it to stdout, 2 inherits ours, 3 goes to a file read back by
// Proc.errtail. Answers (pid, socket). As host.c, plus: the child gets
// only 0-2 (Bun opens descriptors without close-on-exec), and starts with
// every signal at its default and none blocked.
function proc_spawn(cmd, args, cwd, err) {
  if (bp_proc_bun()) {
    return bp_proc_spawn(cmd, host_list_js(args), cwd, Number(err));
  }
  // Windows without the Bun loop's handles (loop.js not in the program):
  // nothing to spawn with, never a libc call
  if (process.platform === "win32") {
    return io_fail(38);
  }
  const c = host_libc();
  const sys = io_sys();
  const argv = host_self([cmd, ...host_list_js(args)]);
  const v = host_vec(argv);
  const env = host_vec(host_envp());
  const dir = host_cstr(cwd);
  if (v === null || dir === null || env === null) {
    return io_fail(host_eilseq());
  }
  const sv = new Int32Array(2);
  if (c.socketpair(1, c.mac ? 1 : 1 | 0x80000, 0, c.ptr(sv)) < 0) {
    return io_fail(sys.errno());
  }
  const fa = new Uint8Array(1024);
  const at = new Uint8Array(1024);
  const set = new Uint8Array(256);
  const pid = new Int32Array(1);
  const devnull = host_cstr("/dev/null");
  let code = 0;
  let errf = null;
  c.posix_spawn_file_actions_init(c.ptr(fa));
  c.posix_spawnattr_init(c.ptr(at));
  if (cwd.length > 0) {
    code = c.addchdir === undefined ? 38 : c.addchdir(c.ptr(fa), c.ptr(dir));
  }
  c.posix_spawn_file_actions_adddup2(c.ptr(fa), sv[1], 0);
  c.posix_spawn_file_actions_adddup2(c.ptr(fa), sv[1], 1);
  if (err === 0) {
    c.posix_spawn_file_actions_addopen(c.ptr(fa), 2, c.ptr(devnull), 1, 0);
  } else if (err === 1) {
    c.posix_spawn_file_actions_adddup2(c.ptr(fa), sv[1], 2);
  } else if (err === 3) {
    // stderr to a file of its own, named for the pid once it is known
    // (Proc.errtail reads its end and removes it)
    errf = host_err_open();
    if (errf !== null) {
      c.posix_spawn_file_actions_adddup2(c.ptr(fa), errf[0], 2);
    } else {
      c.posix_spawn_file_actions_addopen(c.ptr(fa), 2, c.ptr(devnull), 1, 0);
    }
  }
  // the child keeps only the copies on 0-2: left open, a process it puts
  // in the background would hold the pipe, and the caller never sees EOF
  // (macOS: POSIX_SPAWN_CLOEXEC_DEFAULT below; a close action there on a
  // descriptor the child does not inherit, Bun's kqueue, fails the spawn)
  if (c.mac) {
  } else if (c.addclosefrom !== undefined) {
    c.addclosefrom(c.ptr(fa), 3);
  } else {
    for (const fd of host_fds()) {
      c.posix_spawn_file_actions_addclose(c.ptr(fa), fd);
    }
  }
  // every signal at its default (a hub started from a background job
  // ignores SIGINT and SIGQUIT, and Bun ignores SIGPIPE and SIGXFSZ). This
  // differs from host.c, which resets only SIGPIPE: agents run under
  // systemd and must answer the signals the hub and their own tools send
  c.sigfillset(c.ptr(set));
  c.posix_spawnattr_setsigdefault(c.ptr(at), c.ptr(set));
  c.sigemptyset(c.ptr(set));
  c.posix_spawnattr_setsigmask(c.ptr(at), c.ptr(set));
  c.posix_spawnattr_setflags(c.ptr(at), 0x04 | 0x08 | (c.mac ? 0x4000 : 0));
  if (code === 0) {
    code = host_child_stack(() => c.posix_spawnp(c.ptr(pid), c.ptr(v.bufs[0]), c.ptr(fa), c.ptr(at),
      c.ptr(v.vec), c.ptr(env.vec)));
  }
  c.posix_spawn_file_actions_destroy(c.ptr(fa));
  c.posix_spawnattr_destroy(c.ptr(at));
  c.close(sv[1]);
  if (errf !== null) {
    const fs = require("fs");
    fs.closeSync(errf[0]);
    try {
      if (code === 0) {
        fs.renameSync(errf[1], host_err_path(pid[0]));
      } else {
        fs.unlinkSync(errf[1]);
      }
    } catch (_) {
    }
  }
  if (code !== 0) {
    c.close(sv[0]);
    return io_fail(code);
  }
  sys.fcntl(sv[0], 4, sys.fcntl(sv[0], 3, 0) | (c.mac ? 4 : 0x800));
  sys.fcntl(sv[0], 2, 1);
  return io_done(io_tup(pid[0] >>> 0, bp_loop_bun() ? globalThis.BP_H.fromFd(sv[0], "proc") : sv[0]));
}

// The last 2048 bytes a process started with err 3 wrote to stderr ("" when
// none); its file is removed.
function proc_errtail(pid) {
  const fs = require("fs");
  const path = host_err_path(pid);
  let fd;
  try {
    fd = fs.openSync(path, "r");
  } catch (_) {
    return "";
  }
  let text = "";
  try {
    const end = fs.fstatSync(fd).size;
    const n = Math.min(end, 2048);
    const b = Buffer.alloc(n);
    const got = n > 0 ? fs.readSync(fd, b, 0, n, end - n) : 0;
    text = b.subarray(0, got).toString("utf8");
  } catch (_) {
  }
  fs.closeSync(fd);
  try {
    fs.unlinkSync(path);
  } catch (_) {
  }
  return text;
}

// a wait status as Proc.wait answers it: the exit code, 128 + the signal
function host_status(st) {
  return (st & 0x7f) === 0 ? st >> 8 & 0xff : 128 + (st & 0x7f);
}

// waitpid(pid, WNOHANG): the code once it ended (255 when it is no child
// of ours), undefined while it runs
function host_reap(pid) {
  const c = host_libc();
  const st = new Int32Array(1);
  for (;;) {
    const r = c.waitpid(pid | 0, c.ptr(st), 1);
    if (r === (pid | 0)) {
      return host_status(st[0]);
    }
    if (r === 0) {
      return undefined;
    }
    if (io_sys().errno() !== 4) {
      return 255;
    }
  }
}

// a descriptor readable once pid has ended (Linux 5.3+), or -1
function host_pidfd(pid) {
  const c = host_libc();
  if (c.mac) {
    return -1;
  }
  let fd = c.pidfd_open === undefined ? -1 : c.pidfd_open(pid | 0, 0);
  if (fd < 0 && c.pidfd_open === undefined) {
    const sc = globalThis.BACKPLANE_SYSCALL ?? (globalThis.BACKPLANE_SYSCALL = (() => {
      try {
        return require("bun:ffi").dlopen("libc.so.6", { syscall: { args: ["i64", "i64", "i64"], returns: "i64" } }).symbols.syscall;
      } catch (_) {
        return null;
      }
    })());
    const nr = process.arch === "x64" || process.arch === "arm64" ? 434 : -1;
    fd = sc && nr > 0 ? Number(sc(BigInt(nr), BigInt(pid | 0), 0n)) : -1;
  }
  if (fd >= 0) {
    io_sys().fcntl(fd, 2, 1);
  }
  return fd;
}

// Waits for pid without blocking the loop: parked on its pidfd (or, where
// there is none, on a 20 ms clock), then reaped. calls k with the code.
function host_wait_then(pid, k, done) {
  const now = host_reap(pid);
  if (now !== undefined) {
    return done(now);
  }
  const fd = host_pidfd(pid);
  const more = () => {
    const r = host_reap(pid);
    if (r !== undefined) {
      if (fd >= 0) {
        host_libc().close(fd);
      }
      return done(r);
    }
    if (fd >= 0) {
      io_park_on(fd, false, k, more);
    } else {
      io_park_on(undefined, false, k, more, io_until(20));
    }
    return undefined;
  };
  if (fd >= 0) {
    io_park_on(fd, false, k, more);
  } else {
    io_park_on(undefined, false, k, more, io_until(20));
  }
  return undefined;
}

// The exit code (128 + signal when killed), once the child ends.
function proc_wait(pid, k) {
  if (bp_proc_bun() && bp_procs().has(pid >>> 0)) {
    return bp_proc_wait(pid, k);
  }
  // Windows has no libc: a pid not ours (or already waited for) reads 255,
  // as waitpid's ECHILD does in C
  if (process.platform === "win32") {
    return 255;
  }
  if (bp_loop_bun()) {
    return bp_chld_wait(pid >>> 0, k);
  }
  return host_wait_then(pid, k, (code) => code);
}

// Proc.exited: the exit code of a child that has exited (left unreaped:
// WNOWAIT), 4294967295 while it runs; one already reaped reads 255.
function proc_exited(pid) {
  if (bp_proc_bun() && bp_procs().has(pid >>> 0)) {
    return bp_proc_exited(pid);
  }
  if (process.platform === "win32") {
    return 255;
  }
  const c = host_libc();
  const si = new Uint8Array(256);
  const dv = new DataView(si.buffer);
  const r = c.waitid(1, pid >>> 0, c.ptr(si), c.mac ? 0x04 | 0x01 | 0x20 : 4 | 1 | 0x1000000);
  if (r < 0) {
    return 255;
  }
  const spid = dv.getInt32(c.mac ? 12 : 16, true);
  if (spid === 0) {
    return 4294967295;
  }
  const scode = dv.getInt32(8, true);
  const status = dv.getInt32(c.mac ? 20 : 24, true);
  return (scode === 1 ? status : 128 + status) >>> 0;
}

function proc_kill(pid, sig) {
  if (process.platform === "win32") {
    bp_proc_kill(pid, sig);
    return { $: "Unit" };
  }
  host_libc().kill(pid | 0, sig | 0);
  return { $: "Unit" };
}

function sock_shut_read(socket) {
  host_libc().shutdown(socket, 0);
  return socket;
}

// a raw descriptor on the same socket (a dup, close-on-exec); 0 when none
function sock_raw(socket) {
  const got = io_sys().fcntl(socket, io_sys().mac ? 67 : 1030, 0);
  return io_tup(socket, got < 3 ? 0 : got);
}

function fd_shut(fd) {
  if (Number(fd) > 2) {
    host_libc().shutdown(Number(fd), 2);
    host_libc().close(Number(fd));
  }
  return { $: "Unit" };
}

function fd_close(fd) {
  if (Number(fd) > 2) {
    host_libc().close(Number(fd));
  }
  return { $: "Unit" };
}

function fs_stat(path) {
  try {
    const st = require("fs").statSync(path);
    const kind = st.isFile() ? 0 : st.isDirectory() ? 1 : 2;
    return io_done(io_tup(kind, io_tup(Math.min(st.size, 0xffffffff) >>> 0,
      Math.floor(st.mtimeMs / 1000) >>> 0)));
  } catch (e) {
    return io_fail(host_errno(e));
  }
}

function fs_revision(path) {
  try {
    const st = require("fs").statSync(path, { bigint: true });
    const rev = [st.dev, st.ino, st.size, st.mtimeNs / 1000000000n,
      st.mtimeNs % 1000000000n, st.ctimeNs / 1000000000n,
      st.ctimeNs % 1000000000n].join(":");
    return io_done(io_tup(Number(st.size > 0xffffffffn ? 0xffffffffn : st.size), rev));
  } catch (e) {
    return io_fail(host_errno(e));
  }
}

function fs_list(path) {
  try {
    return io_done(host_list(require("fs").readdirSync(path)));
  } catch (e) {
    return io_fail(host_errno(e));
  }
}

function fs_mkdirs(path) {
  try {
    require("fs").mkdirSync(path, { recursive: true });
    return io_done({ $: "Unit" });
  } catch (e) {
    return io_fail(host_errno(e));
  }
}

function fs_rename(from, to) {
  try {
    require("fs").renameSync(from, to);
    return io_done({ $: "Unit" });
  } catch (e) {
    return io_fail(host_errno(e));
  }
}

function fs_remove(path) {
  try {
    require("fs").rmSync(path);
    return io_done({ $: "Unit" });
  } catch (e) {
    try {
      require("fs").rmdirSync(path);
      return io_done({ $: "Unit" });
    } catch (e2) {
      return io_fail(host_errno(e2));
    }
  }
}

function fs_chmod(path, mode) {
  try {
    require("fs").chmodSync(path, mode);
    return io_done({ $: "Unit" });
  } catch (e) {
    return io_fail(host_errno(e));
  }
}

// as net_listen_run in host.c: SO_REUSEADDR, a backlog of 64, nonblocking,
// close-on-exec (Base's tcp_listen.js through the same ffi)
function net_listen(host, port) {
  const at = io_addr(host, Number(port));
  if (at === null) {
    return io_fail(22);
  }
  const sys = io_sys();
  const fd = sys.socket(2, 1, 0);
  if (fd < 0) {
    return io_fail(sys.errno());
  }
  const one = new Int32Array([1]);
  sys.setsockopt(fd, sys.mac ? 0xffff : 1, sys.mac ? 4 : 2, sys.ptr(one), 4);
  if (sys.bind(fd, sys.ptr(at), 16) < 0 || sys.listen(fd, 64) < 0
    || sys.fcntl(fd, 4, sys.fcntl(fd, 3, 0) | (sys.mac ? 4 : 0x800)) < 0) {
    const code = sys.errno();
    sys.close(fd);
    return io_fail(code);
  }
  sys.fcntl(fd, 2, 1);
  return io_done(fd);
}

// setenv(name, value, 1): processes started afterwards inherit it (they
// get process.env; libc's copy is kept the same for PATH lookups)
function env_put(name, value) {
  if (typeof process !== "undefined" && process.env && name.length > 0
    && !name.includes("\0") && !value.includes("\0")) {
    process.env[name] = value;
    // Windows has no libc copy to keep (Bun's own environment is the one)
    if (process.platform !== "win32") try {
      host_libc().setenv(host_libc().ptr(host_cstr(name)), host_libc().ptr(host_cstr(value)), 1);
    } catch (_) {
    }
  }
  return { $: "Unit" };
}

// The running program's full path: a single-file binary's own path read
// from /proc/self/exe, as host.c (so a replaced binary reads back
// "<path> (deleted)"); under `bun x.js` the script's absolute path.
function host_exe() {
  if (!host_compiled()) {
    return process.argv[1] === undefined ? "" : host_script();
  }
  if (process.platform !== "darwin") {
    try {
      return require("fs").readlinkSync("/proc/self/exe");
    } catch (_) {
    }
  }
  return process.execPath ?? "";
}

// The directory holding the running binary ("" when unknown).
function sys_exe_dir() {
  const raw = host_exe();
  const at = raw.lastIndexOf("/");
  return at < 0 ? "" : raw.slice(0, at);
}

function sys_exe_path() {
  return host_exe();
}

// Replace this process with path (argv[0] = path, then args), as host.c:
// every descriptor is close-on-exec but the agents' (adopt.js) and the
// hub's lock (hublock.js), which those files' hooks keep open through it.
// Answers only on failure. Under `bun x.js` the script runs through bun
// (host_self).
function sys_exec(path, args) {
  const c = host_libc();
  const argv = host_self([path, ...host_list_js(args)]);
  if (argv.some((a) => a.includes("\0"))) {
    return io_fail(host_eilseq());
  }
  for (const fd of host_fds()) {
    io_sys().fcntl(fd, 2, 1);
  }
  const hooks = [globalThis.BACKPLANE_ADOPT_EXEC, globalThis.BACKPLANE_HUBLOCK_EXEC]
    .filter((f) => typeof f === "function");
  for (const f of hooks) {
    f(1);
  }
  const v = host_vec(argv);
  const env = host_vec(host_envp());
  let code = host_eilseq();
  if (v !== null && env !== null) {
    c.execve(c.ptr(v.bufs[0]), c.ptr(v.vec), c.ptr(env.vec));
    code = io_sys().errno();
  }
  for (const f of hooks.reverse()) {
    f(0);
  }
  return io_fail(code);
}

// Json.cbor_frame and Sock.send_cbor: JSON text to one binary WebSocket
// frame of its CBOR, as core/cbor.bend's Cbor.encode over J.Json.parse (see
// host.c): plain 32-bit integers as major 0/1, other numbers as tag 7 over
// their text, dictionary words as tag 6, dictionary keys as their index.
function jc_frame(text, keys, words) {
  const ks = keys.split("\n"), ws = words.split("\n");
  const enc = new TextEncoder();
  const out = [];
  const head = (mb, v) => {
    if (v < 24) out.push(mb | v);
    else if (v < 256) out.push(mb | 24, v);
    else if (v < 65536) out.push(mb | 25, v >> 8, v & 255);
    else out.push(mb | 26, (v >>> 24) & 255, (v >>> 16) & 255, (v >>> 8) & 255, v & 255);
  };
  const txt = (s) => { const b = enc.encode(s); head(0x60, b.length); for (const x of b) out.push(x); };
  const pos = (r) => /^(0|[1-9][0-9]{0,9})$/.test(r) && Number(r) <= 0xffffffff ? Number(r) : -1;
  let i = 0;
  const wsp = () => { while (i < text.length && " \t\n\r".includes(text[i])) i += 1; };
  const str = () => {
    let s = "";
    while (i < text.length) {
      const c = text[i++];
      if (c === '"') return s;
      if (c !== "\\") { s += c; continue; }
      const e = text[i++];
      if (e === "u") { s += String.fromCharCode(parseInt(text.slice(i, i + 4), 16)); i += 4; }
      else s += ({ '"': '"', "\\": "\\", "/": "/", b: "\b", f: "\f", n: "\n", r: "\r", t: "\t" })[e] ?? (() => { throw 0; })();
    }
    throw 0;
  };
  const val = () => {
    wsp();
    const c = text[i];
    if (c === "{" || c === "[") {
      i += 1;
      const map = c === "{";
      const at = out.length;
      const items = [];
      wsp();
      if (text[i] === (map ? "}" : "]")) { i += 1; }
      else for (;;) {
        const mark = out.length;
        if (map) {
          wsp();
          if (text[i] !== '"') throw 0;
          i += 1;
          const k = str();
          const n = ks.indexOf(k);
          if (n >= 0) head(0, n); else txt(k);
          wsp();
          if (text[i++] !== ":") throw 0;
        }
        val();
        items.push(out.splice(mark));
        wsp();
        const d = text[i++];
        if (d === ",") continue;
        if (d === (map ? "}" : "]")) break;
        throw 0;
      }
      head(map ? 0xa0 : 0x80, items.length);
      for (const it of items) for (const x of it) out.push(x);
      return at;
    }
    if (c === '"') { i += 1; const s = str(); const n = ws.indexOf(s); if (n >= 0) { out.push(0xc6); head(0, n); } else txt(s); return; }
    if (text.startsWith("true", i)) { i += 4; out.push(0xf5); return; }
    if (text.startsWith("false", i)) { i += 5; out.push(0xf4); return; }
    if (text.startsWith("null", i)) { i += 4; out.push(0xf6); return; }
    const m = /^[-+0-9.eE]+/.exec(text.slice(i, i + 64));
    if (!m) throw 0;
    const r = m[0];
    i += r.length;
    if (r[0] === "-") { const v = pos(r.slice(1)); if (v > 0) { head(0x20, v - 1); return; } }
    else { const v = pos(r); if (v >= 0) { head(0, v); return; } }
    out.push(0xc7);
    txt(r);
  };
  try { val(); wsp(); if (i !== text.length) throw 0; } catch { out.length = 0; out.push(0xf6); }
  const n = out.length;
  const h = n < 126 ? [0x82, n] : n < 65536 ? [0x82, 126, n >> 8, n & 255] : [0x82, 127, 0, 0, 0, 0, (n >>> 24) & 255, (n >>> 16) & 255, (n >>> 8) & 255, n & 255];
  return h.concat(out);
}

function json_cbor_frame(text, keys, words) {
  let xs = { $: "Nil" };
  const b = jc_frame(text, keys, words);
  for (let k = b.length - 1; k >= 0; k -= 1) xs = { $: "Con", head: b[k], tail: xs };
  return xs;
}

// Json.items_frames and Sock.send_items: a join frame's lines as
// outbox.bend's Items.texts.s splits them, each frame's CBOR (see host.c)
function ji_frames(hd, plain, split, lines, tl, ends, most, bytes, keys, words) {
  const ls = [];
  for (let x = lines; x.$ === "Con"; x = x.tail) ls.push(x.head);
  const cps = ls.map((l) => [...l].length);
  const count = (i) => {
    if (i >= ls.length) return 0;
    let r = Math.max(0, bytes - cps[i]), c = 1;
    for (let j = i + 1, left = Math.max(0, most - 1); j < ls.length && left > 0; j += 1, left -= 1) {
      if (cps[j] > r) break;
      r -= cps[j]; c += 1;
    }
    return c;
  };
  let c = count(0);
  const out = jc_frame(hd + (c >= ls.length ? plain : split) + ls.slice(0, c).join(",") + tl, keys, words);
  for (let i = c; i < ls.length; i += c) {
    c = count(i);
    const last = ends !== 0 && i + c >= ls.length;
    for (const b of jc_frame(`{"t":"changes","boot":${last ? "false" : "true"},"items":[` + ls.slice(i, i + c).join(",") + "]}", keys, words)) out.push(b);
  }
  return out;
}

function json_items_frames(hd, plain, split, lines, tl, ends, most, bytes, keys, words) {
  let xs = { $: "Nil" };
  const b = ji_frames(hd, plain, split, lines, tl, ends, most, bytes, keys, words);
  for (let k = b.length - 1; k >= 0; k -= 1) xs = { $: "Con", head: b[k], tail: xs };
  return xs;
}

function sock_send_items(socket, hd, plain, split, lines, tl, ends, most, bytes, keys, words, ms, k) {
  let xs = { $: "Nil" };
  const b = ji_frames(hd, plain, split, lines, tl, ends, most, bytes, keys, words);
  for (let j = b.length - 1; j >= 0; j -= 1) xs = { $: "Con", head: b[j], tail: xs };
  return sock_send_until(socket, xs, ms, k);
}

function sock_send_cbor(socket, text, keys, words, ms, k) {
  let xs = { $: "Nil" };
  const b = jc_frame(text, keys, words);
  for (let j = b.length - 1; j >= 0; j -= 1) xs = { $: "Con", head: b[j], tail: xs };
  return sock_send_until(socket, xs, ms, k);
}

io_eff(CID(Clock.epoch), clock_epoch);
io_eff(CID(Sock.recv_bytes), sock_recv_bytes);
io_eff(CID(Sock.save), sock_save);
io_eff(CID(Sock.send_bytes), sock_send_bytes);
io_eff(CID(Sock.send_until), sock_send_until);
io_eff(CID(Sock.send_cbor), sock_send_cbor);
io_eff(CID(Json.cbor_frame), json_cbor_frame);
io_eff(CID(Sock.send_items), sock_send_items);
io_eff(CID(Json.items_frames), json_items_frames);
io_eff(CID(Sock.send_text), sock_send_text);
io_eff(CID(Sock.dup), sock_dup);
io_eff(CID(Sock.peer), sock_peer);
io_eff(CID(Sock.shutdown), sock_shutdown);
io_eff(CID(Proc.spawn), proc_spawn);
io_eff(CID(Proc.wait), proc_wait);
io_eff(CID(Proc.kill), proc_kill);
io_eff(CID(Proc.exited), proc_exited);
io_eff(CID(Proc.errtail), proc_errtail);
io_eff(CID(Sock.shut_read), sock_shut_read);
io_eff(CID(Sock.raw), sock_raw);
io_eff(CID(Fd.shut), fd_shut);
io_eff(CID(Fd.close), fd_close);
io_eff(CID(FS.stat), fs_stat);
io_eff(CID(FS.revision), fs_revision);
io_eff(CID(FS.list), fs_list);
io_eff(CID(FS.mkdirs), fs_mkdirs);
io_eff(CID(FS.rename), fs_rename);
io_eff(CID(FS.remove), fs_remove);
io_eff(CID(FS.chmod), fs_chmod);
io_eff(CID(Net.listen), net_listen);
io_eff(CID(Env.put), env_put);
io_eff(CID(Sys.exe_dir), sys_exe_dir);
io_eff(CID(Sys.exec), sys_exec);
io_eff(CID(Sys.exe_path), sys_exe_path);

io_eff(CID(Sys.native_pid), () => process.pid >>> 0);
io_eff(CID(Sys.native_path), (path) => path);
