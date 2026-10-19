// The JS twin of hublock.c: the same flock through bun:ffi where Bun runs
// it (the JS host never execs, so nothing is handed over). Imports are lazy
// so a web or phone bundle can include it without touching IO.
let bp_hub_fd = -1;
let bp_hub_path = "";
let bp_hub_ffi;
function bp_hub_flock() {
  if (!bp_hub_ffi) {
    const { dlopen } = require("bun:ffi");
    bp_hub_ffi = dlopen(process.platform === "darwin" ? "/usr/lib/libSystem.B.dylib" : "libc.so.6", {
      flock: { args: ["i32", "i32"], returns: "i32" },
    });
  }
  return bp_hub_ffi.symbols.flock;
}
function bp_hub_read(path) {
  try {
    return require("node:fs").readFileSync(path, "utf8").split("\n")[0].trim();
  } catch {
    return "";
  }
}
function hub_lock_take(path, port) {
  const line = `${process.pid} ${port} flock`;
  const fs = require("node:fs");
  try {
    if (bp_hub_fd < 0 || bp_hub_path !== path) {
      const fd = fs.openSync(path, "a+", 0o644);
      // LOCK_EX | LOCK_NB
      if (bp_hub_flock()(fd, 2 | 4) !== 0) {
        fs.closeSync(fd);
        return "held " + bp_hub_read(path);
      }
      if (bp_hub_fd >= 0) fs.closeSync(bp_hub_fd);
      bp_hub_fd = fd;
      bp_hub_path = path;
    }
    fs.ftruncateSync(bp_hub_fd, 0);
    fs.writeSync(bp_hub_fd, line + "\n", 0);
    return "ok";
  } catch (e) {
    return "fail " + path + ": " + String(e && e.message ? e.message : e);
  }
}
function hub_lock_held(path) {
  if (bp_hub_fd >= 0 && bp_hub_path === path) return "";
  const fs = require("node:fs");
  let fd;
  try {
    fd = fs.openSync(path, "r");
  } catch {
    return "";
  }
  try {
    // LOCK_SH | LOCK_NB, then LOCK_UN
    if (bp_hub_flock()(fd, 1 | 4) === 0) {
      bp_hub_flock()(fd, 8);
      return "";
    }
    return bp_hub_read(path) || "? ?";
  } catch {
    return "";
  } finally {
    fs.closeSync(fd);
  }
}
io_eff(CID(HubLock.take), hub_lock_take);
io_eff(CID(HubLock.held), hub_lock_held);
