// The JS twin of hublock.c: one hub per home, an exclusive flock on
// <home>/hub.lock held for the life of the process. The descriptor is
// close-on-exec, so no child holds it; before an exec (host.js's Sys.exec
// calls BACKPLANE_HUBLOCK_EXEC) it survives the exec instead and its
// number goes in BACKPLANE_HUB_LOCK_FD, and the new image takes that same
// descriptor back (the lock is never let go across a restart). Imports are
// lazy so a web or phone bundle can include it without touching IO.
let bp_hub_fd = -1;
let bp_hub_ffi;
function bp_hub_libc() {
  if (!bp_hub_ffi) {
    const ffi = require("bun:ffi");
    const mac = process.platform === "darwin";
    const vari = mac && process.arch === "arm64";
    const c = ffi.dlopen(mac ? "/usr/lib/libSystem.B.dylib" : "libc.so.6", {
      flock: { args: ["i32", "i32"], returns: "i32" },
      fcntl: { args: vari ? ["i32", "i32", "i32", "i32", "i32", "i32", "i32", "i32", "i32"] : ["i32", "i32", "i32"], returns: "i32" },
      setenv: { args: ["ptr", "ptr", "i32"], returns: "i32" },
      unsetenv: { args: ["ptr"], returns: "i32" },
      [mac ? "__error" : "__errno_location"]: { args: [], returns: "ptr" },
    }).symbols;
    const cs = (s) => ffi.ptr(Buffer.from(s + "\0"));
    bp_hub_ffi = {
      flock: c.flock,
      fcntl: (fd, cmd, arg) => vari ? c.fcntl(fd, cmd, 0, 0, 0, 0, 0, 0, arg) : c.fcntl(fd, cmd, arg),
      setenv: (k, v) => { process.env[k] = v; c.setenv(cs(k), cs(v), 1); },
      unsetenv: (k) => { delete process.env[k]; c.unsetenv(cs(k)); },
      errno: () => ffi.read.i32(c[mac ? "__error" : "__errno_location"](), 0),
      mac,
    };
  }
  return bp_hub_ffi;
}

// Before an exec (keep 1) the lock's descriptor survives it and is named
// in the environment; after an exec that failed (keep 0) it is
// close-on-exec again.
globalThis.BACKPLANE_HUBLOCK_EXEC = (keep) => {
  if (bp_hub_fd < 0) return;
  const c = bp_hub_libc();
  if (keep) {
    c.fcntl(bp_hub_fd, 2, 0);
    c.setenv("BACKPLANE_HUB_LOCK_FD", String(bp_hub_fd));
  } else {
    c.fcntl(bp_hub_fd, 2, 1);
    c.unsetenv("BACKPLANE_HUB_LOCK_FD");
  }
};

// An image an exec handed the lock to: its descriptor is close-on-exec
// again at load, so no child started before the lock is taken back holds
// it past the hub's death.
(() => {
  try {
    const env = typeof process === "undefined" ? "" : process.env.BACKPLANE_HUB_LOCK_FD ?? "";
    const fd = env ? parseInt(env, 10) : -1;
    if (fd > 2) bp_hub_libc().fcntl(fd, 2, 1);
  } catch (_) {
  }
})();

// fd is open on the file at path
function bp_hub_same(fd, path) {
  if (fd < 0) return false;
  const fs = require("node:fs");
  try {
    const a = fs.fstatSync(fd), b = fs.statSync(path);
    return a.dev === b.dev && a.ino === b.ino;
  } catch {
    return false;
  }
}

// what the holder wrote ("<pid> <port>"); a holder that has just taken the
// lock writes it at once, so an empty file is read again for a moment
function bp_hub_read(path) {
  for (let i = 0; i < 20; i++) {
    try {
      const line = require("node:fs").readFileSync(path, "utf8").split("\n")[0];
      if (line) return line;
    } catch {
    }
    Bun.sleepSync(50);
  }
  return "";
}

function bp_hub_fail(path, code) {
  return "fail " + path + ": " + io_strerror(code);
}

// Windows: no flock. The hub keeps the file open for writing and lets
// others only read it (CreateFileW, FILE_SHARE_READ): another hub's open
// for writing fails with a sharing violation, while the line it wrote stays
// readable, and the system lets go when the hub dies. No exec hands it on
// (Windows has none; the supervisor will own restarts).
let bp_hub_h = null;
let bp_hub_k32;
function bp_hub_win() {
  if (!bp_hub_k32) {
    const ffi = require("bun:ffi");
    bp_hub_k32 = { ffi, ...ffi.dlopen("kernel32.dll", {
      CreateFileW: { args: ["ptr", "u32", "u32", "ptr", "u32", "u32", "ptr"], returns: "i64" },
      CloseHandle: { args: ["i64"], returns: "i32" },
      WriteFile: { args: ["i64", "ptr", "u32", "ptr", "ptr"], returns: "i32" },
      SetFilePointerEx: { args: ["i64", "i64", "ptr", "u32"], returns: "i32" },
      SetEndOfFile: { args: ["i64"], returns: "i32" },
      GetLastError: { args: [], returns: "u32" },
    }).symbols };
  }
  return bp_hub_k32;
}

// a handle on path for writing (share: what others may still do), or the
// Windows error
function bp_hub_open(path, share, create) {
  const k = bp_hub_win();
  const name = Buffer.from(require("node:path").resolve(path) + "\0", "utf16le");
  // GENERIC_WRITE; OPEN_ALWAYS or OPEN_EXISTING; FILE_ATTRIBUTE_NORMAL
  const h = k.CreateFileW(k.ffi.ptr(name), 0x40000000, share, null, create ? 4 : 3, 0x80, null);
  return Number(h) === -1 ? { err: k.GetLastError() } : { h };
}

function hub_lock_take_win(path, port) {
  const k = bp_hub_win();
  if (bp_hub_h === null) {
    const got = bp_hub_open(path, 1, true);
    if (got.err !== undefined) {
      // ERROR_SHARING_VIOLATION / ERROR_LOCK_VIOLATION
      return got.err === 32 || got.err === 33 ? "held " + bp_hub_read(path)
        : "fail " + path + ": Windows error " + got.err;
    }
    bp_hub_h = got.h;
  }
  const line = Buffer.from(`${process.pid} ${port} flock\n`);
  const n = new Uint32Array(1);
  k.SetFilePointerEx(bp_hub_h, 0, null, 0);
  k.WriteFile(bp_hub_h, k.ffi.ptr(line), line.length, k.ffi.ptr(n), null);
  k.SetEndOfFile(bp_hub_h);
  return "ok";
}

function hub_lock_held_win(path) {
  if (bp_hub_h !== null) return "";
  // FILE_SHARE_READ | FILE_SHARE_WRITE: refused only while a holder writes
  const got = bp_hub_open(path, 3, false);
  if (got.err === undefined) {
    bp_hub_win().CloseHandle(got.h);
    return "";
  }
  return got.err === 32 || got.err === 33 ? bp_hub_read(path) || "? ?" : "";
}

// "ok" (this process holds the lock, and the file says "<pid> <port>
// flock"), "held <pid> <port> flock" (another process holds it) or
// "fail <why>"
function hub_lock_take(path, port) {
  if (process.platform === "win32") return hub_lock_take_win(path, port);
  const line = `${process.pid} ${port} flock`;
  const fs = require("node:fs");
  const c = bp_hub_libc();
  let fd = -1;
  if (bp_hub_same(bp_hub_fd, path)) {
    fd = bp_hub_fd;
  } else {
    const env = process.env.BACKPLANE_HUB_LOCK_FD ?? "";
    if (env) {
      const got = parseInt(env, 10);
      c.unsetenv("BACKPLANE_HUB_LOCK_FD");
      // the descriptor the image before the exec held, on this same file
      if (got > 2 && bp_hub_same(got, path) && c.flock(got, 2 | 4) === 0) {
        c.fcntl(got, 2, 1);
        fd = got;
      }
    }
  }
  if (fd < 0) {
    let opened;
    try {
      opened = fs.openSync(path, fs.constants.O_RDWR | fs.constants.O_CREAT, 0o644);
    } catch (e) {
      return bp_hub_fail(path, Math.abs(e?.errno ?? 5));
    }
    fd = c.fcntl(opened, c.mac ? 67 : 1030, 3);
    const code = c.errno();
    fs.closeSync(opened);
    if (fd < 0) return bp_hub_fail(path, code);
    // LOCK_EX | LOCK_NB
    if (c.flock(fd, 2 | 4) !== 0) {
      const err = c.errno();
      fs.closeSync(fd);
      return err === (c.mac ? 35 : 11) ? "held " + bp_hub_read(path) : bp_hub_fail(path, err);
    }
  }
  if (bp_hub_fd >= 0 && bp_hub_fd !== fd) {
    try {
      fs.closeSync(bp_hub_fd);
    } catch {
    }
  }
  bp_hub_fd = fd;
  try {
    fs.ftruncateSync(fd, 0);
    fs.writeSync(fd, line + "\n", 0);
  } catch {
  }
  return "ok";
}

// "" when no other process holds the lock (or this one does), else what
// the holder wrote ("<pid> <port>"; "? ?" when it wrote nothing)
function hub_lock_held(path) {
  if (process.platform === "win32") return hub_lock_held_win(path);
  if (bp_hub_same(bp_hub_fd, path)) return "";
  const fs = require("node:fs");
  const c = bp_hub_libc();
  let fd;
  try {
    fd = fs.openSync(path, "r");
  } catch {
    return "";
  }
  let out = "";
  // LOCK_SH | LOCK_NB, then LOCK_UN
  if (c.flock(fd, 1 | 4) === 0) {
    c.flock(fd, 8);
  } else if (c.errno() === (c.mac ? 35 : 11)) {
    out = bp_hub_read(path) || "? ?";
  }
  fs.closeSync(fd);
  return out;
}
io_eff(CID(HubLock.take), hub_lock_take);
io_eff(CID(HubLock.held), hub_lock_held);
