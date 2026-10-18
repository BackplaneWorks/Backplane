// Backplane host effects: the JS twin of host.c.
//
// The server ships as a native binary; this twin exists so `bend server.bend`
// runs for quick checks. Process effects answer ENOSYS here.

function host_libc() {
  if (globalThis.BACKPLANE_LIBC === undefined) {
    const ffi = require("bun:ffi");
    const mac = process.platform === "darwin";
    globalThis.BACKPLANE_LIBC = ffi.dlopen(mac ? "libSystem.dylib" : "libc.so.6", {
      dup: { args: ["i32"], returns: "i32" },
      shutdown: { args: ["i32", "i32"], returns: "i32" },
      close: { args: ["i32"], returns: "i32" },
    }).symbols;
  }
  return globalThis.BACKPLANE_LIBC;
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

function sock_recv_bytes(socket, max, k) {
  const sys = io_sys();
  const b = new Uint8Array(Math.max(Number(max), 1));
  const again = sys.mac ? 35 : 11;
  const go = () => {
    const n = Number(sys.recv(socket, sys.ptr(b), b.length, 0));
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
      const got = Number(sys.recv(socket, sys.ptr(b), Math.min(left, b.length), 0));
      if (got < 0 && sys.errno() === again) {
        io_park_on(socket, false, k, go);
        return undefined;
      }
      if (got <= 0) {
        fs.closeSync(out);
        return io_tup(socket, io_fail(got === 0 ? 32 : sys.errno()));
      }
      fs.writeSync(out, b.subarray(0, got));
      left -= got;
    }
    fs.closeSync(out);
    return io_tup(socket, io_done({ $: "Unit" }));
  };
  if (left <= 0) return go();
  io_park_on(socket, false, k, go);
  return undefined;
}

function host_send(socket, b) {
  const sys = io_sys();
  let at = 0;
  while (at < b.length) {
    const n = Number(sys.send(socket, sys.ptr(b.subarray(at)), BigInt(b.length - at), 0x4000));
    if (n < 0) {
      const code = sys.errno();
      if (code === (sys.mac ? 35 : 11)) {
        continue;
      }
      return io_tup(socket, io_fail(code));
    }
    at += n;
  }
  return io_tup(socket, io_done({ $: "Unit" }));
}

function sock_send_bytes(socket, data) {
  const bytes = [];
  for (let xs = data; xs.$ === "Con"; xs = xs.tail) {
    bytes.push(xs.head);
  }
  if (bytes.some((x) => x > 255)) {
    return io_tup(socket, io_fail(22));
  }
  return host_send(socket, Uint8Array.from(bytes));
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
      const n = Number(sys.send(socket, sys.ptr(b.subarray(at)), BigInt(b.length - at), 0x4000));
      if (n < 0) {
        const code = sys.errno();
        if (code === again && performance.now() < until) {
          io_park_on(socket, true, k, go, until);
          return undefined;
        }
        host_libc().shutdown(socket, 2);
        return io_tup(socket, io_fail(code === again ? 110 : code));
      }
      at += n;
      until = performance.now() + span;
    }
    return io_tup(socket, io_done({ $: "Unit" }));
  };
  return go();
}

function sock_send_text(socket, text) {
  return host_send(socket, io_bytes(text));
}

function sock_dup(socket) {
  const got = host_libc().dup(socket);
  return io_tup(socket, got < 0 ? io_fail(io_sys().errno()) : io_done(got));
}

function sock_peer(socket) {
  return io_tup(socket, "");
}

function sock_shutdown(socket) {
  host_libc().shutdown(socket, 1);
  return socket;
}

function proc_spawn(cmd, args, cwd, err) {
  return io_fail(38);
}

function proc_wait(pid) {
  return 255;
}

function proc_errtail(pid) {
  return "";
}

function proc_exited(pid) {
  return 4294967295;
}

function sock_shut_read(socket) {
  host_libc().shutdown(socket, 0);
  return socket;
}

function sock_raw(socket) {
  const got = host_libc().dup(socket);
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

function proc_kill(pid, sig) {
  try {
    process.kill(pid, sig);
  } catch (e) {
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

function net_listen(host, port) {
  return io_fail(38);
}

function env_put(name, value) {
  if (typeof process !== "undefined" && process.env) {
    process.env[name] = value;
  }
  return { $: "Unit" };
}

function sys_exe_dir() {
  return require("path").dirname(process.argv[1] ?? "");
}

function sys_exec(path, args) {
  return io_fail(38);
}

function sys_exe_path() {
  return process.argv[1] ?? "";
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
