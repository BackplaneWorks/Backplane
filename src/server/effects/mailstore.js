// Twins of the low-level file operations; no journal parsing or policy here.
function mi_open(path, write) {
  const fs = require('fs'), C = fs.constants;
  let dir = -1, fd = -1, code = 0;
  try {
    if (path.includes('\0')) throw { errno: 22 };
    const at = path.lastIndexOf('/');
    if (at < 1 || at === path.length - 1) throw { errno: 22 };
    dir = fs.openSync(path.slice(0, at), C.O_RDONLY | C.O_DIRECTORY | C.O_NOFOLLOW);
    // openat binds the final component to the checked directory descriptor.
    const ffi = require('bun:ffi');
    const lib = ffi.dlopen(process.platform === 'darwin' ? '/usr/lib/libSystem.B.dylib' : 'libc.so.6', {
      openat: { args: ['i32', 'ptr', 'i32', 'i32'], returns: 'i32' }
    });
    const name = Buffer.from(path.slice(at + 1) + '\0');
    fd = lib.symbols.openat(dir, ffi.ptr(name), (write !== 0 ? C.O_RDWR | C.O_CREAT : C.O_RDONLY) | C.O_NOFOLLOW | C.O_NONBLOCK, 0o600);
    if (fd < 0) code = io_sys().errno();
    lib.close();
    if (!code) { const st = fs.fstatSync(fd); if (!st.isFile() || (write !== 0 && st.nlink !== 1)) code = 22; }
  } catch (e) { code = Math.abs(e.errno ?? 5); }
  if (dir >= 0) { try { fs.closeSync(dir); } catch (e) { code ||= Math.abs(e.errno ?? 5); } }
  if (code) { if (fd >= 0) try { fs.closeSync(fd); } catch {} throw { errno: code }; }
  return fd;
}
function mailfile_open(path, write) {
  try { return io_done(mi_open(path, write)); }
  catch (e) { return io_fail(Math.abs(e.errno ?? 5)); }
}
function mailfile_finish(file, cut, line) {
  const fs = require('fs'); let code = 0;
  try {
    const data = Buffer.from(line, 'utf8');
    if (cut + data.length > 0xffffffff) throw { errno: 27 };
    fs.ftruncateSync(file, cut);
    let at = 0;
    while (at < data.length) {
      let n;
      try { n = fs.writeSync(file, data, at, data.length - at, cut + at); }
      catch (e) { if (e.code === 'EINTR') continue; throw e; }
      if (!n) { code = 5; break; }
      at += n;
    }
  } catch (e) { code = Math.abs(e.errno ?? 5); }
  try { fs.closeSync(file); } catch (e) { code ||= Math.abs(e.errno ?? 5); }
  return code ? io_fail(code) : io_done({ $: CID(Unit) });
}
io_eff(CID(MailFile.open), mailfile_open);
io_eff(CID(MailFile.finish), mailfile_finish);

// Safe positional IO, sharing open's regular-file and link checks.
function mi_text(path, at, size) {
  const fs = require('fs');
  let fd;
  try { fd = mi_open(path, 0); } catch (e) { return io_fail(Math.abs(e.errno ?? 5)); }
  let code = 0, text = '';
  try {
    if (fs.fstatSync(fd).size > 0xffffffff || at + size > 0xffffffff) throw { errno: 27 };
    const data = Buffer.alloc(size); let n = 0;
    while (n < size) {
      let got;
      try { got = fs.readSync(fd, data, n, size - n, at + n); }
      catch (e) { if (e.code === 'EINTR') continue; throw e; }
      if (!got) throw { errno: 5 };
      n += got;
    }
    text = data.toString('utf8');
  } catch (e) { code = Math.abs(e.errno ?? 5); }
  try { fs.closeSync(fd); } catch (e) { code ||= Math.abs(e.errno ?? 5); }
  return code ? io_fail(code) : io_done(text);
}
function mi_patch(path, at, text, truncate) {
  const fs = require('fs');
  let fd;
  try { fd = mi_open(path, 1); } catch (e) { return io_fail(Math.abs(e.errno ?? 5)); }
  let code = 0;
  try {
    const data = Buffer.from(text, 'utf8');
    if (fs.fstatSync(fd).size > 0xffffffff || at + data.length > 0xffffffff) throw { errno: 27 };
    let n = 0;
    while (n < data.length) {
      let got;
      try { got = fs.writeSync(fd, data, n, data.length - n, at + n); }
      catch (e) { if (e.code === 'EINTR') continue; throw e; }
      if (!got) throw { errno: 5 };
      n += got;
    }
    if (truncate) fs.ftruncateSync(fd, at + data.length);
  } catch (e) { code = Math.abs(e.errno ?? 5); }
  try { fs.closeSync(fd); } catch (e) { code ||= Math.abs(e.errno ?? 5); }
  return code ? io_fail(code) : io_done({ $: CID(Unit) });
}
io_eff(CID(MailFile.text), mi_text);
io_eff(CID(MailFile.patch), mi_patch);
function mi_texts(path, ranges) {
  const fs = require('fs'); let fd;
  try { fd = mi_open(path, 0); } catch (e) { return io_fail(Math.abs(e.errno ?? 5)); }
  let code = 0; const texts = [];
  try {
    if (fs.fstatSync(fd).size > 0xffffffff) throw { errno: 27 };
    for (let xs = ranges; xs.$ === 'Con';) {
      const at = xs.head; xs = xs.tail;
      if (xs.$ !== 'Con') throw { errno: 22 };
      const size = xs.head; xs = xs.tail;
      if (at + size > 0xffffffff) throw { errno: 27 };
      const data = Buffer.alloc(size); let n = 0;
      while (n < size) {
        let got;
        try { got = fs.readSync(fd, data, n, size - n, at + n); }
        catch (e) { if (e.code === 'EINTR') continue; throw e; }
        if (!got) throw { errno: 5 };
        n += got;
      }
      texts.push(data.toString('utf8'));
    }
  } catch (e) { code = Math.abs(e.errno ?? 5); }
  try { fs.closeSync(fd); } catch (e) { code ||= Math.abs(e.errno ?? 5); }
  if (code) return io_fail(code);
  let out = { $: CID(Nil) };
  for (let i = texts.length - 1; i >= 0; i--) out = { $: CID(Con), head: texts[i], tail: out };
  return io_done(out);
}
io_eff(CID(MailFile.texts), mi_texts);
