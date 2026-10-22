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

// as host.js's host_child_stack (each effect file has its own scope): the
// shell starts with a soft stack limit of 8 MiB at most, not the hub's 4G
function pty_child_stack(spawn) {
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
  if (lib.getrlimit(3, ptr(was)) !== 0 || was[0] <= want) {
    return spawn();
  }
  const lowered = lib.setrlimit(3, ptr(new BigUint64Array([want, was[1]]))) === 0;
  try {
    return spawn();
  } finally {
    if (lowered) {
      lib.setrlimit(3, ptr(was));
    }
  }
}

// Terminals on Bun's own APIs
// ---------------------------
// Where the runtime runs on Bun's event loop (bp_pty_bun), the
// shell starts with Bun.spawn's terminal option (openpty on POSIX, ConPTY on
// Windows): a session leader with the terminal as its controlling one, as
// pty.c makes it. The master is a BP_H handle (a BpStream): reading the
// terminal's output, writing its input, closing it hangs the terminal up.

// Made at first use: loop.js's BpStream may load after this file.
function bp_pty_class() {
  if (globalThis.BP_PTY_CLASS !== undefined) {
    return globalThis.BP_PTY_CLASS;
  }
  globalThis.BP_PTY_CLASS = class BpPty extends globalThis.BP_H.Stream {
    constructor() {
      super("pty");
      this.t = null;
    }
    // a terminal takes every byte (what it cannot pass on yet it holds and
    // sends on as the program reads: Bun 1.3's write answers only the part
    // that went at once, and its drain callback does not fire), so a write
    // never waits: a terminal's input is what the person types or pastes
    write(b) {
      if (this.t === null || this.t.closed) {
        return -globalThis.BP_H.ERRNO.EIO;
      }
      this.t.write(b);
      return b.length;
    }
    // a pty has no half close (a program is sent ^D), as pty.c
    shutdown(how) {
      if (how !== 1) {
        this.shutRead();
      }
    }
    close() {
      this.rshut = true;
      this.q = [];
      this.qsize = 0;
      try {
        this.t?.close();
      } catch (_) {
      }
      this.wakeAll();
    }
    resize(cols, rows) {
      try {
        this.t?.resize(cols, rows);
      } catch (_) {
      }
    }
  };
  return globalThis.BP_PTY_CLASS;
}

// as host.js's (each effect file has its own scope): Bun's backend where
// the runtime runs on Bun's loop, and the children it started by pid
function bp_pty_bun() {
  return (process.platform === "win32" || process.env.BACKPLANE_JS_PROC === "bun")
    && globalThis.BP_H?.Stream !== undefined;
}

function bp_pty_procs() {
  return globalThis.BP_PROCS ?? (globalThis.BP_PROCS = new Map());
}

function bp_pty_size(n, d) {
  return n > 0 && n < 65536 ? n : d;
}

// Windows: Bun's terminal option is POSIX-only (Bun 1.3: "terminal option
// is not supported on this platform"), so the shell gets a pseudoconsole
// through kernel32: two pipes, CreatePseudoConsole, CreateProcessW with
// PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE. Anonymous pipes cannot be waited on
// without blocking, so while the terminal lives a 15 ms timer peeks its
// output pipe and its process. Answers a child shaped as Bun.spawn's with a
// terminal (pid, terminal.write/resize/close, exited, exitCode, kill).
function bp_conpty_lib() {
  if (globalThis.BP_CONPTY !== undefined) {
    return globalThis.BP_CONPTY;
  }
  const ffi = require("bun:ffi");
  const k = ffi.dlopen("kernel32.dll", {
    CreatePipe: { args: ["ptr", "ptr", "ptr", "u32"], returns: "i32" },
    CreatePseudoConsole: { args: ["u32", "ptr", "ptr", "u32", "ptr"], returns: "i32" },
    ResizePseudoConsole: { args: ["ptr", "u32"], returns: "i32" },
    ClosePseudoConsole: { args: ["ptr"], returns: "void" },
    InitializeProcThreadAttributeList: { args: ["ptr", "u32", "u32", "ptr"], returns: "i32" },
    UpdateProcThreadAttribute: { args: ["ptr", "u32", "u64", "ptr", "u64", "ptr", "ptr"], returns: "i32" },
    DeleteProcThreadAttributeList: { args: ["ptr"], returns: "void" },
    CreateProcessW: { args: ["ptr", "ptr", "ptr", "ptr", "i32", "u32", "ptr", "ptr", "ptr", "ptr"], returns: "i32" },
    CloseHandle: { args: ["ptr"], returns: "i32" },
    ReadFile: { args: ["ptr", "ptr", "u32", "ptr", "ptr"], returns: "i32" },
    WriteFile: { args: ["ptr", "ptr", "u32", "ptr", "ptr"], returns: "i32" },
    PeekNamedPipe: { args: ["ptr", "ptr", "u32", "ptr", "ptr", "ptr"], returns: "i32" },
    GetExitCodeProcess: { args: ["ptr", "ptr"], returns: "i32" },
    TerminateProcess: { args: ["ptr", "u32"], returns: "i32" },
    GetLastError: { args: [], returns: "u32" },
    SetNamedPipeHandleState: { args: ["ptr", "ptr", "ptr", "ptr"], returns: "i32" },
  }).symbols;
  globalThis.BP_CONPTY = { k, ptr: ffi.ptr, read: ffi.read };
  return globalThis.BP_CONPTY;
}

// one argument as CreateProcessW's command line parses it back (the
// rules of CommandLineToArgvW: backslashes double only before a quote)
function bp_win_quote(a) {
  if (a !== "" && !/[ \t\n\v"]/.test(a)) {
    return a;
  }
  let out = '"';
  let bs = 0;
  for (const c of a) {
    if (c === "\\") {
      bs += 1;
    } else if (c === '"') {
      out += "\\".repeat(bs * 2 + 1) + '"';
      bs = 0;
    } else {
      out += "\\".repeat(bs) + c;
      bs = 0;
    }
  }
  return out + "\\".repeat(bs * 2) + '"';
}

function bp_wide(s) {
  return Buffer.from(s + "\0", "utf16le");
}

function bp_conpty(argv, cwd, env, cols, rows) {
  const { k, ptr, read } = bp_conpty_lib();
  // BACKPLANE_DEBUG_CONPTY=1: what the console and the process did, to
  // stderr (creation results, the first reads, the exit)
  const dbg = process.env.BACKPLANE_DEBUG_CONPTY === "1"
    ? (...a) => { try { require("fs").writeSync(2, "conpty: " + a.join(" ") + "\n"); } catch (_) {} }
    : () => {};
  dbg("argv", JSON.stringify(argv), "cwd", JSON.stringify(cwd), "size", cols + "x" + rows);
  const handle = (b) => read.ptr(ptr(b), 0);
  const inR = new BigUint64Array(1), inW = new BigUint64Array(1);
  const outR = new BigUint64Array(1), outW = new BigUint64Array(1);
  const fail = (what) => Object.assign(new Error(what + " failed (" + k.GetLastError() + ")"), { code: "EIO" });
  if (!k.CreatePipe(ptr(inR), ptr(inW), null, 0) || !k.CreatePipe(ptr(outR), ptr(outW), null, 0)) {
    throw fail("CreatePipe");
  }
  const size = (rows & 0xffff) << 16 | (cols & 0xffff);
  const hpc = new BigUint64Array(1);
  const hr = k.CreatePseudoConsole(size >>> 0, handle(inR), handle(outW), 0, ptr(hpc));
  dbg("CreatePseudoConsole hr", hr >>> 0, "hpc", String(hpc[0]));
  // the console holds its own copies: ours of its ends go
  k.CloseHandle(handle(inR));
  k.CloseHandle(handle(outW));
  if (hr !== 0) {
    k.CloseHandle(handle(inW));
    k.CloseHandle(handle(outR));
    throw fail("CreatePseudoConsole");
  }
  const pc = handle(hpc);
  // the input pipe never blocks the hub: PIPE_NOWAIT makes WriteFile take
  // what fits and answer at once; the rest waits in a queue the 15 ms
  // timer drains (a big paste into a program that is not reading stalled
  // the whole loop on a blocking write)
  k.SetNamedPipeHandleState(handle(inW), ptr(new Uint32Array([1])), null, null);
  const need = new BigUint64Array(1);
  k.InitializeProcThreadAttributeList(null, 1, 0, ptr(need));
  const list = new Uint8Array(Number(need[0]) + 64);
  // STARTUPINFOEXW: STARTUPINFOW (104 bytes on x64) then the list pointer
  const si = new Uint8Array(112);
  const pi = new Uint8Array(24);
  const sv = new DataView(si.buffer);
  sv.setUint32(0, 112, true);
  // A parent whose own stdio is redirected (the hub, a service) passes those
  // handles on unless STARTF_USESTDHANDLES says otherwise: the child then
  // wrote to them and the pseudoconsole saw nothing (session 7: cmd's and
  // bash's echo on the probe's own stdout). With the flag and null handles
  // the child takes the console's (node-pty's ConPTY does the same). Session
  // 6's crash with it came with a wrong attribute value (no console at all).
  // BACKPLANE_CONPTY_STDHANDLES=invalid passes INVALID_HANDLE_VALUE, =none
  // leaves the flag off, to compare on a machine.
  const how = process.env.BACKPLANE_CONPTY_STDHANDLES || "null";
  if (how !== "none") {
    sv.setUint32(60, 0x100, true);
    if (how === "invalid") {
      for (const off of [80, 88, 96]) {
        sv.setBigUint64(off, 0xffffffffffffffffn, true);
      }
    }
  }
  dbg("std handles", how);
  if (!k.InitializeProcThreadAttributeList(ptr(list), 1, 0, ptr(need))
    // the attribute's value is the HPCON itself (not a pointer to it): passed
    // a pointer, the child got no pseudoconsole and its output went nowhere
    || !k.UpdateProcThreadAttribute(ptr(list), 0, 0x00020016n, pc, 8n, null, null)) {
    k.ClosePseudoConsole(pc);
    throw fail("ProcThreadAttribute");
  }
  new DataView(si.buffer).setBigUint64(104, BigInt(ptr(list)), true);
  const line = bp_wide(argv.map(bp_win_quote).join(" "));
  // the environment as Windows wants it: one entry per name (names are
  // case-insensitive), sorted, with SystemRoot and windir (DLL initialization
  // fails without them: 0xC0000142) and the program's folder first on PATH
  // (Cygwin's programs find cygwin1.dll there)
  const names = new Map();
  for (const [n, v] of Object.entries(env)) {
    if (n !== "" && !n.includes("=") && v !== undefined) {
      names.set(n.toUpperCase(), [n, String(v)]);
    }
  }
  const root = process.env.SystemRoot || process.env.SYSTEMROOT || process.env.windir || "C:\\Windows";
  if (!names.has("SYSTEMROOT")) names.set("SYSTEMROOT", ["SystemRoot", root]);
  if (!names.has("WINDIR")) names.set("WINDIR", ["windir", root]);
  const at = argv[0].lastIndexOf("\\");
  if (at > 0) {
    const dir = argv[0].slice(0, at);
    const path = names.get("PATH");
    names.set("PATH", [path ? path[0] : "Path", path ? dir + ";" + path[1] : dir]);
  }
  const lines = [...names.entries()].sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0)).map(([, [n, v]]) => n + "=" + v + "\0");
  const block = Buffer.from(lines.join("") + "\0", "utf16le");
  dbg("env", lines.length, "names; SystemRoot", JSON.stringify(names.get("SYSTEMROOT")?.[1] ?? ""));
  const dir = cwd === "" ? null : bp_wide(cwd);
  // EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT
  const ok = k.CreateProcessW(null, ptr(line), null, null, 0, 0x00080000 | 0x00000400,
    ptr(block), dir === null ? null : ptr(dir), ptr(si), ptr(pi));
  k.DeleteProcThreadAttributeList(ptr(list));
  dbg("CreateProcessW", ok, ok ? "" : "error " + k.GetLastError());
  if (!ok) {
    const e = k.GetLastError();
    k.ClosePseudoConsole(pc);
    k.CloseHandle(handle(inW));
    k.CloseHandle(handle(outR));
    throw Object.assign(new Error("CreateProcessW failed (" + e + ")"), { code: e === 2 || e === 3 ? "ENOENT" : "EIO" });
  }
  const pv = new DataView(pi.buffer);
  const proc = read.ptr(ptr(pi), 0);
  k.CloseHandle(read.ptr(ptr(pi), 8));
  const pid = pv.getUint32(16, true);
  dbg("pid", pid);
  let reads = 0;
  const term = { closed: false, data: null, exit: null };
  const child = { pid, terminal: term, exitCode: null, signalCode: null };
  let done;
  child.exited = new Promise((r) => { done = r; });
  const buf = new Uint8Array(65536);
  const n = new Uint32Array(1);
  const avail = new Uint32Array(1);
  const code = new Uint32Array(1);
  const hang = () => {
    if (!term.closed) {
      term.closed = true;
      k.ClosePseudoConsole(pc);
      k.CloseHandle(handle(inW));
    }
  };
  const tick = () => {
    pump();
    // what the console wrote, then whether the process ended
    while (k.PeekNamedPipe(handle(outR), null, 0, null, ptr(avail), null) && avail[0] > 0) {
      if (!k.ReadFile(handle(outR), ptr(buf), Math.min(avail[0], buf.length), ptr(n), null) || n[0] === 0) {
        break;
      }
      if (reads < 20) {
        reads += 1;
        dbg("read", n[0], "bytes:", JSON.stringify(Buffer.from(buf.slice(0, Math.min(n[0], 80))).toString("latin1")));
      }
      term.data?.(term, buf.slice(0, n[0]));
    }
    if (child.exitCode === null && k.GetExitCodeProcess(proc, ptr(code)) && code[0] !== 259) {
      child.exitCode = code[0] & 0xff;
      dbg("exit", code[0], "after", reads, "reads");
      hang();
    }
    if (child.exitCode !== null && !(k.PeekNamedPipe(handle(outR), null, 0, null, ptr(avail), null) && avail[0] > 0)) {
      clearInterval(timer);
      k.CloseHandle(handle(outR));
      k.CloseHandle(proc);
      term.exit?.(term, 0, null);
      done(child.exitCode);
    }
  };
  const timer = setInterval(tick, 15);
  const queue = [];
  const wrote = new Uint32Array(1);
  const pump = () => {
    while (queue.length > 0 && !term.closed) {
      const head = queue[0];
      const part = head.subarray(0, 4096);
      if (!k.WriteFile(handle(inW), ptr(part), part.length, ptr(wrote), null)) {
        // the console is gone (ERROR_NO_DATA): what is left goes nowhere
        queue.length = 0;
        return;
      }
      if (wrote[0] === 0) {
        return;
      }
      if (wrote[0] >= head.length) {
        queue.shift();
      } else {
        queue[0] = head.subarray(wrote[0]);
      }
    }
  };
  term.write = (b) => {
    if (term.closed) {
      return 0;
    }
    const u = typeof b === "string" ? Buffer.from(b) : new Uint8Array(b);
    queue.push(u);
    pump();
    return u.length;
  };
  term.resize = (c, r) => {
    if (!term.closed) {
      k.ResizePseudoConsole(pc, ((r & 0xffff) << 16 | (c & 0xffff)) >>> 0);
    }
  };
  term.close = () => {
    hang();
  };
  // a signal ends it: Windows has no signals, so the code is 128 + sig
  child.kill = (sig) => {
    if (child.exitCode === null) {
      k.TerminateProcess(proc, 128 + ((sig ?? 15) & 0x7f));
    }
    hang();
  };
  return child;
}

// the console's output into the handle, its end as the stream's end
function bp_conpty_wire(child, h) {
  child.terminal.data = (_, d) => h.feed(d);
  child.terminal.exit = () => h.finish(0);
  return child;
}

// On Windows a command named by an absolute POSIX path that is not there
// ("/bin/bash", the hub's default SHELL; "/bin/sh") is the program of that
// name on PATH outside the system's folders: the Windows package ships its
// runtime's bin (bash.exe, sh.exe) there. Anything else is left as it is.
// PATH by any case of its name: Windows keeps it as "Path", and the
// environment object need not match names case-insensitively
function bp_pty_path() {
  const key = Object.keys(process.env).find((k) => k.toUpperCase() === "PATH");
  return key === undefined ? "" : String(process.env[key] ?? "");
}

function bp_pty_win_cmd(cmd) {
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
  for (const dir of bp_pty_path().split(";")) {
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

function bp_pty_spawn(cmd, args, cwd, cols, rows) {
  cmd = bp_pty_win_cmd(cmd);
  const fs = require("fs");
  const argv = [cmd, ...args];
  if (cmd === "" || argv.some((a) => a.includes("\0")) || cwd.includes("\0")) {
    return io_fail(process.platform === "darwin" ? 92 : 84);
  }
  if (process.env.BACKPLANE_DEBUG_CONPTY === "1") {
    require("fs").writeSync(2, "pty: " + JSON.stringify(cmd) + " -> " + JSON.stringify(Bun.which(cmd, { PATH: bp_pty_path() })) + " (PATH " + bp_pty_path() + ")\n");
  }
  if (Bun.which(cmd, { PATH: bp_pty_path() }) === null
    && !((cmd.includes("/") || cmd.includes("\\")) && fs.existsSync(cmd))) {
    return io_fail(globalThis.BP_H.ERRNO.ENOENT);
  }
  if (cwd !== "") {
    try {
      if (!fs.statSync(cwd).isDirectory()) {
        return io_fail(globalThis.BP_H.ERRNO.ENOTDIR);
      }
    } catch (e) {
      return io_fail(globalThis.BP_H.errno(e));
    }
  }
  const env = {};
  for (const [k, v] of Object.entries(process.env)) {
    if (v !== undefined && k !== "" && !k.includes("=") && k !== "TERM"
      && k !== "COLORTERM" && k !== "LINES" && k !== "COLUMNS") {
      env[k] = v;
    }
  }
  env.TERM = "xterm-256color";
  env.COLORTERM = "truecolor";
  const h = new (bp_pty_class())();
  let child;
  try {
    child = process.platform === "win32"
      ? bp_conpty_wire(bp_conpty([Bun.which(cmd, { PATH: bp_pty_path() }) ?? cmd, ...args], cwd, env,
        bp_pty_size(Number(cols), 80), bp_pty_size(Number(rows), 24)), h)
      : Bun.spawn(argv, {
      cwd: cwd === "" ? undefined : cwd,
      env,
      terminal: {
        cols: bp_pty_size(Number(cols), 80),
        rows: bp_pty_size(Number(rows), 24),
        name: "xterm-256color",
        data: (_, d) => h.feed(d),
        exit: () => h.finish(0),
      },
    });
  } catch (e) {
    return io_fail(globalThis.BP_H.errno(e));
  }
  h.t = child.terminal;
  bp_pty_procs().set(child.pid >>> 0, child);
  return io_done(io_tup(child.pid >>> 0, globalThis.BP_H.add(h)));
}

// Answers (pid, master socket).
function pty_spawn(cmd, args, cwd, cols, rows) {
  if (bp_pty_bun()) {
    const list = [];
    for (let xs = args; xs.$ === "Con"; xs = xs.tail) {
      list.push(xs.head);
    }
    return bp_pty_spawn(cmd, list, cwd, cols, rows);
  }
  if (process.platform === "win32") {
    return io_fail(38);
  }
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
    code = pty_child_stack(() => p.posix_spawn(p.ptr(pid), p.ptr(eb), p.ptr(fa), p.ptr(at), p.ptr(av), p.ptr(ev)));
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
  // on Bun's loop the master is a handle over the descriptor (loop.js's
  // BpFd: EIO reads as the end), its fd kept for Pty.resize
  const bun = globalThis.BEND_ASYNC === true && globalThis.BP_H?.fromFd !== undefined;
  return io_done(io_tup(pid[0] >>> 0, bun ? globalThis.BP_H.fromFd(m, "pty") : m));
}

// a new window size; the kernel sends the child SIGWINCH
function pty_resize(socket, cols, rows) {
  if (globalThis.BP_H !== undefined && globalThis.BP_H.is(socket)) {
    const h = globalThis.BP_H.get(socket);
    if (h?.fd !== undefined) {
      pty_size(h.fd, Number(cols), Number(rows));
    } else {
      h?.resize?.(bp_pty_size(Number(cols), 80), bp_pty_size(Number(rows), 24));
    }
    return socket;
  }
  pty_size(Number(socket), Number(cols), Number(rows));
  return socket;
}

io_eff(CID(Pty.spawn), pty_spawn);
io_eff(CID(Pty.resize), pty_resize);
