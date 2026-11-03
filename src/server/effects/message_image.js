// A reference is authorized in Bend with the same parser used by the clients.
// This boundary reads only that indexed assistant text and retains raster bytes.
const MI_BYTES = 24 * 1024 * 1024, MI_PIXELS = 32 * 1024 * 1024;
function mi_modules() { return {fs: require('node:fs'), path: require('node:path'), crypto: require('node:crypto')}; }
function mi_hash(value) { return mi_modules().crypto.createHash('sha256').update(value).digest('hex'); }
function mi_home() {
  const home = process.env.BACKPLANE_HOME;
  if (!home) throw Error('No hub home');
  return home;
}
function mi_text(entry) {
  if (typeof entry !== 'string' || !entry || entry.length > 1024) return '';
  let db;
  try {
    const {Database} = require('bun:sqlite');
    db = new Database(mi_modules().path.join(mi_home(), 'store.db'), {readonly: true});
    return db.query("SELECT e.text FROM entry e JOIN thread t ON t.id=e.thread WHERE e.id=? AND e.kind='msg' AND e.role='assistant' AND t.deleted=0 LIMIT 1").get(entry)?.text || '';
  } catch { return ''; }
  finally { db?.close(); }
}
// Only raster signatures; reject SVG/HTML renamed to .png. Read dimensions
// before a decoder allocates memory (24 MiB encoded, 32 megapixels decoded).
function mi_type(b) {
  let ext, w, h;
  if (b.length >= 24 && b.subarray(0, 8).equals(Buffer.from('89504e470d0a1a0a', 'hex')) && b.toString('ascii', 12, 16) === 'IHDR') {
    ext = 'png';w = b.readUInt32BE(16);h = b.readUInt32BE(20);
  } else if (b.length >= 10 && /^GIF8[79]a$/.test(b.toString('ascii', 0, 6))) {
    ext = 'gif';w = b.readUInt16LE(6);h = b.readUInt16LE(8);
  } else if (b.length >= 26 && b.toString('ascii', 0, 2) === 'BM' && b.readUInt32LE(14) >= 40) {
    ext = 'bmp';w = b.readInt32LE(18);h = Math.abs(b.readInt32LE(22));
  } else if (b.length >= 30 && b.toString('ascii', 0, 4) === 'RIFF' && b.toString('ascii', 8, 12) === 'WEBP') {
    ext = 'webp';const kind = b.toString('ascii', 12, 16);
    if (kind === 'VP8X') { w = b.readUIntLE(24, 3) + 1;h = b.readUIntLE(27, 3) + 1; }
    else if (kind === 'VP8L' && b[20] === 0x2f) { const n = b.readUInt32LE(21);w = (n & 0x3fff) + 1;h = ((n >>> 14) & 0x3fff) + 1; }
    else if (kind === 'VP8 ' && b.subarray(23, 26).equals(Buffer.from([0x9d, 1, 0x2a]))) { w = b.readUInt16LE(26) & 0x3fff;h = b.readUInt16LE(28) & 0x3fff; }
  } else if (b.length >= 4 && b[0] === 0xff && b[1] === 0xd8) {
    ext = 'jpg';let i = 2;
    while (i + 4 <= b.length) {
      if (b[i++] !== 0xff) break;
      while (b[i] === 0xff) i++;
      const marker = b[i++];
      if (marker === 0xda || marker === 0xd9) break;
      if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) continue;
      if (i + 2 > b.length) break;
      const len = b.readUInt16BE(i);
      if (len < 2 || i + len > b.length) break;
      if ([0xc0,0xc1,0xc2,0xc3,0xc5,0xc6,0xc7,0xc9,0xca,0xcb,0xcd,0xce,0xcf].includes(marker) && len >= 8) {
        h = b.readUInt16BE(i + 3);w = b.readUInt16BE(i + 5);break;
      }
      i += len;
    }
  }
  if (!ext || !(w > 0 && h > 0 && w * h <= MI_PIXELS)) throw Error('Unsupported or oversized image');
  return ext;
}
function mi_regular(file) {
  const {fs} = mi_modules(), st = fs.lstatSync(file);
  if (!st.isFile() || st.isSymbolicLink() || !st.size || st.size > MI_BYTES) throw Error('Image unavailable');
  return st;
}
function mi_read(file) {
  const {fs} = mi_modules();
  const before = mi_regular(file);
  let fd;
  try {
    fd = fs.openSync(file, fs.constants.O_RDONLY | (fs.constants.O_NONBLOCK || 0) | (fs.constants.O_NOFOLLOW || 0));
    const st = fs.fstatSync(fd);
    if (!st.isFile() || st.dev !== before.dev || st.ino !== before.ino || st.size !== before.size) throw Error('Image changed');
    const b = Buffer.alloc(st.size);
    let n = 0;
    while (n < b.length) { const got = fs.readSync(fd, b, n, b.length - n, n);if (!got) throw Error('Image truncated');n += got; }
    const after = fs.fstatSync(fd);
    if (after.size !== st.size || after.mtimeMs !== st.mtimeMs || after.ctimeMs !== st.ctimeMs) throw Error('Image changed');
    return b;
  } finally { if (fd !== undefined) fs.closeSync(fd); }
}
function mi_dir() {
  const {fs, path} = mi_modules(), dir = path.join(mi_home(), 'message-images');
  fs.mkdirSync(dir, {recursive: true, mode: 0o700});
  const st = fs.lstatSync(dir);
  if (!st.isDirectory() || st.isSymbolicLink() || (process.platform !== 'win32' && ((st.mode & 0o077) || st.uid !== process.getuid()))) throw Error('Private image directory unavailable');
  return dir;
}
function mi_source(source) {
  const {fs, path} = mi_modules();
  if (typeof source !== 'string' || source.length > 1024 || /[\0\r\n\\]/.test(source) || source.split('/').some(x => x === '.' || x === '..')) throw Error('Invalid image path');
  const file = source.startsWith('~/') ? path.join(require('node:os').homedir(), source.slice(2)) : source;
  if (!path.isAbsolute(file) || !/\.(png|jpe?g|gif|webp|bmp)$/i.test(file)) throw Error('Invalid image path');
  if (fs.realpathSync(file) !== path.resolve(file)) throw Error('Image symlink');
  return file;
}
function mi_snapshot(entry, source) {
  const {fs, path} = mi_modules();
  try {
    if (!entry || entry.length > 1024) return '';
    const dir = mi_dir(), ref = path.join(dir, mi_hash(entry + '\0' + source) + '.json');
    if (fs.existsSync(ref)) {
      const st = fs.lstatSync(ref);
      if (!st.isFile() || st.isSymbolicLink() || st.size > 256) return '';
      const name = JSON.parse(fs.readFileSync(ref, 'utf8')).image;
      if (!/^[a-f0-9]{64}\.(png|jpg|gif|webp|bmp)$/.test(name)) return '';
      const file = path.join(dir, name), b = mi_read(file), ext = mi_type(b);
      return name === mi_hash(b) + '.' + ext ? file : '';
    }
    const file = mi_source(source), b = mi_read(file);
    if (mi_source(source) !== file) return '';
    const ext = mi_type(b), name = mi_hash(b) + '.' + ext, out = path.join(dir, name);
    try { fs.writeFileSync(out, b, {flag: 'wx', mode: 0o600}); }
    catch (e) { if (e.code !== 'EEXIST') throw e;if (!mi_read(out).equals(b)) throw Error('Image snapshot changed'); }
    // Publish the reference atomically, only after the complete image exists.
    const tmp = ref + '.' + mi_modules().crypto.randomBytes(8).toString('hex');
    try { fs.writeFileSync(tmp, JSON.stringify({image: name}), {flag: 'wx', mode: 0o600});fs.renameSync(tmp, ref); }
    finally { if (fs.existsSync(tmp)) fs.unlinkSync(tmp); }
    return out;
  } catch { return ''; }
}
function mi_retain(entry, source) { mi_snapshot(entry, source);return {$: CID(Unit)}; }
io_eff(CID(MessageImage.text), mi_text);
io_eff(CID(MessageImage.snapshot), mi_snapshot);
io_eff(CID(MessageImage.retain), mi_retain);
