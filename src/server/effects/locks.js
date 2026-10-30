// The Bun hub's twin of locks.c: the same POSIX flock, and the same guard
// child, started with posix_spawn (no Node child_process: Bun's event loop
// never runs under Bend's IO loop). The child gets stdin the pipe, fd 3
// the flock (kept across exec), stdout and stderr /dev/null, nothing else
// open. Imports are lazy so a web/phone bundle can include these effects
// without touching IO.
const bp_lock_guards = new Map();
let bp_lock_ffi;
function bp_lock_posix() {
  if (!bp_lock_ffi) {
    const ffi = require("bun:ffi");
    const mac = process.platform === "darwin";
    const vari = mac && process.arch === "arm64";
    const lib = mac ? "/usr/lib/libSystem.B.dylib" : "libc.so.6";
    const c = ffi.dlopen(lib, {
      flock: { args: ["i32", "i32"], returns: "i32" },
      close: { args: ["i32"], returns: "i32" },
      pipe: { args: ["ptr"], returns: "i32" },
      waitpid: { args: ["i32", "ptr", "i32"], returns: "i32" },
      fcntl: { args: vari ? ["i32", "i32", "i32", "i32", "i32", "i32", "i32", "i32", "i32"] : ["i32", "i32", "i32"], returns: "i32" },
      posix_spawn: { args: ["ptr", "ptr", "ptr", "ptr", "ptr", "ptr"], returns: "i32" },
      posix_spawn_file_actions_init: { args: ["ptr"], returns: "i32" },
      posix_spawn_file_actions_destroy: { args: ["ptr"], returns: "i32" },
      posix_spawn_file_actions_adddup2: { args: ["ptr", "i32", "i32"], returns: "i32" },
      posix_spawn_file_actions_addopen: { args: ["ptr", "i32", "ptr", "i32", "u32"], returns: "i32" },
      posix_spawn_file_actions_addclose: { args: ["ptr", "i32"], returns: "i32" },
      posix_spawnattr_init: { args: ["ptr"], returns: "i32" },
      posix_spawnattr_destroy: { args: ["ptr"], returns: "i32" },
      posix_spawnattr_setflags: { args: ["ptr", "i16"], returns: "i32" },
      [mac ? "__error" : "__errno_location"]: { args: [], returns: "ptr" },
    }).symbols;
    let closefrom;
    try {
      closefrom = ffi.dlopen(lib, { posix_spawn_file_actions_addclosefrom_np: { args: ["ptr", "i32"], returns: "i32" } })
        .symbols.posix_spawn_file_actions_addclosefrom_np;
    } catch (_) {
    }
    let pidfd;
    try {
      pidfd = mac ? undefined : ffi.dlopen(lib, { pidfd_open: { args: ["i32", "u32"], returns: "i32" } }).symbols.pidfd_open;
    } catch (_) {
    }
    bp_lock_ffi = { ...c, closefrom, pidfd, mac, ptr: ffi.ptr,
      fcntl: (fd, cmd, arg) => vari ? c.fcntl(fd, cmd, 0, 0, 0, 0, 0, 0, arg) : c.fcntl(fd, cmd, arg),
      errno: () => ffi.read.i32(c[mac ? "__error" : "__errno_location"](), 0) };
  }
  return bp_lock_ffi;
}

// the descriptors open above 3 (closed in the child where closefrom is
// missing); the listing's own, closed by now, is left out (a close action
// on it fails the spawn with EBADF on macOS)
function bp_lock_fds() {
  try {
    return require("node:fs").readdirSync(process.platform === "darwin" ? "/dev/fd" : "/proc/self/fd")
      .map(Number).filter((fd) => fd > 3 && io_sys().fcntl(fd, 1, 0) >= 0);
  } catch (_) {
    return [];
  }
}

function bp_lock_spawn(c, pipe, fd, home, holder, script) {
  const cs = (s) => Buffer.from(s + "\0");
  const strs = ["sh", "-c", script, "sh", home, holder].map(cs);
  if ([script, home, holder].some((s) => s.includes("\0"))) return -1;
  const argv = new BigUint64Array(strs.length + 1);
  strs.forEach((b, i) => { argv[i] = BigInt(c.ptr(b)); });
  const envs = Object.entries(process.env).filter(([k, v]) => v !== undefined && k && !k.includes("="))
    .map(([k, v]) => cs(k + "=" + v)).filter((b) => !b.subarray(0, b.length - 1).includes(0));
  const envp = new BigUint64Array(envs.length + 1);
  envs.forEach((b, i) => { envp[i] = BigInt(c.ptr(b)); });
  const fa = new Uint8Array(1024);
  const pid = new Int32Array(1);
  const sh = cs("/bin/sh"), devnull = cs("/dev/null");
  if (c.posix_spawn_file_actions_init(c.ptr(fa)) !== 0) return -1;
  let ok = c.posix_spawn_file_actions_adddup2(c.ptr(fa), pipe, 0) === 0
    && c.posix_spawn_file_actions_adddup2(c.ptr(fa), fd, 3) === 0
    && c.posix_spawn_file_actions_addopen(c.ptr(fa), 1, c.ptr(devnull), 1, 0) === 0
    && c.posix_spawn_file_actions_adddup2(c.ptr(fa), 1, 2) === 0;
  // macOS: POSIX_SPAWN_CLOEXEC_DEFAULT closes the rest (a close action on
  // a descriptor the child does not inherit, Bun's kqueue, fails the spawn)
  const at = new Uint8Array(1024);
  const attr = c.mac && c.posix_spawnattr_init(c.ptr(at)) === 0
    && c.posix_spawnattr_setflags(c.ptr(at), 0x4000) === 0;
  if (c.mac) {
    ok = ok && attr;
  } else if (ok && c.closefrom !== undefined) {
    ok = c.closefrom(c.ptr(fa), 4) === 0;
  } else if (ok) {
    for (const n of bp_lock_fds()) c.posix_spawn_file_actions_addclose(c.ptr(fa), n);
  }
  let got = -1;
  if (ok && c.posix_spawn(c.ptr(pid), c.ptr(sh), c.ptr(fa), attr ? c.ptr(at) : null, c.ptr(argv), c.ptr(envp)) === 0) {
    got = pid[0];
  }
  if (attr) {
    c.posix_spawnattr_destroy(c.ptr(at));
  }
  c.posix_spawn_file_actions_destroy(c.ptr(fa));
  return got;
}

// Windows: no flock and no descriptor passing. The hub holds the lock file
// open with no sharing (CreateFileW, share mode 0), so another taker gets a
// sharing violation and the system lets go when the hub dies; the cleanup
// is the same script under the package's sh, started with a stdin pipe the
// hub closes (or dies with). Unlike POSIX the lock goes with the hub, not
// after the cleanup ran.
let bp_lock_k32;
function bp_lock_win() {
  if (!bp_lock_k32) {
    const ffi = require("bun:ffi");
    bp_lock_k32 = { ffi, ...ffi.dlopen("kernel32.dll", {
      CreateFileW: { args: ["ptr", "u32", "u32", "ptr", "u32", "u32", "ptr"], returns: "i64" },
      CloseHandle: { args: ["i64"], returns: "i32" },
    }).symbols };
  }
  return bp_lock_k32;
}

function guard_take_win(name, holder, home, script) {
  const k = bp_lock_win();
  const dir = process.env.BACKPLANE_LOCK_DIR || require("node:os").tmpdir();
  const path = Buffer.from(require("node:path").join(dir, `bp-${name}.lock`) + "\0", "utf16le");
  // GENERIC_READ|GENERIC_WRITE, no sharing, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL
  const h = k.CreateFileW(k.ffi.ptr(path), 0xC0000000, 0, null, 4, 0x80, null);
  // INVALID_HANDLE_VALUE is -1
  if (Number(h) === -1) return false;
  let child;
  try {
    child = Bun.spawn(["sh", "-c", script, "sh", home, holder],
      { stdin: "pipe", stdout: "ignore", stderr: "ignore" });
  } catch (_) {
    k.CloseHandle(h);
    return false;
  }
  bp_lock_guards.set(name, { holder, h, child });
  return true;
}

function guard_drop_win(g, k) {
  g.child.stdin.end();
  return io_park_promise(g.child.exited, k, () => {
    bp_lock_win().CloseHandle(g.h);
    return { $: "Unit" };
  });
}

// (a Bend Bool is a JS boolean)
function guard_take(name, holder, home, script) {
  if (!/^[A-Za-z0-9_-]{1,64}$/.test(name) || !holder) return false;
  const prior = bp_lock_guards.get(name);
  if (prior) return prior.holder === holder;
  if (process.platform === "win32") return guard_take_win(name, holder, home, script);
  const fs = require("node:fs");
  const c = bp_lock_posix();
  const dupx = c.mac ? 67 : 1030;
  let opened;
  try {
    opened = fs.openSync(`${process.env.BACKPLANE_LOCK_DIR || "/tmp"}/bp-${name}.lock`, fs.constants.O_RDWR | fs.constants.O_CREAT, 0o600);
  } catch {
    return false;
  }
  let fd = c.fcntl(opened, dupx, 3);
  fs.closeSync(opened);
  if (fd < 0) return false;
  if (c.flock(fd, 2 | 4) !== 0) { c.close(fd); return false; }
  const raw = new Int32Array(2);
  if (c.pipe(c.ptr(raw)) !== 0) { c.close(fd); return false; }
  const p = [c.fcntl(raw[0], dupx, 3), c.fcntl(raw[1], dupx, 3)];
  c.close(raw[0]); c.close(raw[1]);
  if (p[0] < 0 || p[1] < 0) {
    if (p[0] >= 0) c.close(p[0]);
    if (p[1] >= 0) c.close(p[1]);
    c.close(fd);
    return false;
  }
  // fd 3 would be overwritten by the pipe's dup2 order otherwise
  if (fd === 3) {
    const moved = c.fcntl(fd, dupx, 4);
    c.close(fd);
    if (moved < 0) { c.close(p[0]); c.close(p[1]); return false; }
    fd = moved;
  }
  const pid = bp_lock_spawn(c, p[0], fd, home, holder, script);
  c.close(p[0]);
  c.close(fd);
  if (pid < 0) { c.close(p[1]); return false; }
  bp_lock_guards.set(name, { holder, pipe: p[1], pid });
  return true;
}

// the pid's exit reaped: true once it is (or it is no child of ours)
function bp_lock_reaped(c, pid) {
  const st = new Int32Array(1);
  for (;;) {
    const r = c.waitpid(pid, c.ptr(st), 1);
    if (r === pid) return true;
    if (r === 0) return false;
    if (c.errno() !== 4) return true;
  }
}

// Closing the pipe starts a non-killing ownership cleanup. Return at once:
// a live job may retain the guard for hours, and must not park the hub.
const bp_lock_retired = [];
function guard_reap() {
  if (process.platform === 'win32' || bp_lock_retired.length === 0) return 0;
  const c = bp_lock_posix();
  for (let i = bp_lock_retired.length - 1; i >= 0; --i) {
    if (bp_lock_reaped(c, bp_lock_retired[i])) bp_lock_retired.splice(i, 1);
  }
  return bp_lock_retired.length;
}
function guard_drop(name, holder, k) {
  guard_reap();
  const g = bp_lock_guards.get(name), unit = { $: 'Unit' };
  if (!g || g.holder !== holder) return unit;
  bp_lock_guards.delete(name);
  if (process.platform === 'win32') return guard_drop_win(g, k);
  const c = bp_lock_posix();
  c.close(g.pipe);
  if (!bp_lock_reaped(c, g.pid)) bp_lock_retired.push(g.pid);
  return unit;
}
io_eff(CID(Guard.take), guard_take);
io_eff(CID(Guard.drop), guard_drop);

io_eff(CID(Guard.reap), guard_reap);
