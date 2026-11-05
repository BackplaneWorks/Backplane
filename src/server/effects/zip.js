// The Files tab's downloads (src/server/zip.bend): where a path really is,
// and a folder streamed as a ZIP. Every policy (what is left out, what is
// stored, names, the note's words) comes from core/fm.bend as data; this
// file only applies it. test/files_zip_test.bend's driver
// (test/tools/files_zip.mjs) runs it in a scope of its own, as Bend loads
// it, checks its rules against Fm.zip.* and makes real zips with it.

function zip_list(xs) {
  const out = [];
  for (; xs && xs.$ === 'Con'; xs = xs.tail) out.push(xs.head);
  return out;
}

function zip_bend_list(xs) {
  let out = { $: 'Nil' };
  for (let i = xs.length - 1; i >= 0; i--) out = { $: 'Con', head: xs[i], tail: out };
  return out;
}

// Fm.zip.skip over data: "=" a name, "*" the end of a name, "^" its start,
// "/" a path from the root. A rule of another kind is an error, never a
// rule that matches nothing.
function zip_skip(rules, p) {
  p = p.toLowerCase();
  const name = p.slice(p.lastIndexOf('/') + 1);
  for (const r of rules) {
    const kind = r[0], body = r.slice(1);
    if (kind === '=') { if (name === body) return true; }
    else if (kind === '*') { if (name.endsWith(body)) return true; }
    else if (kind === '^') { if (name.startsWith(body)) return true; }
    else if (kind === '/') { if (p === body) return true; }
    else throw Error('unknown zip rule: ' + JSON.stringify(r));
  }
  return false;
}

// Fm.zip.within: the path, or a folder it is in, is left out
function zip_within(rules, p) {
  if (p === '') return false;
  const parts = p.split('/');
  for (let i = 1; i <= parts.length; i++) if (zip_skip(rules, parts.slice(0, i).join('/'))) return true;
  return false;
}

// Fm.zip.entry: each character of swap in a name made "_"
function zip_entry(swap, name) {
  let out = '';
  for (const c of name) out += swap.includes(c) ? '_' : c;
  return out;
}

// Fm.zip.store: a name ending (lower case) in one of stored
function zip_store(stored, name) {
  const lower = name.toLowerCase();
  return stored.some(s => lower.endsWith(s));
}

// All IO is anchored to descriptors. POSIX opens each component with
// O_NOFOLLOW (openat on macOS, /proc/self/fd on Linux). Checking only a
// realpath before open is insufficient: an ancestor can change meanwhile.
function zip_below(pathm, root, at) {
  const r = pathm.relative(root, at);
  return r === '' || (r !== '..' && !r.startsWith('..' + pathm.sep) && !pathm.isAbsolute(r));
}
function zip_protected(fs, pathm, root, at, home) {
  const real = p => { try { return fs.realpathSync(p); } catch { return pathm.resolve(p); } };
  const user = real(require('os').homedir());
  if (zip_below(pathm, root, user)) return true; // home itself, or its ancestors
  const protectedPaths = [home, ...['.ssh', '.aws', '.gnupg', '.config', '.codex', '.claude'].map(n => pathm.join(user, n))];
  if (process.platform !== 'win32') protectedPaths.push('/etc', '/proc', '/sys', '/dev');
  return protectedPaths.filter(Boolean).some(p => zip_below(pathm, real(p), at));
}
function zip_flags(fs, dir) {
  const C = fs.constants;
  return C.O_RDONLY | (C.O_NONBLOCK || 0) | (C.O_NOFOLLOW || 0) | (dir ? C.O_DIRECTORY : 0);
}
function zip_child(fs, parent, name, dir, abs) {
  if (!name || name === '.' || name === '..' || /[\/\\\0]/.test(name)) throw Error('invalid component');
  if (process.platform === 'linux') return fs.openSync('/proc/self/fd/' + parent + '/' + name, zip_flags(fs, dir));
  if (process.platform === 'darwin') {
    const ffi = require('bun:ffi'), vari = process.arch === 'arm64';
    const lib = globalThis.BACKPLANE_ZIP_OPENAT ??= ffi.dlopen('/usr/lib/libSystem.B.dylib', {
      openat: { args: vari ? ['i32','ptr','i32','i64','i64','i64','i64','i64','i32'] : ['i32','ptr','i32','i32'], returns: 'i32' }
    });
    const n = Buffer.from(name + '\0'), flags = zip_flags(fs, dir) | 0x1000000;
    const fd = vari ? lib.symbols.openat(parent, ffi.ptr(n), flags, 0,0,0,0,0,0) : lib.symbols.openat(parent, ffi.ptr(n), flags, 0);
    if (fd < 0) throw Object.assign(Error('cannot open component'), { code: 'ENOENT' });
    return fd;
  }
  // Windows' descriptor is verified by its native final path before any
  // bytes are read; a junction swapped into any ancestor cannot escape.
  if (process.platform === 'win32') {
    if (fs.lstatSync(abs).isSymbolicLink()) throw Error('link');
    return fs.openSync(abs, zip_flags(fs, dir));
  }
  throw Error('unsupported filesystem');
}
function zip_fdpath(fs, fd) {
  if (process.platform === 'linux') return fs.realpathSync('/proc/self/fd/' + fd);
  const ffi = require('bun:ffi');
  if (process.platform === 'darwin') {
    const lib = globalThis.BACKPLANE_ZIP_FCNTL ??= ffi.dlopen('/usr/lib/libSystem.B.dylib', {
      // fcntl is variadic: the third argument is on the stack on arm64.
      fcntl: { args: process.arch === 'arm64' ? ['i32','i32','i64','i64','i64','i64','i64','i64','ptr'] : ['i32','i32','ptr'], returns: 'i32' }
    });
    const b = Buffer.alloc(1024), ptr = ffi.ptr(b);
    const rc = process.arch === 'arm64' ? lib.symbols.fcntl(fd, 50, 0,0,0,0,0,0,ptr) : lib.symbols.fcntl(fd, 50, ptr);
    if (rc < 0) throw Error('cannot resolve descriptor');
    return b.subarray(0, b.indexOf(0)).toString();
  }
  const crt = globalThis.BACKPLANE_ZIP_CRT ??= ffi.dlopen('msvcrt.dll', { _get_osfhandle: { args: ['i32'], returns: 'i64' } });
  const lib = globalThis.BACKPLANE_ZIP_WIN ??= ffi.dlopen('kernel32.dll', { GetFinalPathNameByHandleW: { args: ['ptr','ptr','u32','u32'], returns: 'u32' } });
  const b = Buffer.alloc(65536), handle = Number(crt.symbols._get_osfhandle(fd));
  const n = lib.symbols.GetFinalPathNameByHandleW(handle, ffi.ptr(b), b.length / 2, 0);
  if (!n || n >= b.length / 2) throw Error('cannot resolve descriptor');
  const p = b.subarray(0, n * 2).toString('utf16le');
  return p.startsWith('\\\\?\\UNC\\') ? '\\\\' + p.slice(8) : p.replace(/^\\\\\?\\/, '');
}
function zip_open(fs, pathm, root, rel, home, rules = []) {
  let fd = -1;
  try {
    if (!root || !pathm.isAbsolute(root) || /[\\\0]/.test(rel) || pathm.isAbsolute(rel) || rel.split('/').includes('..')) throw Error('invalid path');
    const at = pathm.resolve(root, rel || '.');
    if (!zip_below(pathm, root, at) || zip_protected(fs, pathm, root, at, home) || zip_within(rules, rel)) throw Error('protected');
    // Open even the root's ancestors without following a link. The caller
    // supplies the canonical root resolved by zip_where, never a URL path.
    const anchor = pathm.parse(root).root;
    fd = fs.openSync(anchor, zip_flags(fs, true));
    const parts = pathm.relative(anchor, at).split(pathm.sep).filter(Boolean);
    let abs = anchor;
    for (let i = 0; i < parts.length; i++) {
      abs = pathm.join(abs, parts[i]);
      const next = zip_child(fs, fd, parts[i], i < parts.length - 1, abs);
      fs.closeSync(fd); fd = next;
    }
    const actual = zip_fdpath(fs, fd), st = fs.fstatSync(fd);
    const r = pathm.relative(root, actual).split(pathm.sep).join('/');
    if (!zip_below(pathm, root, actual) || zip_protected(fs, pathm, root, actual, home) || zip_within(rules, r)
        || (!st.isFile() && !st.isDirectory()) || (st.isFile() && st.nlink !== 1)) throw Error('unsafe file');
    return { fd, st, abs: actual };
  } catch (e) { if (fd >= 0) try { fs.closeSync(fd); } catch {} throw e; }
}
// A bounded directory iterator. macOS's directory stream is duplicated
// from the checked fd, never reopened by its mutable path.
function zip_dir(fs, fd, abs) {
  if (process.platform !== 'darwin') return fs.opendirSync(process.platform === 'linux' ? '/proc/self/fd/' + fd : abs);
  const ffi = require('bun:ffi');
  const lib = globalThis.BACKPLANE_ZIP_DIR ??= ffi.dlopen('/usr/lib/libSystem.B.dylib', {
    dup: { args:['i32'], returns:'i32' }, fdopendir: { args:['i32'], returns:'ptr' },
    readdir: { args:['ptr'], returns:'ptr' }, closedir: { args:['ptr'], returns:'i32' }
  });
  const copy = lib.symbols.dup(fd), dir = lib.symbols.fdopendir(copy);
  if (!dir) { fs.closeSync(copy); throw Error('cannot list directory'); }
  return { readSync() {
    const ptr = lib.symbols.readdir(dir);
    if (!ptr) return null;
    const head = Buffer.from(ffi.toArrayBuffer(ptr, 0, 21));
    const n = head.readUInt16LE(18);
    if (!n || n > 1023) throw Error('invalid directory entry');
    return { name: Buffer.from(ffi.toArrayBuffer(ptr, 21, n)).toString() };
  }, closeSync() { lib.symbols.closedir(dir); } };
}
function zip_limit() { return Object.assign(Error('Folder exceeds download limit (1 GiB or 10,000 entries); download smaller folders.'), { code: 'EFBIG' }); }
function zip_walk(fs, pathm, plan) {
  const stack = [], rules = zip_list(plan.rules);
  let first = true, count = 0, bytes = 0;
  const maxBytes = Math.min(Number(plan.maxBytes ?? 1073741824), 1073741824);
  const maxEntries = Math.min(Number(plan.maxEntries ?? 10000), 10000);
  if (!(maxBytes > 0 && maxEntries > 0)) throw zip_limit();
  const close = () => { while (stack.length) { const t = stack.pop(); try { t.dir.closeSync(); } catch {} try { fs.closeSync(t.fd); } catch {} } };
  const enter = (rel, name, top = false) => {
    let o;
    try { o = zip_open(fs, pathm, plan.root, rel, plan.home, rules); }
    catch (e) { if (top) throw e; return { skipped: true, proj: rel, error: e }; }
    const { fd, st, abs } = o;
    if (st.isDirectory()) {
      if (stack.length >= 64) { fs.closeSync(fd); throw zip_limit(); }
      let dir;
      try { dir = zip_dir(fs, fd, abs); } catch (e) { fs.closeSync(fd); throw e; }
      stack.push({ fd, dir, rel, name });
      return { st, name: name + '/', proj: rel, dir: true };
    }
    bytes += st.size;
    if (!Number.isSafeInteger(st.size) || bytes > maxBytes) { fs.closeSync(fd); throw zip_limit(); }
    return { fd, st, name, proj: rel, dir: false };
  };
  return { close, next() {
    if (first) { first = false; return enter(plan.rel, zip_entry(plan.swap, plan.top), true); }
    for (let i = 0; i < ZIP_STEP; i++) {
      const t = stack.at(-1);
      if (!t) return null;
      const n = t.dir.readSync()?.name;
      if (n === undefined) { stack.pop(); t.dir.closeSync(); fs.closeSync(t.fd); continue; }
      if (n === '.' || n === '..') continue;
      if (++count > maxEntries) throw zip_limit();
      const rel = t.rel ? t.rel + '/' + n : n;
      if (zip_skip(rules, rel)) continue;
      if (n.includes('\\')) return { skipped: true, proj: rel, error: { code: 'backslash' } };
      // A stale iterator may name something since moved, but zip_open
      // independently binds and checks every component of the new path.
      return enter(rel, t.name + '/' + zip_entry(plan.swap, n));
    }
    return { yield: true };
  } };
}

// rel from root, every link resolved, while it stays in root: [the root's
// real path, the path from it with "/", "d" | "f" | "o"]; null when it is
// gone or leaves root
function zip_where(fs, pathm, root, rel, home) {
  try {
    if (typeof root !== 'string' || root === '' || typeof rel !== 'string' || (root + rel).includes('\0')) return null;
    const top = fs.realpathSync(root);
    const at = fs.realpathSync(pathm.resolve(top, rel || '.'));
    const r = pathm.relative(top, at);
    if (r !== '' && (r === '..' || r.startsWith('..' + pathm.sep) || pathm.isAbsolute(r))) return null;
    if (zip_protected(fs, pathm, top, at, home)) return null;
    const st = fs.statSync(at);
    return [top, r.split(pathm.sep).join('/'), st.isDirectory() ? 'd' : st.isFile() ? 'f' : 'o'];
  } catch (_) {
    return null;
  }
}

function zip_listing(root, rel, home) {
  const fs = require('fs'), pathm = require('path');
  let opened, dir;
  try {
    const where = zip_where(fs, pathm, root, rel, home);
    if (!where || where[2] !== 'd') return zip_bend_list([]);
    opened = zip_open(fs, pathm, where[0], where[1], home);
    dir = zip_dir(fs, opened.fd, opened.abs);
    const folders = [], files = [];
    for (let count = 0; count < 10000; count++) {
      const name = dir.readSync()?.name;
      if (name === undefined) break;
      if (name.startsWith('.') || /[\t\r\n\\]/.test(name)) continue;
      let child;
      try {
        const childRel = where[1] ? where[1] + '/' + name : name;
        child = zip_open(fs, pathm, where[0], childRel, home);
        if (child.st.isDirectory() && folders.length < 2000) folders.push({ name, row: 'd\t0\t' + name + '\n' });
        if (child.st.isFile() && files.length < 2000) files.push({ name, row: 'f\t' + child.st.size + '\t' + name + '\n' });
      } catch {} finally { if (child) fs.closeSync(child.fd); }
      if (folders.length === 2000 && files.length === 2000) break;
    }
    const order = (a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0;
    return zip_bend_list([folders.sort(order).concat(files.sort(order)).map(x => x.row).join('')]);
  } catch { return zip_bend_list([]); }
  finally {
    if (dir) try { dir.closeSync(); } catch {}
    if (opened) try { fs.closeSync(opened.fd); } catch {}
  }
}

function zip_inside(root, rel, home) {
  const w = zip_where(require('fs'), require('path'), root, rel, home);
  return zip_bend_list(w === null ? [] : w);
}

let zip_crc_table = null;
function zip_crc32(zlib, b, crc) {
  if (typeof zlib.crc32 === 'function') return zlib.crc32(b, crc);
  if (zip_crc_table === null) {
    zip_crc_table = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let i = 0; i < 8; i++) c = c & 1 ? 0xEDB88320 ^ (c >>> 1) : c >>> 1;
      zip_crc_table[n] = c;
    }
  }
  let c = ~crc;
  for (let i = 0; i < b.length; i++) c = zip_crc_table[(c ^ b[i]) & 255] ^ (c >>> 8);
  return ~c >>> 0;
}

// A streaming ZIP of the confined folder. next() returns one bounded
// piece, an empty buffer to yield, or null at completion. The walker uses
// descriptor-bound opens and bounded directory iterators, never following
// symlinks in a tree. Each file is read in 128 KiB pieces and stored or
// deflated at level 1. UTF-8 names, data descriptors and ZIP64 records are
// supported; the public folder route separately caps source bytes/entries
// before headers and again during the write. Only the bounded central
// directory metadata is retained. close() releases files and iterators on
// success, disconnect, timeout or any exception.
const ZIP_PIECE = 131072;
const ZIP_STEP = 256;
// ZIP_MAX marks a field the zip64 record holds; ZIP_LIMIT and ZIP_COUNT
// are where zip64 starts (lowered by test/tools/files_zip.mjs)
const ZIP_MAX = 0xFFFFFFFF;
const ZIP_LIMIT = 0xFFFFFFFF;
const ZIP_COUNT = 0xFFFF;
function zip_writer(fs, zlib, pathm, plan) {
  const rules = zip_list(plan.rules), stored = zip_list(plan.stored), why = zip_list(plan.why);
  const swap = plan.swap, top = plan.top;
  for (const r of rules) zip_skip([r], '');
  const reason = (code) => {
    for (const w of why) { const i = w.indexOf('='); if (w.slice(0, i) === code) return w.slice(i + 1); }
    for (const w of why) if (w.startsWith('*=')) return w.slice(2);
    return code;
  };
  const walk = zip_walk(fs, pathm, plan);
  const central = [], missed = [], names = new Set();
  const piece = Buffer.allocUnsafe(ZIP_PIECE);
  let offset = 0, file = null, phase = 'walk', at = 0, cdStart = 0, cdSize = 0;
  const dos = (ms) => {
    const d = new Date(ms);
    let y = d.getFullYear();
    if (!(y >= 1980)) return { time: 0, date: (1 << 5) | 1 };
    if (y > 2107) y = 2107;
    return { time: (d.getHours() << 11) | (d.getMinutes() << 5) | (d.getSeconds() >> 1),
      date: ((y - 1980) << 9) | ((d.getMonth() + 1) << 5) | d.getDate() };
  };
  const u64 = (b, n, i) => b.writeBigUInt64LE(BigInt(n), i);
  const local = (e) => {
    const name = Buffer.from(e.name, 'utf8');
    const extra = Buffer.alloc(e.z64 ? 20 : 0);
    if (e.z64) { extra.writeUInt16LE(1, 0); extra.writeUInt16LE(16, 2); }
    const b = Buffer.alloc(30 + name.length + extra.length);
    b.writeUInt32LE(0x04034b50, 0);
    b.writeUInt16LE(e.z64 ? 45 : 20, 4);
    b.writeUInt16LE(e.flags, 6);
    b.writeUInt16LE(e.method, 8);
    b.writeUInt16LE(e.time, 10);
    b.writeUInt16LE(e.date, 12);
    b.writeUInt32LE(e.flags & 8 ? 0 : e.crc >>> 0, 14);
    b.writeUInt32LE(e.z64 ? ZIP_MAX : e.flags & 8 ? 0 : e.csize, 18);
    b.writeUInt32LE(e.z64 ? ZIP_MAX : e.flags & 8 ? 0 : e.usize, 22);
    b.writeUInt16LE(name.length, 26);
    b.writeUInt16LE(extra.length, 28);
    name.copy(b, 30);
    extra.copy(b, 30 + name.length);
    return b;
  };
  const record = (e) => {
    const name = Buffer.from(e.name, 'utf8');
    const big = e.z64 || e.usize >= ZIP_LIMIT || e.csize >= ZIP_LIMIT;
    const far = e.offset >= ZIP_LIMIT;
    const extra = Buffer.alloc(big || far ? 4 + (big ? 16 : 0) + (far ? 8 : 0) : 0);
    if (extra.length) {
      extra.writeUInt16LE(1, 0);
      extra.writeUInt16LE(extra.length - 4, 2);
      let i = 4;
      if (big) { u64(extra, e.usize, i); u64(extra, e.csize, i + 8); i += 16; }
      if (far) u64(extra, e.offset, i);
    }
    const b = Buffer.alloc(46 + name.length + extra.length);
    b.writeUInt32LE(0x02014b50, 0);
    b.writeUInt16LE((3 << 8) | 45, 4);
    b.writeUInt16LE(big || far ? 45 : 20, 6);
    b.writeUInt16LE(e.flags, 8);
    b.writeUInt16LE(e.method, 10);
    b.writeUInt16LE(e.time, 12);
    b.writeUInt16LE(e.date, 14);
    b.writeUInt32LE(e.crc >>> 0, 16);
    b.writeUInt32LE(big ? ZIP_MAX : e.csize, 20);
    b.writeUInt32LE(big ? ZIP_MAX : e.usize, 24);
    b.writeUInt16LE(name.length, 28);
    b.writeUInt16LE(extra.length, 30);
    b.writeUInt32LE((((e.mode & 0xFFFF) << 16) | (e.dir ? 0x10 : 0)) >>> 0, 38);
    b.writeUInt32LE(far ? ZIP_MAX : e.offset, 42);
    name.copy(b, 46);
    extra.copy(b, 46 + name.length);
    return b;
  };
  const out = (b) => { offset += b.length; return b; };
  const miss = (t, e) => { missed.push((t.proj || '.') + (t.dir ? '/' : '') + ' (' + reason(e && e.code || 'short') + ')'); };
  const visit = (t) => {
    if (t.yield) return null;
    if (t.skipped) {
      if (t.error?.code === 'backslash') miss(t, t.error);
      return null;
    }
    const st = t.st;
    names.add(t.name);
    if (t.dir) {
      const e = { name: t.name, dir: true, z64: false, flags: 0x0800, method: 0, ...dos(st.mtimeMs),
        crc: 0, csize: 0, usize: 0, offset, mode: 0o040755 };
      central.push(e);
      return out(local(e));
    }
    const size = st.size, store = zip_store(stored, t.name);
    const e = { name: t.name, dir: false, z64: size + Math.floor(size / 1000) + 65536 >= ZIP_LIMIT,
      flags: 0x0808, method: store ? 0 : 8, ...dos(st.mtimeMs), crc: 0, csize: 0, usize: 0, offset, mode: 0o100644 };
    file = { fd: t.fd, e, size, t };
    return out(local(e));
  };
  // the next piece of the open file, or its end: the descriptor
  const step = () => {
    const { fd, e, size, t } = file;
    const want = Math.min(ZIP_PIECE, size - e.usize);
    let got = 0;
    if (want > 0) {
      try { got = fs.readSync(fd, piece, 0, want, e.usize); } catch (err) { miss(t, err); got = 0; }
      if (got === 0) miss(t, { code: 'short' });
    }
    if (got > 0) {
      const raw = piece.subarray(0, got);
      e.crc = zip_crc32(zlib, raw, e.crc);
      e.usize += got;
      const z = e.method === 0 ? Buffer.from(raw) : zlib.deflateRawSync(raw, { level: 1, finishFlush: zlib.constants.Z_SYNC_FLUSH });
      e.csize += z.length;
      return out(z);
    }
    file = null;
    try { fs.closeSync(fd); } catch (_) {}
    const end = e.method === 0 ? Buffer.alloc(0) : Buffer.from([3, 0]);
    e.csize += end.length;
    const d = Buffer.alloc(end.length + (e.z64 ? 24 : 16));
    end.copy(d, 0);
    const i = end.length;
    d.writeUInt32LE(0x08074b50, i);
    d.writeUInt32LE(e.crc >>> 0, i + 4);
    if (e.z64) { u64(d, e.csize, i + 8); u64(d, e.usize, i + 16); }
    else { d.writeUInt32LE(e.csize, i + 8); d.writeUInt32LE(e.usize, i + 12); }
    central.push(e);
    return out(d);
  };
  // the note of what could not be put in, a stored entry at the top
  const note = () => {
    let name = zip_entry(swap, top) + '/' + plan.note;
    while (names.has(name)) name = zip_entry(swap, top) + '/_' + name.slice(name.lastIndexOf('/') + 1);
    const body = Buffer.from(plan.head + missed.map(m => m + '\n').join(''), 'utf8');
    const e = { name, dir: false, z64: false, flags: 0x0800, method: 0, ...dos(Date.now()),
      crc: zip_crc32(zlib, body, 0), csize: body.length, usize: body.length, offset, mode: 0o100644 };
    central.push(e);
    return out(Buffer.concat([local(e), body]));
  };
  // the central directory, ZIP_STEP records a step, then the end records
  const directory = () => {
    if (at === 0) cdStart = offset;
    if (at < central.length) {
      const parts = central.slice(at, at + ZIP_STEP).map(record);
      at += parts.length;
      const b = Buffer.concat(parts);
      cdSize += b.length;
      return out(b);
    }
    const count = central.length, z64 = count >= ZIP_COUNT || cdStart >= ZIP_LIMIT || cdSize >= ZIP_LIMIT;
    const parts = [];
    if (z64) {
      const r = Buffer.alloc(56 + 20);
      r.writeUInt32LE(0x06064b50, 0);
      u64(r, 44, 4);
      r.writeUInt16LE((3 << 8) | 45, 12);
      r.writeUInt16LE(45, 14);
      u64(r, count, 24);
      u64(r, count, 32);
      u64(r, cdSize, 40);
      u64(r, cdStart, 48);
      r.writeUInt32LE(0x07064b50, 56);
      u64(r, cdStart + cdSize, 64);
      r.writeUInt32LE(1, 72);
      parts.push(r);
    }
    const e = Buffer.alloc(22);
    e.writeUInt32LE(0x06054b50, 0);
    e.writeUInt16LE(z64 ? 0xFFFF : count, 8);
    e.writeUInt16LE(z64 ? 0xFFFF : count, 10);
    e.writeUInt32LE(z64 ? ZIP_MAX : cdSize, 12);
    e.writeUInt32LE(z64 ? ZIP_MAX : cdStart, 16);
    parts.push(e);
    phase = 'done';
    return out(Buffer.concat(parts));
  };
  return {
    next() {
      if (phase === 'done') return null;
      if (file !== null) return step();
      if (phase === 'walk') {
        const t = walk.next();
        if (t !== null) return visit(t) ?? Buffer.alloc(0);
        phase = 'note';
      }
      if (phase === 'note') {
        phase = 'directory';
        if (missed.length) return note();
      }
      return directory();
    },
    close() {
      walk.close();
      if (file !== null) { const fd = file.fd; file = null; try { fs.closeSync(fd); } catch (_) {} }
    },
    missed,
  };
}

function zip_text(status, text) {
  const body = Buffer.from(text);
  return Buffer.concat([Buffer.from('HTTP/1.1 ' + status + '\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: '
    + body.length + '\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n'), body]);
}

function zip_errno(e) {
  return Math.abs(e?.errno ?? 5);
}

// first, then what more() answers until null (an empty buffer: nothing to
// write this time), written as the socket takes it: a full one parks, a
// stall of 30 s fails, and the loop runs between pieces; done() runs once
// at the end, however it ends. Works on a Bun-loop handle (loop.js's BP_H)
// and on a descriptor alike.
function zip_stream(socket, k, first, more, done) {
  const handles = globalThis.BP_H;
  const handle = handles?.is(socket);
  const sys = handle ? null : io_sys();
  const again = handle ? (handles.EAGAIN ?? 11) : (sys.mac ? 35 : 11);
  const timeout = process.platform === 'darwin' ? 60 : 110;
  let pending = first, at = 0, until = performance.now() + 30000;
  const deadline = performance.now() + 300000;
  const finish = (code) => {
    try { done?.(); } catch (_) {}
    pending = null;
    return io_tup(socket, code ? io_fail(code) : io_done({ $: 'Unit' }));
  };
  const write = (b) => handle
    ? (handles.get(socket)?.write(b) ?? -9)
    : Number(sys.send(socket, sys.ptr(b), BigInt(b.length), sys.mac ? 0 : 0x4000));
  const go = () => {
    try {
      if (performance.now() >= deadline || performance.now() >= until) return finish(timeout);
      // Probe the transport even during preflight/empty steps, so cancel
      // stops walking immediately instead of waiting for the first header.
      if (handle && (!handles.get(socket) || handles.get(socket).gone || handles.get(socket).err)) return finish(9);
      while (at < pending.length) {
        const n = write(pending.subarray(at));
        const code = n < 0 ? (handle ? -n : sys.errno()) : 0;
        if (n === 0 || code === again) {
          io_park_on(socket, true, k, go, Math.min(until, performance.now() + 1000));
          return undefined;
        }
        if (code) return finish(code);
        at += n;
        until = performance.now() + 30000;
      }
      const next = more ? more() : null;
      if (next === null) return finish(0);
      pending = next;
      at = 0;
      if (next.length === 0) until = performance.now() + 30000;
      io_park_on(undefined, false, k, go, performance.now());
      return undefined;
    } catch (e) { return finish(zip_errno(e)); }
  };
  return go();
}

function zip_send(socket, plan, k) {
  const fs = require('fs'), zlib = require('zlib'), pathm = require('path');
  const answer = (status, text) => zip_stream(socket, k, zip_text(status, text), null);
  let scan;
  try {
    if (!plan || typeof plan.root !== 'string' || typeof plan.rel !== 'string' || typeof plan.home !== 'string'
        || typeof plan.top !== 'string' || !plan.top || /[\/\\\0]/.test(plan.top) || /[\0\r\n]/.test(plan.disposition)
        || !plan.note || /[\/\\\0]/.test(plan.note)) return answer('400 Bad Request', 'invalid download');
    const w = zip_where(fs, pathm, plan.root, plan.rel, plan.home);
    if (w === null || w[2] !== 'd' || w[0] !== plan.root || w[1] !== plan.rel) return answer('404 Not Found', 'no such folder');
    scan = zip_walk(fs, pathm, plan);
  } catch { return answer('404 Not Found', 'no such folder'); }
  const head = Buffer.from('HTTP/1.1 200 OK\r\nContent-Type: application/zip\r\nContent-Disposition: ' + plan.disposition
    + '\r\nTransfer-Encoding: chunked\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n');
  let zip = null, last = false;
  const more = () => {
    if (last) return null;
    if (!zip) {
      try {
        for (let i = 0; i < ZIP_STEP; i++) {
          const t = scan.next();
          if (t === null) { scan.close(); zip = zip_writer(fs, zlib, pathm, plan); return head; }
          if (t.fd !== undefined) fs.closeSync(t.fd);
        }
        return Buffer.alloc(0);
      } catch (e) {
        scan.close(); last = true;
        return zip_text(e.code === 'EFBIG' ? '413 Content Too Large' : '404 Not Found', e.code === 'EFBIG' ? e.message : 'cannot read folder');
      }
    }
    const b = zip.next();
    if (b === null) { last = true; return Buffer.from('0\r\n\r\n'); }
    if (b.length === 0) return b;
    return Buffer.concat([Buffer.from(b.length.toString(16) + '\r\n'), b, Buffer.from('\r\n')]);
  };
  return zip_stream(socket, k, Buffer.alloc(0), more, () => { scan.close(); zip?.close(); });
}

function zip_file(socket, root, rel, home, disposition, range, rules, k) {
  const fs = require('fs'), pathm = require('path');
  let opened;
  try {
    if (/[\0\r\n]/.test(disposition)) throw Error('invalid name');
    opened = zip_open(fs, pathm, root, rel, home, zip_list(rules));
    if (!opened.st.isFile() || !Number.isSafeInteger(opened.st.size)) throw Error('not a file');
  } catch { if (opened) fs.closeSync(opened.fd); return zip_stream(socket, k, zip_text('404 Not Found', 'not found'), null); }
  const { fd, st } = opened;
  let pos = 0, end = st.size - 1, status = '200 OK', extra = '';
  const m = /^bytes=(\d*)-(\d*)$/.exec(range);
  if (m && (m[1] || m[2])) {
    const size = BigInt(st.size), first = m[1] ? BigInt(m[1]) : null, last = m[2] ? BigInt(m[2]) : null;
    if (size === 0n || (first === null ? last === 0n : first >= size || (last !== null && last < first))) {
      fs.closeSync(fd);
      return zip_stream(socket, k, zip_text('416 Range Not Satisfiable', 'range not satisfiable'), null);
    }
    pos = Number(first === null ? (last >= size ? 0n : size - last) : first);
    end = first === null || last === null || last >= size ? st.size - 1 : Number(last);
    status = '206 Partial Content'; extra = 'Content-Range: bytes ' + pos + '-' + end + '/' + st.size + '\r\n';
  }
  const head = Buffer.from('HTTP/1.1 ' + status + '\r\nContent-Type: application/octet-stream\r\nContent-Disposition: '
    + disposition + '\r\nContent-Length: ' + (end - pos + 1) + '\r\nAccept-Ranges: bytes\r\nCache-Control: no-store\r\n'
    + 'X-Content-Type-Options: nosniff\r\nConnection: close\r\n' + extra + '\r\n');
  const piece = Buffer.allocUnsafe(ZIP_PIECE);
  return zip_stream(socket, k, head, () => {
    if (pos > end) return null;
    const n = fs.readSync(fd, piece, 0, Math.min(piece.length, end - pos + 1), pos);
    if (!n) throw Error('file truncated');
    pos += n; return piece.subarray(0, n);
  }, () => fs.closeSync(fd));
}

// Test access to the same transport and writer used by the hub.
globalThis.BP_ZIP = { zip_skip, zip_within, zip_entry, zip_store, zip_where, zip_open, zip_walk, zip_writer, zip_stream, zip_send, zip_file };
io_eff(CID(Zip.inside), zip_inside);
io_eff(CID(Zip.send), zip_send);
io_eff(CID(Zip.file), zip_file);

io_eff(CID(Zip.list), zip_listing);
