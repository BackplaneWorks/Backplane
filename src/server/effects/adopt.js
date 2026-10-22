// The JS twin of adopt.c: agents kept through a restart in place
// (src/server/adopt.bend; the decisions are src/core/adopt.bend's). A
// record of every agent process the hub started: thread, pid, start
// identity (boot id and /proc/<pid>/stat field 22, so a reused pid never
// matches) and a descriptor on its socket, kept in memory and written to
// the record file on each change. Before an exec (host.js's Sys.exec calls
// BACKPLANE_ADOPT_EXEC) the descriptors lose FD_CLOEXEC, so the new image
// reads and writes the same processes. Imports are lazy, so a bundle can
// include it without touching IO.

const bp_adopt = { path: "", list: [], inherited: [] };

function bp_adopt_libc() {
  if (globalThis.BACKPLANE_ADOPT_LIBC === undefined) {
    const ffi = require("bun:ffi");
    const mac = process.platform === "darwin";
    const vari = mac && process.arch === "arm64";
    const c = ffi.dlopen(mac ? "libSystem.dylib" : "libc.so.6", {
      close: { args: ["i32"], returns: "i32" },
      kill: { args: ["i32", "i32"], returns: "i32" },
      waitpid: { args: ["i32", "ptr", "i32"], returns: "i32" },
      fcntl: { args: vari ? ["i32", "i32", "i32", "i32", "i32", "i32", "i32", "i32", "i32"] : ["i32", "i32", "i32"], returns: "i32" },
      [mac ? "__error" : "__errno_location"]: { args: [], returns: "ptr" },
    }).symbols;
    globalThis.BACKPLANE_ADOPT_LIBC = {
      close: c.close,
      kill: c.kill,
      waitpid: (pid, st, opt) => c.waitpid(pid, ffi.ptr(st), opt),
      fcntl: (fd, cmd, arg) => vari ? c.fcntl(fd, cmd, 0, 0, 0, 0, 0, 0, arg) : c.fcntl(fd, cmd, arg),
      errno: () => ffi.read.i32(c[mac ? "__error" : "__errno_location"](), 0),
      mac,
    };
  }
  return globalThis.BACKPLANE_ADOPT_LIBC;
}

// boot id and start time ("" when the process is gone or unreadable)
function bp_adopt_id(pid) {
  if (pid <= 1) return "";
  const fs = require("fs");
  let stat;
  try {
    stat = fs.readFileSync("/proc/" + pid + "/stat", "latin1");
  } catch (_) {
    return "";
  }
  const p = stat.lastIndexOf(")");
  if (p < 0) return "";
  // after "comm)": field 3 (state) onwards; start time is field 22
  const fields = stat.slice(p + 2).split(" ");
  const m = /^[0-9]+/.exec(fields[19] ?? "");
  if (m === null) return "";
  let boot = "";
  try {
    boot = fs.readFileSync("/proc/sys/kernel/random/boot_id", "latin1").split("\n")[0];
  } catch (_) {
  }
  return boot + ":" + m[0];
}

function bp_adopt_write() {
  if (!bp_adopt.path) return;
  const fs = require("fs");
  const tmp = bp_adopt.path + ".tmp";
  try {
    fs.writeFileSync(tmp, bp_adopt.list.map((a) => `${a.thread}\t${a.pid}\t${a.ident}\t${a.fd}\n`).join(""));
    fs.renameSync(tmp, bp_adopt.path);
  } catch (_) {
  }
}

function bp_adopt_remove(thread, pid, any) {
  bp_adopt.list = bp_adopt.list.filter((a) => {
    if (a.thread === thread && (any || a.pid === pid)) {
      if (a.fd > 2) bp_adopt_libc().close(a.fd);
      return false;
    }
    return true;
  });
}

function bp_inherited_drop(pid) {
  bp_adopt.inherited = bp_adopt.inherited.filter((p) => p !== pid);
}

// Before an exec (keep 1) each recorded descriptor survives it; after an
// exec that failed (keep 0) they are close-on-exec again.
globalThis.BACKPLANE_ADOPT_EXEC = (keep) => {
  for (const a of bp_adopt.list) {
    if (a.fd > 2) bp_adopt_libc().fcntl(a.fd, 2, keep ? 0 : 1);
  }
  if (keep) bp_adopt_write();
};

// the record's file; the record starts empty (and is written so)
function adopt_record(path) {
  bp_adopt.path = path;
  for (const a of bp_adopt.list) {
    if (a.fd > 2) bp_adopt_libc().close(a.fd);
  }
  bp_adopt.list = [];
  bp_adopt_write();
  return { $: "Unit" };
}

// a process the hub started for a thread: recorded with a descriptor of
// its own on the socket (close-on-exec until an exec keeps it); an
// earlier one for the thread leaves the record
function adopt_keep(thread, pid, socket) {
  pid >>>= 0;
  bp_adopt_remove(thread, 0, true);
  bp_inherited_drop(pid);
  // a Bun handle (loop.js's BP_H) or a Windows hub has no descriptor to
  // keep through an exec: the process is recorded alone, as adopt_note's
  // (a handle over a real descriptor, BP_H.fromFd's, keeps that one)
  const h = globalThis.BP_H?.get(socket);
  const fd = h === undefined ? socket : h.fd;
  const bare = process.platform === "win32" || fd === undefined;
  const c = bare ? null : bp_adopt_libc();
  const got = bare ? -1 : c.fcntl(fd, c.mac ? 67 : 1030, 3);
  if (pid > 1) {
    bp_adopt.list.unshift({ thread, pid, ident: bp_adopt_id(pid), fd: got });
  } else if (got > 2) {
    c.close(got);
  }
  bp_adopt_write();
  return socket;
}

// a per-turn process, recorded with no descriptor: it is never adopted, so
// the next image stops it if it still runs
function adopt_note(thread, pid) {
  pid >>>= 0;
  bp_adopt_remove(thread, 0, true);
  if (pid > 1) {
    bp_adopt.list.unshift({ thread, pid, ident: bp_adopt_id(pid), fd: -1 });
  }
  bp_adopt_write();
  return { $: "Unit" };
}

// the process ended: its entry goes (only if it is still that pid's)
function adopt_drop(thread, pid) {
  bp_adopt_remove(thread, pid >>> 0, false);
  bp_adopt_write();
  return { $: "Unit" };
}

function adopt_ident(pid) {
  return bp_adopt_id(pid >>> 0);
}

// a descriptor an exec kept: a socket again (close-on-exec, non-blocking),
// or an error when it is no socket
function adopt_take(fd) {
  fd >>>= 0;
  let sock = false;
  try {
    sock = fd > 2 && require("fs").fstatSync(fd).isSocket();
  } catch (_) {
  }
  if (!sock) return io_fail(9);
  const c = bp_adopt_libc();
  c.fcntl(fd, 2, 1);
  c.fcntl(fd, 4, c.fcntl(fd, 3, 0) | (c.mac ? 4 : 0x800));
  return io_done(fd);
}

// the parent pid in /proc/<pid>/stat (0 when unreadable)
function bp_adopt_ppid(pid) {
  try {
    const stat = require("fs").readFileSync("/proc/" + pid + "/stat", "latin1");
    const m = /^ \S+ ([0-9]+)/.exec(stat.slice(stat.lastIndexOf(")") + 1));
    return m === null ? 0 : Number(m[1]);
  } catch (_) {
    return 0;
  }
}

// whether pid is this process or one of its ancestors
function bp_adopt_ancestor(pid) {
  let p = process.pid;
  for (let depth = 0; depth < 64 && p > 1; depth++) {
    if (p === pid) return true;
    p = bp_adopt_ppid(p);
    if (p === 0) return false;
  }
  return p === pid;
}

// SIGTERM to pid, only when it is still the process recorded (same start
// identity), never 0 or 1, this process or an ancestor; answers whether
// it was signalled
function adopt_stop(pid, want) {
  pid >>>= 0;
  let ok = false;
  if (pid > 1 && want.length > 0 && !bp_adopt_ancestor(pid)) {
    const now = bp_adopt_id(pid);
    if (now && now === want) {
      try {
        process.kill(pid, "SIGTERM");
        ok = true;
      } catch (_) {
      }
    }
    if (ok) bp_inherited_drop(pid);
  }
  return ok;
}

// Children inherited through an exec: at load, before this image starts
// anything, every child there is is one the old image started; each one
// adopted or stopped leaves the list, and Adopt.reap waits, without
// blocking, for the rest (never waitpid(-1): Proc.wait waits by pid).
function bp_inherited_scan() {
  if (typeof process === "undefined" || process.platform !== "linux") return;
  const fs = require("fs");
  let names = [];
  try {
    names = fs.readdirSync("/proc");
  } catch (_) {
    return;
  }
  for (const name of names) {
    if (!/^[0-9]+$/.test(name)) continue;
    const pid = Number(name);
    if (pid <= 1 || pid === process.pid) continue;
    if (bp_adopt_ppid(pid) === process.pid && bp_adopt.inherited.length < 4096) {
      bp_adopt.inherited.push(pid);
    }
  }
}

// waits, without blocking, for each inherited child; answers how many are
// left
function adopt_reap() {
  // none inherited (always so on Windows, where no exec hands any on)
  if (bp_adopt.inherited.length === 0) return 0;
  const c = bp_adopt_libc();
  const st = new Int32Array(1);
  bp_adopt.inherited = bp_adopt.inherited.filter((pid) => {
    const r = c.waitpid(pid, st, 1);
    return !(r === pid || (r < 0 && c.errno() === 10));
  });
  return bp_adopt.inherited.length;
}

try {
  bp_inherited_scan();
} catch (_) {
}

io_eff(CID(Adopt.record), adopt_record);
io_eff(CID(Adopt.reap), adopt_reap);
io_eff(CID(Adopt.keep), adopt_keep);
io_eff(CID(Adopt.note), adopt_note);
io_eff(CID(Adopt.drop), adopt_drop);
io_eff(CID(Adopt.ident), adopt_ident);
io_eff(CID(Adopt.take), adopt_take);
io_eff(CID(Adopt.stop), adopt_stop);
