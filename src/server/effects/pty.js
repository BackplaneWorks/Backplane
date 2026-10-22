// Backplane pty effects: the JS twin of pty.c.
//
// Bend's JS IO loop is one synchronous loop over select(), so the pty
// master must be a plain non-blocking fd it can park on, exactly as in C:
// it comes back as a Socket (a bare fd, non-blocking, close-on-exec), and
// host.js's Sock.recv_bytes / Sock.send_bytes read and write it (they fall
// back from recv/send to read/write on ENOTSOCK and read EIO as end of
// stream). Socket.close hangs it up.
//
// Bun cannot fork safely, so the child starts through posix_spawn (bun:ffi):
// POSIX_SPAWN_SETSID makes it a session leader, default signal dispositions
// and an empty mask, then the file actions open the slave on fd 0 (without
// O_NOCTTY), dup it to 1 and 2, close everything else and chdir. On Linux a
// session leader with no controlling terminal takes the first tty it opens
// as its own, so that is the whole job. macOS (BSD) never does that
// implicitly, so there the slave is opened here and dup'ed to 0-2, the
// child is /usr/bin/perl, which takes fd 0 with TIOCSCTTY and execs the
// program (stock macOS ships perl), and POSIX_SPAWN_CLOEXEC_DEFAULT closes
// the rest. posix_spawn reports exec and file-action failures (ENOENT, a
// missing cwd) as errno, like pty.c's report pipe.

function pty_lib() {
  if (globalThis.BACKPLANE_PTY === undefined) {
    const ffi = require("bun:ffi");
    const mac = process.platform === "darwin";
    const vari = mac && process.arch === "arm64";
    const name = mac ? "libSystem.dylib" : "libc.so.6";
    const want = {
      posix_openpt: { args: ["i32"], returns: "i32" },
      grantpt: { args: ["i32"], returns: "i32" },
      unlockpt: { args: ["i32"], returns: "i32" },
      // arm64 macOS passes variadic arguments on the stack: pad the
      // registers so the pointer lands there (as io_sys does for fcntl)
      ioctl: vari
        ? { args: ["i32", "u64", "i64", "i64", "i64", "i64", "i64", "i64", "ptr"], returns: "i32" }
        : { args: ["i32", "u64", "ptr"], returns: "i32" },
      posix_spawn: { args: ["ptr", "ptr", "ptr", "ptr", "ptr", "ptr"], returns: "i32" },
      posix_spawn_file_actions_init: { args: ["ptr"], returns: "i32" },
      posix_spawn_file_actions_destroy: { args: ["ptr"], returns: "i32" },
      posix_spawn_file_actions_addopen: { args: ["ptr", "i32", "ptr", "i32", "u32"], returns: "i32" },
      posix_spawn_file_actions_adddup2: { args: ["ptr", "i32", "i32"], returns: "i32" },
      posix_spawn_file_actions_addchdir_np: { args: ["ptr", "ptr"], returns: "i32" },
      posix_spawnattr_init: { args: ["ptr"], returns: "i32" },
      posix_spawnattr_destroy: { args: ["ptr"], returns: "i32" },
      posix_spawnattr_setflags: { args: ["ptr", "i16"], returns: "i32" },
      posix_spawnattr_setsigdefault: { args: ["ptr", "ptr"], returns: "i32" },
      posix_spawnattr_setsigmask: { args: ["ptr", "ptr"], returns: "i32" },
      sigfillset: { args: ["ptr"], returns: "i32" },
      sigemptyset: { args: ["ptr"], returns: "i32" },
    };
    if (mac) {
      want.ptsname = { args: ["i32"], returns: "cstring" };
      want.open = { args: ["ptr", "i32"], returns: "i32" };
    } else {
      want.ptsname_r = { args: ["i32", "ptr", "u64"], returns: "i32" };
    }
    const lib = ffi.dlopen(name, want).symbols;
    // glibc 2.34+; older ones close the inherited fds one by one
    let closefrom = null;
    let addclose = null;
    try {
      closefrom = ffi.dlopen(name, {
        posix_spawn_file_actions_addclosefrom_np: { args: ["ptr", "i32"], returns: "i32" },
      }).symbols.posix_spawn_file_actions_addclosefrom_np;
    } catch (_) {
      addclose = ffi.dlopen(name, {
        posix_spawn_file_actions_addclose: { args: ["ptr", "i32"], returns: "i32" },
      }).symbols.posix_spawn_file_actions_addclose;
    }
    const ioctl = (fd, req, p) => vari ? lib.ioctl(fd, req, 0, 0, 0, 0, 0, 0, p) : lib.ioctl(fd, req, p);
    globalThis.BACKPLANE_PTY = {
      ...lib, ioctl, closefrom, addclose, mac, ptr: ffi.ptr,
      // Linux / macOS constants
      O_RDWR: 2,
      O_NOCTTY: mac ? 0x20000 : 0x100,
      O_NONBLOCK: mac ? 0x4 : 0x800,
      TIOCSWINSZ: mac ? 0x80087467 : 0x5414,
      SPAWN_FLAGS: mac
        ? 0x04 | 0x08 | 0x400 | 0x4000 // SETSIGDEF SETSIGMASK SETSID CLOEXEC_DEFAULT
        : 0x04 | 0x08 | 0x80,          // SETSIGDEF SETSIGMASK SETSID
      EILSEQ: mac ? 92 : 84,
      ENOENT: 2,
      ENOTTY: 25,
    };
  }
  return globalThis.BACKPLANE_PTY;
}

function pty_cstr(s) {
  return Buffer.from(s + "\0", "utf8");
}

function pty_size(fd, cols, rows) {
  const p = pty_lib();
  const c = cols > 0 && cols < 65536 ? cols : 80;
  const r = rows > 0 && rows < 65536 ? rows : 24;
  const ws = new Uint16Array([r, c, 0, 0]);
  p.ioctl(fd, p.TIOCSWINSZ, p.ptr(ws));
}

// cmd itself when it has a slash, else the first executable regular file
// called cmd on PATH (null when none)
function pty_which(cmd) {
  const fs = require("fs");
  const ok = (f) => {
    try {
      fs.accessSync(f, fs.constants.X_OK);
      return fs.statSync(f).isFile();
    } catch (_) {
      return false;
    }
  };
  if (cmd.includes("/")) {
    return cmd;
  }
  const path = process.env.PATH || "/usr/local/bin:/usr/bin:/bin";
  for (const d of path.split(":")) {
    const full = (d === "" ? "." : d) + "/" + cmd;
    if (ok(full)) {
      return full;
    }
  }
  return null;
}

// our environment with TERM=xterm-256color and COLORTERM=truecolor, and
// without LINES / COLUMNS (the pty's size is the truth)
function pty_env() {
  const out = [];
  for (const [k, v] of Object.entries(process.env)) {
    if (k === "TERM" || k === "COLORTERM" || k === "LINES" || k === "COLUMNS" || v === undefined) {
      continue;
    }
    out.push(k + "=" + v);
  }
  out.push("TERM=xterm-256color", "COLORTERM=truecolor");
  return out;
}

// a NULL-ended char* array; keep holds the buffers alive through the call
function pty_strv(xs, keep) {
  const p = pty_lib();
  const v = new BigUint64Array(xs.length + 1);
  xs.forEach((s, i) => {
    const b = pty_cstr(s);
    keep.push(b);
    v[i] = BigInt(p.ptr(b));
  });
  keep.push(v);
  return v;
}

// the fds a spawned child would inherit (not close-on-exec), 3 and up;
// only for libcs without addclosefrom_np
function pty_open_fds() {
  const sys = io_sys();
  const out = [];
  let names = [];
  try {
    names = require("fs").readdirSync("/proc/self/fd");
  } catch (_) {
    for (let fd = 3; fd < 1024; fd += 1) names.push(String(fd));
  }
  for (const n of names) {
    const fd = Number(n);
    if (fd > 2) {
      const fl = sys.fcntl(fd, 1, 0); // F_GETFD
      if (fl >= 0 && (fl & 1) === 0) out.push(fd);
    }
  }
  return out;
}

// Answers (pid, master socket).
function pty_spawn(cmd, args, cwd, cols, rows) {
  const p = pty_lib();
  const sys = io_sys();
  const argv = [cmd];
  for (let xs = args; xs.$ === "Con"; xs = xs.tail) {
    argv.push(xs.head);
  }
  if (cmd.length === 0 || argv.some((s) => s.includes("\0")) || cwd.includes("\0")) {
    return io_fail(p.EILSEQ);
  }
  const prog = pty_which(cmd);
  if (prog === null) {
    return io_fail(p.ENOENT);
  }
  const m = p.posix_openpt(p.O_RDWR | p.O_NOCTTY);
  if (m < 0) {
    return io_fail(sys.errno());
  }
  const fail = (code) => {
    sys.close(m);
    return io_fail(code);
  };
  if (p.grantpt(m) < 0 || p.unlockpt(m) < 0) {
    return fail(sys.errno());
  }
  let slave;
  if (p.mac) {
    const sn = p.ptsname(m);
    if (sn === null || String(sn) === "") {
      return fail(sys.errno() || p.ENOTTY);
    }
    slave = String(sn);
  } else {
    const b = new Uint8Array(256);
    const r = p.ptsname_r(m, p.ptr(b), 256);
    if (r !== 0) {
      return fail(r || p.ENOTTY);
    }
    slave = new TextDecoder().decode(b.subarray(0, b.indexOf(0)));
  }
  sys.fcntl(m, 2, 1); // F_SETFD FD_CLOEXEC
  const sb = pty_cstr(slave);
  // macOS refuses TIOCSWINSZ on a master whose slave nobody has open
  // (ENOTTY), so there we open it first, set the size, and hand the child
  // this descriptor; Linux's child opens the slave itself to take it as
  // its controlling terminal
  let s = -1;
  if (p.mac) {
    s = p.open(p.ptr(sb), p.O_RDWR | p.O_NOCTTY | 0x1000000); // O_CLOEXEC
    if (s < 0) {
      return fail(sys.errno());
    }
  }
  pty_size(m, cols, rows);

  const keep = [];
  // perl on macOS: setsid already ran (POSIX_SPAWN_SETSID); take the tty
  // on fd 0 as ours, then exec the program under its own argv[0]
  const exe = p.mac ? "/usr/bin/perl" : prog;
  const run = p.mac
    ? ["perl", "-e", "ioctl(STDIN, 0x20007461, 0) or exit 126; exec {shift @ARGV} @ARGV; exit 127", "--", prog, ...argv]
    : argv;
  const av = pty_strv(run, keep);
  const ev = pty_strv(pty_env(), keep);
  const fa = new Uint8Array(1024);
  const at = new Uint8Array(1024);
  const none = new Uint8Array(256);
  const all = new Uint8Array(256);
  const eb = pty_cstr(exe);
  const cb = pty_cstr(cwd);
  p.posix_spawn_file_actions_init(p.ptr(fa));
  p.posix_spawnattr_init(p.ptr(at));
  let code = 0;
  const step = (r) => {
    if (code === 0 && r !== 0) code = r;
  };
  step(p.posix_spawnattr_setflags(p.ptr(at), p.SPAWN_FLAGS));
  p.sigemptyset(p.ptr(none));
  p.sigfillset(p.ptr(all));
  step(p.posix_spawnattr_setsigmask(p.ptr(at), p.ptr(none)));
  step(p.posix_spawnattr_setsigdefault(p.ptr(at), p.ptr(all)));
  if (p.mac) {
    step(p.posix_spawn_file_actions_adddup2(p.ptr(fa), s, 0));
  } else {
    step(p.posix_spawn_file_actions_addopen(p.ptr(fa), 0, p.ptr(sb), p.O_RDWR, 0));
  }
  step(p.posix_spawn_file_actions_adddup2(p.ptr(fa), 0, 1));
  step(p.posix_spawn_file_actions_adddup2(p.ptr(fa), 0, 2));
  // nothing else of ours leaks into the shell (listening sockets, files);
  // macOS has POSIX_SPAWN_CLOEXEC_DEFAULT in the flags instead
  if (!p.mac) {
    if (p.closefrom !== null) {
      step(p.closefrom(p.ptr(fa), 3));
    } else {
      for (const fd of pty_open_fds()) {
        step(p.addclose(p.ptr(fa), fd));
      }
    }
  }
  if (cwd !== "") {
    step(p.posix_spawn_file_actions_addchdir_np(p.ptr(fa), p.ptr(cb)));
  }
  const pid = new Int32Array(1);
  if (code === 0) {
    code = p.posix_spawn(p.ptr(pid), p.ptr(eb), p.ptr(fa), p.ptr(at), p.ptr(av), p.ptr(ev));
  }
  p.posix_spawn_file_actions_destroy(p.ptr(fa));
  p.posix_spawnattr_destroy(p.ptr(at));
  keep.length = 0;
  if (s >= 0) {
    sys.close(s);
  }
  if (code !== 0) {
    return fail(code);
  }
  sys.fcntl(m, 4, sys.fcntl(m, 3, 0) | p.O_NONBLOCK); // F_SETFL
  return io_done(io_tup(pid[0] >>> 0, m));
}

// a new window size; the kernel sends the child SIGWINCH
function pty_resize(socket, cols, rows) {
  pty_size(Number(socket), Number(cols), Number(rows));
  return socket;
}

io_eff(CID(Pty.spawn), pty_spawn);
io_eff(CID(Pty.resize), pty_resize);
