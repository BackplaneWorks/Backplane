// Containment of what the hub opens and writes for the viewers
// (src/server/effects/zip.js: Zip.file for /view/get, Zip.stage for the
// converters, Zip.scratch and Zip.publish for the renderer's pictures), on
// synthetic files in temporary folders. Everything is checked after the fix:
// plain files and a root that is an alias work; a link anywhere below the
// root, a hard link, a folder, a name that climbs are refused; nothing is
// written outside the folder; descriptors are closed on refusal and cancel.
//
//   bun test/tools/contain_fs.mjs
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as zlib from 'node:zlib';
import * as crypto from 'node:crypto';
import * as pathm from 'node:path';
import * as os from 'node:os';
import { join, resolve } from 'node:path';
import { createContext, runInContext } from 'node:vm';

const source = fs.readFileSync(resolve('src/server/effects/zip.js'), 'utf8');
const list = xs => { const out = []; for (; xs && xs.$ === 'Con'; xs = xs.tail) out.push(xs.head); return out; };
let failed = 0;
async function t(name, f) {
  try { await f(); console.log('ok ' + name); }
  catch (e) { failed++; console.log('FAIL ' + name + ': ' + String(e?.stack ?? e).slice(0, 1500)); }
}

function load({ cap, cancelAfter = Infinity } = {}) {
  let text = source;
  if (cap) text = text.replace('const STAGE_CAP = 256 * 1024 * 1024;', `const STAGE_CAP = ${cap};`);
  const sent = [], queue = [], effects = new Map();
  let calls = 0, now = 0;
  const write = b => {
    calls++;
    if (calls > cancelAfter) return -32;
    if (calls % 7 === 0) return -11;
    const n = Math.min(b.length, 1 + (calls * 7919) % 40000);
    sent.push(Buffer.from(b.subarray(0, n)));
    return n;
  };
  const ctx = createContext({ Buffer, BigInt, process, performance: { now: () => now }, Date, Error, Object, JSON,
    require: name => ({ fs, zlib, path: pathm, os, crypto })[name],
    io_tup: (socket, result) => ({ socket, result }), io_done: () => ({ ok: true }), io_fail: errno => ({ ok: false, errno }),
    io_park_on: (_s, _o, _k, go) => queue.push(go),
    io_sys: () => assert.fail('a Bun-loop handle needs no descriptor'),
    io_eff: (name, fn) => effects.set(name, fn),
    BP_H: { EAGAIN: 11, is: s => s === 73, get: () => ({ write, gone: false }) } });
  runInContext('(function(){' + text.replace(/CID\(([\w.]+)\)/g, (_, n) => JSON.stringify(n)) + '})();', ctx);
  for (const e of ['Zip.file', 'Zip.stage', 'Zip.scratch', 'Zip.publish']) assert.ok(effects.has(e), e + ' registered');
  const drain = r => {
    let turns = 0;
    while (r === undefined) { assert.ok(queue.length); assert.ok(++turns < 1e7); now += 1; r = queue.shift()(); }
    return r;
  };
  return { effects, sent, drain };
}
const body = bufs => { const b = Buffer.concat(bufs); const at = b.indexOf('\r\n\r\n'); return { head: b.subarray(0, at).toString(), body: b.subarray(at + 4) }; };
const fds = () => fs.readdirSync('/proc/self/fd').length;

const base = fs.realpathSync(fs.mkdtempSync(join(os.tmpdir(), 'bp-contain-')));
process.on('exit', () => fs.rmSync(base, { recursive: true, force: true }));
const hub = join(base, 'hub'); fs.mkdirSync(hub);
fs.writeFileSync(join(hub, 'token'), 'secret-token');
const outside = join(base, 'outside'); fs.mkdirSync(outside);
fs.writeFileSync(join(outside, 'secret.step'), 'OUTSIDE SECRET');
fs.mkdirSync(join(outside, 'dir')); fs.writeFileSync(join(outside, 'dir', 'x.step'), 'OUTSIDE IN DIR');

function project() {
  const root = fs.mkdtempSync(join(base, 'proj-'));
  fs.mkdirSync(join(root, 'mech'));
  fs.writeFileSync(join(root, 'mech', 'a.step'), 'ISO-10303-21; synthetic A');
  fs.writeFileSync(join(root, 'board.kicad_pcb'), '(kicad_pcb)');
  return root;
}
const stageDir = join(hub, 'stage');

// ---- Zip.file (/view/get) -------------------------------------------------

await t('view.get streams a plain file from a held descriptor', () => {
  const root = project(), { effects, sent, drain } = load();
  const before = fds();
  const r = drain(effects.get('Zip.file')(73, root, 'mech/a.step', hub, 'attachment', '', [], () => {}));
  assert.equal(r.result.ok, true);
  const res = body(sent);
  assert.match(res.head, /200 OK/);
  assert.equal(res.body.toString(), 'ISO-10303-21; synthetic A');
  assert.equal(fds(), before, 'descriptors');
});

await t('view.get keeps sending the file it opened when the path is swapped for a link', () => {
  const root = project(), { effects, sent, drain } = load({ cancelAfter: Infinity });
  fs.writeFileSync(join(root, 'mech', 'big.step'), 'A'.repeat(300000));
  const first = effects.get('Zip.file')(73, root, 'mech/big.step', hub, 'attachment', '', [], () => {});
  // opened now; the path is replaced by a link to a secret before the bytes go
  fs.rmSync(join(root, 'mech', 'big.step'));
  fs.symlinkSync(join(outside, 'secret.step'), join(root, 'mech', 'big.step'));
  const r = drain(first);
  assert.equal(r.result.ok, true);
  const res = body(sent);
  assert.equal(res.body.length, 300000);
  assert.ok(!res.body.includes(Buffer.from('OUTSIDE')), 'the secret was not sent');
});

for (const [name, setup, rel] of [
  ['a link to a file outside', root => fs.symlinkSync(join(outside, 'secret.step'), join(root, 'mech', 'l.step')), 'mech/l.step'],
  ['a link to a folder outside', root => fs.symlinkSync(join(outside, 'dir'), join(root, 'mech', 'ld')), 'mech/ld/x.step'],
  ['a link to a folder inside (refused, fail closed)', root => fs.symlinkSync(join(root, 'mech'), join(root, 'alias')), 'alias/a.step'],
  ['a hard link', root => fs.linkSync(join(root, 'mech', 'a.step'), join(root, 'mech', 'h.step')), 'mech/h.step'],
  ['a folder', () => {}, 'mech'],
  ['a name that climbs', () => {}, '../outside/secret.step'],
  ['a missing file', () => {}, 'mech/none.step'],
]) {
  await t('view.get refuses ' + name + ' and closes its descriptors', () => {
    const root = project(); setup(root);
    const { effects, sent, drain } = load(), before = fds();
    drain(effects.get('Zip.file')(73, root, rel, hub, 'attachment', '', [], () => {}));
    assert.match(body(sent).head, /404/);
    assert.ok(!body(sent).body.includes(Buffer.from('OUTSIDE')));
    assert.equal(fds(), before);
  });
}

await t('view.get cancelled mid-stream closes its descriptor', () => {
  const root = project();
  fs.writeFileSync(join(root, 'mech', 'big.step'), 'B'.repeat(2000000));
  const { effects, drain } = load({ cancelAfter: 12 }), before = fds();
  const r = drain(effects.get('Zip.file')(73, root, 'mech/big.step', hub, 'attachment', '', [], () => {}));
  assert.equal(r.result.ok, false);
  assert.equal(fds(), before);
});

// ---- Zip.stage (converter inputs) ------------------------------------------

await t('stage copies a plain file into the hub folder, named by its content', () => {
  const root = project(), { effects } = load(), before = fds();
  const [p] = list(effects.get('Zip.stage')(root, 'mech/a.step', hub, stageDir));
  assert.ok(p && p.startsWith(stageDir + '/'), String(p));
  const want = crypto.createHash('sha256').update('ISO-10303-21; synthetic A').digest('hex');
  assert.equal(pathm.basename(p), want + '.step');
  assert.equal(fs.readFileSync(p, 'utf8'), 'ISO-10303-21; synthetic A');
  assert.equal(fs.statSync(p).mode & 0o777, 0o600);
  assert.equal(fs.statSync(stageDir).mode & 0o777, 0o700);
  assert.equal(fds(), before);
});

await t('staging the same bytes again keeps the same file and modification time', async () => {
  const root = project(), { effects } = load();
  const [p1] = list(effects.get('Zip.stage')(root, 'mech/a.step', hub, stageDir));
  const m1 = fs.statSync(p1).mtimeMs;
  await new Promise(r => setTimeout(r, 30));
  const [p2] = list(effects.get('Zip.stage')(root, 'mech/a.step', hub, stageDir));
  assert.equal(p2, p1);
  assert.equal(fs.statSync(p2).mtimeMs, m1);
  assert.deepEqual(fs.readdirSync(stageDir).filter(n => n.startsWith('.part-')), [], 'no partial file left');
});

await t('stage works through an alias of the project root', () => {
  const root = project(), alias = join(base, 'alias-' + pathm.basename(root));
  fs.symlinkSync(root, alias);
  const { effects } = load();
  const [p] = list(effects.get('Zip.stage')(alias, 'mech/a.step', hub, stageDir));
  assert.equal(fs.readFileSync(p, 'utf8'), 'ISO-10303-21; synthetic A');
});

for (const [name, setup, rel] of [
  ['a link to a file outside', root => fs.symlinkSync(join(outside, 'secret.step'), join(root, 'mech', 'l.step')), 'mech/l.step'],
  ['a link to a folder outside', root => fs.symlinkSync(join(outside, 'dir'), join(root, 'mech', 'ld')), 'mech/ld/x.step'],
  ['a link to a folder inside', root => fs.symlinkSync(join(root, 'mech'), join(root, 'alias')), 'alias/a.step'],
  ['a hard link', root => fs.linkSync(join(root, 'mech', 'a.step'), join(root, 'mech', 'h.step')), 'mech/h.step'],
  ['a folder', () => {}, 'mech'],
  ['a name that climbs', () => {}, '../outside/secret.step'],
  ['an absolute name', () => {}, join(outside, 'secret.step')],
  ['a missing file', () => {}, 'mech/none.step'],
  ['hub storage', root => fs.symlinkSync(hub, join(root, 'hubl')), 'hubl/token'],
]) {
  await t('stage refuses ' + name + ' and leaves nothing behind', () => {
    const root = project(); setup(root);
    const { effects } = load(), before = fds();
    const had = fs.existsSync(stageDir) ? fs.readdirSync(stageDir).length : 0;
    assert.deepEqual(list(effects.get('Zip.stage')(root, rel, hub, stageDir)), []);
    assert.equal(fds(), before);
    assert.equal(fs.existsSync(stageDir) ? fs.readdirSync(stageDir).length : 0, had, 'stage folder unchanged');
  });
}

await t('stage refuses a file over its limit', () => {
  const root = project(), { effects } = load({ cap: 10 });
  assert.deepEqual(list(effects.get('Zip.stage')(root, 'mech/a.step', hub, stageDir)), []);
});

await t('stage refuses a stage folder that is a link or is not private', () => {
  const root = project(), { effects } = load();
  const bad = join(base, 'bad-stage'); fs.symlinkSync(outside, bad);
  assert.deepEqual(list(effects.get('Zip.stage')(root, 'mech/a.step', hub, bad)), []);
  assert.ok(!fs.readdirSync(outside).some(n => n.endsWith('.step') && n !== 'secret.step'), 'nothing written through the link');
  const open = join(base, 'open-stage'); fs.mkdirSync(open, { mode: 0o755 }); fs.chmodSync(open, 0o755);
  assert.deepEqual(list(effects.get('Zip.stage')(root, 'mech/a.step', hub, open)), []);
});

// ---- Zip.scratch / Zip.publish (the renderer's pictures) -----------------------

function pictures(effects, names) {
  const [dir] = list(effects.get('Zip.scratch')(stageDir));
  assert.ok(dir && dir.startsWith(stageDir + '/run-'));
  for (const n of names) fs.writeFileSync(join(dir, n), 'PNG ' + n);
  return dir;
}

await t('publish makes renders/ and puts the pictures there, only the .png ones', () => {
  const root = project(), { effects } = load(), before = fds();
  const dir = pictures(effects, ['a.iso.png', 'a.top.png', 'notes.txt', '.hidden.png']);
  const done = list(effects.get('Zip.publish')(root, 'mech/renders', dir, hub));
  assert.deepEqual(done, ['a.iso.png', 'a.top.png']);
  assert.equal(fs.readFileSync(join(root, 'mech/renders/a.iso.png'), 'utf8'), 'PNG a.iso.png');
  assert.ok(!fs.existsSync(join(root, 'mech/renders/notes.txt')));
  assert.deepEqual(fs.readdirSync(join(root, 'mech/renders')).filter(n => n.endsWith('.part')), []);
  assert.equal(fds(), before);
});

await t('publish replaces a link at a picture\'s place and never writes through it', () => {
  const root = project(), { effects } = load();
  fs.mkdirSync(join(root, 'mech/renders'));
  fs.writeFileSync(join(outside, 'victim.png'), 'VICTIM');
  fs.symlinkSync(join(outside, 'victim.png'), join(root, 'mech/renders/a.iso.png'));
  const dir = pictures(effects, ['a.iso.png']);
  assert.deepEqual(list(effects.get('Zip.publish')(root, 'mech/renders', dir, hub)), ['a.iso.png']);
  assert.equal(fs.readFileSync(join(outside, 'victim.png'), 'utf8'), 'VICTIM');
  assert.ok(!fs.lstatSync(join(root, 'mech/renders/a.iso.png')).isSymbolicLink());
});

for (const [name, setup, relDir] of [
  ['renders being a link to a folder outside', root => { fs.mkdirSync(join(outside, 'r')); fs.symlinkSync(join(outside, 'r'), join(root, 'mech/renders')); }, 'mech/renders'],
  ['mech being a link to a folder outside', root => { fs.rmSync(join(root, 'mech'), { recursive: true }); fs.symlinkSync(join(outside, 'dir'), join(root, 'mech')); }, 'mech/renders'],
  ['a folder that climbs', () => {}, '../outside'],
  ['an absolute folder', () => {}, outside],
]) {
  await t('publish refuses ' + name + ', writing nothing outside', () => {
    const root = project(); setup(root);
    const { effects } = load(), before = fds();
    const snapshot = () => JSON.stringify([fs.readdirSync(outside).sort(), fs.readdirSync(join(outside, 'dir')).sort(), fs.existsSync(join(outside, 'r')) ? fs.readdirSync(join(outside, 'r')) : null]);
    const was = snapshot();
    const dir = pictures(effects, ['a.iso.png']);
    assert.deepEqual(list(effects.get('Zip.publish')(root, relDir, dir, hub)), []);
    assert.equal(snapshot(), was, 'outside unchanged');
    assert.equal(fds(), before);
  });
}

await t('publish works through an alias of the project root', () => {
  const root = project(), alias = join(base, 'alias2-' + pathm.basename(root));
  fs.symlinkSync(root, alias);
  const { effects } = load();
  const dir = pictures(effects, ['a.iso.png']);
  assert.deepEqual(list(effects.get('Zip.publish')(alias, 'mech/renders', dir, hub)), ['a.iso.png']);
  assert.ok(fs.existsSync(join(root, 'mech/renders/a.iso.png')));
});

await t('a link in the hub\'s picture folder is not copied', () => {
  const root = project(), { effects } = load();
  const dir = pictures(effects, []);
  fs.symlinkSync(join(hub, 'token'), join(dir, 'a.iso.png'));
  assert.deepEqual(list(effects.get('Zip.publish')(root, 'mech/renders', dir, hub)), []);
  assert.ok(!fs.existsSync(join(root, 'mech/renders/a.iso.png')));
});

console.log(failed ? failed + ' failed' : 'all ok');
process.exit(failed ? 1 : 0);
