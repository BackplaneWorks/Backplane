// The shared service build supplies a verified immutable compressed payload.
// No fetch, subprocess, SDK or user/project-table write occurs here.
let bp_kl_original_config;
let bp_kl_active = null, bp_kl_error = false;
const bp_kl_cache = new Map();
function bp_kl_require(ok, why) { if (!ok) throw new Error(why); }
function bp_kl_sha(data) { return require('node:crypto').createHash('sha256').update(data).digest('hex'); }
function bp_kl_safe(name) {
  return typeof name === 'string' && name.length > 0 && !/[\\:\x00-\x1f]/.test(name)
    && name.split('/').every(p => p !== '' && p !== '.' && p !== '..');
}
function bp_kl_unpack(bundle) {
  const pin = bundle.pin, archive = Buffer.from(bundle.base64, 'base64');
  bp_kl_require(archive.length === pin.bytes && bp_kl_sha(archive) === pin.sha256, 'library archive checksum/size mismatch');
  const data = require('node:zlib').gunzipSync(archive, { maxOutputLength: 64 * 1024 * 1024 });
  const files = new Map(); let total = 0, end = false, pendingPath = '', headers = 0;
  for (let at = 0; at + 512 <= data.length;) {
    const h = data.subarray(at, at + 512);
    if (h.every(b => b === 0)) { end = true; bp_kl_require(data.subarray(at).every(b => b === 0), 'data after tar end'); break; }
    const field = (a, b) => h.subarray(a, b).toString('utf8').replace(/\0.*$/s, '');
    const octal = (a, b) => { const t = field(a, b).trim(); bp_kl_require(/^[0-7]+$/.test(t), 'invalid tar number'); return parseInt(t, 8); };
    const sum = h.reduce((s, b, i) => s + (i >= 148 && i < 156 ? 32 : b), 0);
    bp_kl_require(sum === octal(148, 156), 'tar header checksum mismatch');
    const rawName = field(0, 100), prefix = field(345, 500), size = octal(124, 136);
    bp_kl_require(++headers <= 1024 && at + 512 + size <= data.length, 'library tar exceeds bounds');
    // Python's release packager uses a local POSIX PAX path record for
    // filenames over 100 bytes. Accept only that field, never links/global
    // headers or metadata which can change extraction permissions/ownership.
    if (h[156] === 120) {
      bp_kl_require(!pendingPath && !prefix && rawName === '././@PaxHeader' && size < 4096, 'unsafe library PAX header');
      const record = data.subarray(at + 512, at + 512 + size);
      const space = record.indexOf(32), length = record.subarray(0, space).toString('ascii');
      bp_kl_require(/^[0-9]+$/.test(length) && Number(length) === record.length && record.at(-1) === 10
        && record.subarray(space + 1, space + 6).toString('ascii') === 'path=', 'invalid library PAX path');
      pendingPath = record.subarray(space + 6, -1).toString('utf8');
      bp_kl_require(bp_kl_safe(pendingPath), 'unsafe library PAX path');
      at += 512 + Math.ceil(size / 512) * 512; continue;
    }
    const name = pendingPath || rawName; pendingPath = '';
    bp_kl_require(!prefix && bp_kl_safe(name) && name.startsWith(pin.root + '/')
      && (h[156] === 0 || h[156] === 48) && !field(157, 257), 'unsafe library archive member');
    const rel = name.slice(pin.root.length + 1);
    total += size;
    bp_kl_require(!files.has(rel) && files.size < 512 && total <= 64 * 1024 * 1024
      && at + 512 + size <= data.length, 'library archive exceeds bounds or duplicates a path');
    files.set(rel, data.subarray(at + 512, at + 512 + size));
    at += 512 + Math.ceil(size / 512) * 512;
  }
  bp_kl_require(end && !pendingPath, 'incomplete library archive');
  const manifest = JSON.parse(files.get('BUNDLE_MANIFEST.json').toString('utf8'));
  for (const k of ['repository', 'version', 'source_commit', 'root', 'footprint_count', 'step_model_count', 'library_nicknames'])
    bp_kl_require(JSON.stringify(manifest[k]) === JSON.stringify(pin[k]), 'library manifest mismatch: ' + k);
  bp_kl_require(manifest.schema_version === 1 && manifest.required_path_variable === 'BACKPLANE_KICAD_LIBRARY'
    && manifest.footprint_path === 'libraries' && manifest.model_path === '3dmodels'
    && manifest.files && typeof manifest.files === 'object' && !Array.isArray(manifest.files), 'invalid library manifest');
  const expected = new Set([...Object.keys(manifest.files), 'BUNDLE_MANIFEST.json', 'SHA256SUMS']);
  bp_kl_require(expected.size === files.size && [...files.keys()].every(n => expected.has(n)), 'library manifest inventory mismatch');
  for (const [name, digest] of Object.entries(manifest.files))
    bp_kl_require(bp_kl_safe(name) && /^[a-f0-9]{64}$/.test(digest) && bp_kl_sha(files.get(name)) === digest, 'library file checksum mismatch: ' + name);
  const mods = [...files.keys()].filter(n => n.endsWith('.kicad_mod'));
  bp_kl_require(mods.length === pin.footprint_count && [...files.keys()].filter(n => n.endsWith('.step')).length === pin.step_model_count
    && JSON.stringify([...new Set(mods.map(n => n.split('/')[1].replace(/\.pretty$/, '')))].sort()) === JSON.stringify([...pin.library_nicknames].sort()), 'library geometry inventory mismatch');
  return { pin, files };
}
// Parse only table structure; retain the original bytes, comments and custom
// entries when appending ours. A quoted parenthesis is never a table boundary.
function bp_kl_sexp(text) {
  bp_kl_require(Buffer.byteLength(text) <= 4 * 1024 * 1024, 'KiCad library table exceeds size bound');
  const tokens = []; const re = /\s+|;[^\n]*|\(|\)|"(?:\\.|[^"\\])*"|[^\s();"]+/gy;
  let at = 0;
  while (at < text.length) {
    re.lastIndex = at; const m = re.exec(text);
    bp_kl_require(m, 'malformed KiCad library table'); at = re.lastIndex;
    if (!/^\s|^;/.test(m[0])) { bp_kl_require(tokens.length < 100000, 'KiCad library table exceeds token bound'); tokens.push({ text: m[0], at: m.index }); }
  }
  let n = 0;
  function node(depth) {
    bp_kl_require(depth < 64 && n < tokens.length, 'malformed KiCad library table');
    const t = tokens[n++];
    if (t.text !== '(') {
      bp_kl_require(t.text !== ')', 'unexpected table close');
      return { value: t.text.startsWith('"') ? t.text.slice(1, -1).replace(/\\(.)/gs, '$1') : t.text };
    }
    const children = [];
    while (n < tokens.length && tokens[n].text !== ')') children.push(node(depth + 1));
    bp_kl_require(n < tokens.length, 'unclosed KiCad library table');
    const close = tokens[n++].at; return { children, close };
  }
  const root = node(0); bp_kl_require(n === tokens.length, 'multiple KiCad table roots'); return root;
}
function bp_kl_table(text, pin, original) {
  const tree = bp_kl_sexp(text), libs = new Map();
  bp_kl_require(tree.children?.[0]?.value === 'fp_lib_table', 'not a footprint table');
  for (const c of tree.children.slice(1)) {
    if (c.children?.[0]?.value !== 'lib') continue;
    const fields = Object.fromEntries(c.children.slice(1).filter(x => x.children).map(x => [x.children[0]?.value, x.children[1]?.value]));
    bp_kl_require(fields.name && !libs.has(fields.name), 'duplicate footprint nickname'); libs.set(fields.name, fields);
  }
  const additions = [];
  for (const name of pin.library_nicknames) {
    const uri = '${BACKPLANE_KICAD_LIBRARY}/libraries/' + name + '.pretty';
    const old = libs.get(name);
    bp_kl_require(!old || (old.uri === uri && old.type === 'KiCad'), 'custom footprint nickname conflicts with bundled ' + name);
    if (!old) additions.push('  (lib (name "' + name + '")(type "KiCad")(uri "' + uri + '")(options "")(descr "Backplane bundled library"))\n');
  }
  const merged = text.slice(0, tree.close) + additions.join('') + text.slice(tree.close);
  // An explicit custom config root may itself appear in a URI. Keep its old
  // meaning in the copied table after KICAD_CONFIG_HOME selects our overlay.
  return merged.replaceAll('${KICAD_CONFIG_HOME}', original.replaceAll('\\', '/').replaceAll('"', '\\"'));
}
function bp_kl_plain(path) {
  const fs = require('node:fs'), p = require('node:path');
  let at = p.parse(path).root;
  for (const part of path.slice(at.length).split(p.sep).filter(Boolean)) {
    at = p.join(at, part);
    if (fs.existsSync(at)) bp_kl_require(!fs.lstatSync(at).isSymbolicLink(), 'symlink in managed library/config path');
  }
}
function bp_kl_verify_dir(root, files) {
  const fs = require('node:fs'), p = require('node:path'); const seen = []; let bytes = 0, dirs = 0;
  function walk(dir, rel) {
    bp_kl_require(++dirs <= 1024 && rel.split('/').length <= 16, 'extracted library directory bound exceeded');
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const name = rel ? rel + '/' + e.name : e.name, path = p.join(dir, e.name);
      bp_kl_require(!e.isSymbolicLink(), 'symlink in extracted library');
      if (e.isDirectory()) walk(path, name);
      else {
        bytes += fs.lstatSync(path).size;
        bp_kl_require(bytes <= 64 * 1024 * 1024 && seen.length < 512 && e.isFile() && files.has(name)
          && bp_kl_sha(fs.readFileSync(path)) === files.get(name), 'extracted library checksum mismatch: ' + name); seen.push(name);
      }
    }
  }
  walk(root, ''); bp_kl_require(seen.length === files.size, 'extracted library inventory mismatch');
}
function bp_kl_stamp(root, files) {
  const fs = require('node:fs'), p = require('node:path');
  const stamps = []; let dirs = 0;
  function walk(dir, rel) {
    bp_kl_require(++dirs <= 1024 && rel.split('/').length <= 16, 'extracted library directory bound exceeded');
    for (const e of fs.readdirSync(dir, { withFileTypes: true }).sort((a,b) => a.name.localeCompare(b.name))) {
      const n = rel ? rel + '/' + e.name : e.name, at = p.join(dir, e.name);
      bp_kl_require(!e.isSymbolicLink(), 'symlink in extracted library');
      if (e.isDirectory()) walk(at, n);
      else {
        bp_kl_require(e.isFile() && files.has(n), 'unexpected item in extracted library');
        const s = fs.lstatSync(at); stamps.push([n, s.size, s.mtimeMs, s.ctimeMs]);
      }
    }
  }
  walk(root, ''); bp_kl_require(stamps.length === files.size, 'extracted library inventory mismatch'); return stamps;
}
function bp_kl_copy(src, dest, budget = { files: 0, bytes: 0 }, depth = 0) {
  const fs = require('node:fs'), p = require('node:path');
  bp_kl_require(depth < 32, 'source KiCad profile exceeds depth bound');
  for (const e of fs.readdirSync(src, { withFileTypes: true })) {
    const a = p.join(src, e.name), b = p.join(dest, e.name);
    // Read a user-linked file into the isolated profile, never write through it.
    if (e.isDirectory()) { fs.mkdirSync(b); bp_kl_copy(a, b, budget, depth + 1); }
    else if (e.isFile() || (e.isSymbolicLink() && fs.statSync(a).isFile())) {
      budget.bytes += fs.statSync(a).size;
      bp_kl_require(++budget.files <= 1024 && budget.bytes <= 64 * 1024 * 1024, 'source KiCad profile exceeds size bound');
      fs.copyFileSync(a, b);
    }
    else bp_kl_require(false, 'unsupported item in source KiCad profile');
  }
}
// Kernel-owned nonblocking lock: a crash releases it without PID polling,
// stale-owner reclamation races or a child inheriting the handle. Every
// plot and render child sets up once, taking it for a tenth of a second,
// so children started together met it held: it is tried again for up to
// `wait` ms before the setup that came second fails.
let bp_kl_lock_api;
function bp_kl_retry(take, wait) {
  const end = Date.now() + wait;
  for (let got = take(); ; got = take()) {
    if (got !== null || Date.now() >= end) return got;
    Bun.sleepSync(25);
  }
}
function bp_kl_lock(lock, wait = 5000) {
  const fs = require('node:fs'), ffi = require('bun:ffi');
  bp_kl_plain(lock);
  if (process.platform === 'win32') {
    if (!bp_kl_lock_api) bp_kl_lock_api = ffi.dlopen('kernel32.dll', {
      CreateFileW: { args: ['ptr','u32','u32','ptr','u32','u32','ptr'], returns: 'i64' },
      CloseHandle: { args: ['i64'], returns: 'i32' },
    }).symbols;
    const name = Buffer.from(lock + '\0', 'utf16le');
    const h = bp_kl_retry(() => { const h = bp_kl_lock_api.CreateFileW(ffi.ptr(name), 0x40000000, 1, null, 4, 0x80, null); return Number(h) === -1 ? null : h; }, wait);
    bp_kl_require(h !== null, 'another KiCad library setup is active or its lock cannot be opened; retry after it finishes');
    return () => bp_kl_lock_api.CloseHandle(h);
  }
  if (!bp_kl_lock_api) bp_kl_lock_api = ffi.dlopen(process.platform === 'darwin' ? '/usr/lib/libSystem.B.dylib' : 'libc.so.6', {
    flock: { args: ['i32','i32'], returns: 'i32' },
  }).symbols;
  const fd = fs.openSync(lock, fs.constants.O_CREAT | fs.constants.O_RDWR, 0o600);
  if (bp_kl_retry(() => bp_kl_lock_api.flock(fd, 6) === 0 ? fd : null, wait) === null) {
    fs.closeSync(fd); throw new Error('another KiCad library setup is active; retry after it finishes');
  }
  return () => fs.closeSync(fd);
}
function bp_kl_prepare(runtime, home, suppliedConfig) {
  try {
    const bundle = globalThis.BACKPLANE_KICAD_BUNDLE;
    // Unbundled source/test programs retain their existing KiCad behavior.
    if (!bundle || !runtime) return '';
    const fs = require('node:fs'), p = require('node:path');
    bp_kl_require(home && p.isAbsolute(home), 'absolute BACKPLANE_HOME required for bundled KiCad library');
    // Canonicalize only the trusted hub home/ancestors (macOS /var and /tmp
    // are symlinks). Managed descendants must still never contain a link.
    let parent = home; const rest = [];
    while (!fs.existsSync(parent)) { rest.unshift(p.basename(parent)); parent = p.dirname(parent); }
    home = p.join(fs.realpathSync(parent), ...rest);
    if (bp_kl_original_config === undefined) {
      const origin = process.env.BACKPLANE_KICAD_USER_CONFIG;
      const managed = p.resolve(home, 'kicad-library/profiles') + p.sep;
      bp_kl_original_config = suppliedConfig && p.resolve(suppliedConfig).startsWith(managed) && origin && p.isAbsolute(origin) ? origin : suppliedConfig || '';
    }
    const user = process.env.HOME || require('node:os').homedir();
    const original = p.resolve(bp_kl_original_config || (process.platform === 'darwin' ? p.join(user, 'Library/Preferences/kicad')
      : process.platform === 'win32' ? p.join(process.env.APPDATA || p.join(user, 'AppData/Roaming'), 'kicad')
      : p.join(process.env.XDG_CONFIG_HOME || p.join(user, '.config'), 'kicad')));
    const base = p.resolve(home, 'kicad-library'), pin = bundle.pin;
    bp_kl_require(/^[a-f0-9]{40}$/.test(pin.source_commit) && /^[0-9.]+$/.test(pin.version) && pin.root === 'Backplane_KiCad_Library', 'invalid embedded library pin');
    const installed = p.join(base, pin.version + '-' + pin.source_commit, pin.root);
    const config = p.join(base, 'profiles', bp_kl_sha(original).slice(0, 20));
    const profile = p.join(config, '10.99'), table = p.join(profile, 'fp-lib-table');
    const key = [home, runtime, original, pin.sha256].join('\n'), prior = bp_kl_cache.get(key);
    bp_kl_plain(base); bp_kl_plain(installed); bp_kl_plain(profile);
    let cached = false;
    try { cached = prior && JSON.stringify(bp_kl_stamp(installed, prior.files)) === prior.stamp && fs.readFileSync(table, 'utf8') === prior.table; }
    catch (_) { /* A corrupt/partial managed cache is rebuilt from pinned bytes below. */ }
    if (cached) {
      process.env.BACKPLANE_KICAD_LIBRARY = installed;
      process.env.KICAD_CONFIG_HOME = config;
      process.env.BACKPLANE_KICAD_USER_CONFIG = original;
      bp_kl_active = { installed, config, original }; bp_kl_error = false;
      return '';
    }
    const files = new Map(Object.entries(bundle.files || {}));
    bp_kl_require(files.size > 0 && files.size <= 512 && [...files].every(([n,h]) => bp_kl_safe(n) && /^[a-f0-9]{64}$/.test(h)), 'invalid compiled library inventory');
    fs.mkdirSync(base, { recursive: true, mode: 0o700 });
    const unlock = bp_kl_lock(p.join(base, '.setup.lock'));
    let stage = '';
    try {
      stage = fs.mkdtempSync(p.join(base, '.stage-'));
      bp_kl_plain(installed); bp_kl_plain(profile);
      let valid = false;
      if (fs.existsSync(installed)) try { bp_kl_verify_dir(installed, files); valid = true; } catch (_) { /* retain damaged copy when repairing */ }
      if (!valid) {
        const unpacked = bp_kl_unpack(bundle).files;
        bp_kl_require(unpacked.size === files.size && [...unpacked].every(([n,b]) => bp_kl_sha(b) === files.get(n)), 'compiled library inventory mismatch');
        const out = p.join(stage, pin.root); fs.mkdirSync(out);
        for (const [name, data] of unpacked) {
          const target = p.join(out, name); fs.mkdirSync(p.dirname(target), { recursive: true, mode: 0o700 });
          fs.writeFileSync(target, data, { flag: 'wx', mode: 0o600 });
        }
        bp_kl_verify_dir(out, files);
        fs.mkdirSync(p.dirname(installed), { recursive: true, mode: 0o700 });
        if (fs.existsSync(installed)) fs.renameSync(installed, installed + '.damaged-' + require('node:crypto').randomUUID());
        fs.renameSync(out, installed);
      }
      const existing = fs.existsSync(profile), work = existing ? profile : p.join(stage, 'profile');
      if (!existing) {
        fs.mkdirSync(work);
        const from = p.join(original, '10.99');
        if (fs.existsSync(from)) bp_kl_copy(from, work);
        for (const name of ['sym-lib-table', 'design-block-lib-table']) {
          const at = p.join(work, name);
          if (!fs.existsSync(at)) continue;
          const before = fs.readFileSync(at, 'utf8');
          const after = before.replaceAll('${KICAD_CONFIG_HOME}', original.replaceAll('\\', '/').replaceAll('"', '\\"'));
          if (before !== after) fs.writeFileSync(at, after);
        }
      }
      const tableAt = p.join(work, 'fp-lib-table');
      const stock = p.join(runtime, 'share/kicad/fp-lib-table');
      const before = fs.existsSync(tableAt) ? fs.readFileSync(tableAt, 'utf8') : fs.existsSync(stock) ? fs.readFileSync(stock, 'utf8') : '(fp_lib_table\n)\n';
      const after = bp_kl_table(before, pin, original);
      if (after !== before || !fs.existsSync(tableAt)) {
        const temp = p.join(stage, 'fp-lib-table'); fs.writeFileSync(temp, after, { flag: 'wx', mode: 0o600 }); fs.renameSync(temp, tableAt);
      }
      if (!existing) { fs.mkdirSync(config, { recursive: true, mode: 0o700 }); fs.renameSync(work, profile); }
      // Environment activation is last. Failed setup never changes the prior
      // selected library/profile, and no existing payload is ever unpacked over.
      process.env.BACKPLANE_KICAD_LIBRARY = installed;
      process.env.KICAD_CONFIG_HOME = config;
      process.env.BACKPLANE_KICAD_USER_CONFIG = original;
      bp_kl_cache.set(key, { files, stamp: JSON.stringify(bp_kl_stamp(installed, files)), table: after });
      bp_kl_active = { installed, config, original }; bp_kl_error = false;
      return '';
    } finally { if (stage) fs.rmSync(stage, { recursive: true, force: true }); unlock(); }
  } catch (e) { bp_kl_error = true; return 'KiCad library setup failed: ' + e.message; }
}
function bp_kl_child_env() {
  let values = [];
  if (globalThis.BACKPLANE_KICAD_BUNDLE) {
    const active = bp_kl_active && !bp_kl_error;
    if (active) values = ['BACKPLANE_KICAD_LIBRARY=' + bp_kl_active.installed, 'KICAD_CONFIG_HOME=' + bp_kl_active.config];
    else {
      const original = bp_kl_active?.original || bp_kl_original_config || process.env.KICAD_CONFIG_HOME || '';
      values = original ? ['-u', 'BACKPLANE_KICAD_LIBRARY', 'KICAD_CONFIG_HOME=' + original]
        : ['-u', 'BACKPLANE_KICAD_LIBRARY', '-u', 'KICAD_CONFIG_HOME'];
    }
  }
  let out = { $: 'Nil' };
  for (let i = values.length - 1; i >= 0; i--) out = { $: 'Con', head: values[i], tail: out };
  return out;
}
io_eff(CID(Library.setup), bp_kl_prepare);
io_eff(CID(Library.child_env), bp_kl_child_env);
