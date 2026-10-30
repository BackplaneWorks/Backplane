// Real filesystem sampler, synthetic homes only. No provider, hub or fixture
// process is started; a controlled clock makes coarse timestamp tests stable.
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const source = fs.readFileSync('src/server/effects/liveness.js', 'utf8')
  .replace(/CID\(([^)]+)\)/g, '"$1"');
const root = fs.mkdtempSync(join(tmpdir(), 'bp-progress-'));
const home = join(root, 'hub'), project = join(root, 'project');
fs.mkdirSync(home); fs.mkdirSync(project);
let clock = Date.now() + 2000;
const old = (file: string, text = 'old') => {
  fs.mkdirSync(resolve(file, '..'), { recursive: true });
  fs.writeFileSync(file, text); fs.utimesSync(file, 100, 100);
};
const write = (file: string, text: string) => {
  clock += 1000; fs.writeFileSync(file, text); fs.utimesSync(file, clock / 1000, clock / 1000);
};
let openHandles = 0, reads = 0, stats = 0;
const ctime = new Map<string, number>();
const effects = new Map<string, Function>();
const io = { ...fs,
  readdirSync() { throw new Error('unbounded directory read'); },
  lstatSync(file: string) {
    stats++;
    const st = fs.lstatSync(file);
    if (ctime.has(file)) Object.defineProperty(st, 'ctimeMs', { value: ctime.get(file) });
    return st;
  },
  opendirSync(file: string) {
    const dir = fs.opendirSync(file); let closed = false; openHandles++;
    return { readSync() { reads++; return dir.readSync(); }, closeSync() {
      if (!closed) { closed = true; openHandles--; } return dir.closeSync();
    } };
  }
};
const api = new Function('require', 'Date', 'io_eff', source + '\nreturn { sample: progress_sample, states: bp_progress };')(
  (name: string) => name === 'node:fs' ? io : require(name), { now: () => clock },
  (name: string, effect: Function) => effects.set(name, effect));
const sample = (rows: unknown[]) => {
  reads = 0; stats = 0;
  const value = JSON.parse(api.sample(JSON.stringify(rows), home));
  assert(reads <= 4096, `directory read budget exceeded: ${reads}`);
  assert(stats <= 4096, `metadata budget exceeded: ${stats}`);
  return value as {thread: string; at: number; reason: string}[];
};
const plan = (work = '', cwd = project, thread = 't') => [{ thread, cwd, work }];
const drain = (rows: unknown[], rounds = 8) => {
  const changed = [];
  for (let i = 0; i < rounds; i++) changed.push(...sample(rows));
  return changed;
};
const check = (name: string, run: () => void) => { run(); console.log('ok ' + name); };
try {
  check('effect registers with the actual Progress.sample name', () => assert(effects.has('Progress.sample')));
  const file = join(project, 'source.txt'); old(file);
  check('existing old files establish a baseline without progress', () => assert.deepEqual(drain(plan()), []));
  check('unchanged reads and repeated scans do not advance progress', () => {
    fs.readFileSync(file); assert.deepEqual(drain(plan(), 12), []);
  });
  check('a file write advances the owning thread at observation time', () => {
    write(file, 'new');
    const got = drain(plan());
    assert.equal(got.length, 1); assert.equal(got[0].thread, 't');
    assert.equal(got[0].at, Math.floor(clock / 1000)); assert.match(got[0].reason, /file write: source.txt/);
    assert.deepEqual(drain(plan()), []);
  });
  check('same-size writes with preserved mtime are detected through ctime', () => {
    const stamp = fs.statSync(file).mtime;
    const before = fs.statSync(file).ctimeMs;
    fs.writeFileSync(file, 'NEW'); fs.utimesSync(file, stamp, stamp);
    ctime.set(file, before + 0.125); clock += 1000;
    assert.equal(drain(plan()).length, 1);
    assert.deepEqual(drain(plan()), []);
  });
  check('new file writes with preserved mtime are detected through ctime', () => {
    const created = join(project, 'new-file'); old(created);
    clock += 1000; ctime.set(created, clock);
    assert.equal(drain(plan()).length, 1); assert.deepEqual(drain(plan()), []);
  });
  const output = join(home, 'work/w-one/out'); old(output, 'initial log');
  check('existing work output is baseline and reads are inert', () => {
    assert.deepEqual(drain(plan('w-one\tcommand')), []);
    fs.readFileSync(output); assert.deepEqual(drain(plan('w-one\tcommand')), []);
  });
  check('owned work output growth advances progress once', () => {
    write(output, 'initial log\nmore output');
    const got = drain(plan('w-one\tcommand'));
    assert.equal(got.length, 1); assert.match(got[0].reason, /work output grew: w-one/);
    assert.deepEqual(drain(plan('w-one\tcommand')), []);
  });
  check('output timestamp changes, same-size rewrites and truncation are not growth', () => {
    write(output, 'INITIAL LOG\nMORE OUTPUT');
    assert.deepEqual(drain(plan('w-one\tcommand')), []);
    write(output, ''); assert.deepEqual(drain(plan('w-one\tcommand')), []);
    write(output, 'resumed'); assert.equal(drain(plan('w-one\tcommand')).length, 1);
  });
  check('rotated output still reports new bytes when the new log is smaller', () => {
    fs.renameSync(output, output + '.old'); old(output, 'new');
    assert.equal(drain(plan('w-one\tcommand')).length, 1);
    assert.deepEqual(drain(plan('w-one\tcommand')), []);
  });
  check('many owned outputs share a bounded rotating probe budget', () => {
    sample([]);
    const ids = Array.from({length: 80}, (_, i) => 'w-many-' + i);
    for (const id of ids) old(join(home, 'work', id, 'out'), 'baseline');
    const rows = plan(ids.join('\n'));
    assert.deepEqual(sample(rows), []); assert(api.states.get('t').output.size <= 64);
    assert.deepEqual(drain(rows, 3), []); assert.equal(api.states.get('t').output.size, 80);
    write(join(home, 'work', ids[79], 'out'), 'baseline and added output');
    const got = drain(rows, 4); assert.equal(got.length, 1); assert.match(got[0].reason, /w-many-79/);
    assert.deepEqual(drain(rows, 4), []);
  });
  check('hub state, metadata, ignored trees and symlink escapes cannot feed watchdog', () => {
    sample([]); // Reset the plan before baseline scanning its ancestor.
    old(join(home, 'events.jsonl')); old(join(root, '.backplane/intent.md'));
    old(join(root, '.git/index')); old(join(root, 'component.backplane.json'));
    old(join(root, 'node_modules/package/file'));
    const outside = fs.mkdtempSync(join(tmpdir(), 'bp-progress-outside-'));
    try {
      old(join(outside, 'escape')); fs.symlinkSync(outside, join(root, 'escape'));
      assert.deepEqual(drain(plan('', root)), []);
      for (const f of [join(home, 'events.jsonl'), join(root, '.backplane/intent.md'), join(root, '.git/index'),
        join(root, 'component.backplane.json'), join(root, 'node_modules/package/file'), join(outside, 'escape')]) write(f, 'changed');
      assert.deepEqual(drain(plan('', root)), []);
    } finally { fs.rmSync(outside, { recursive: true, force: true }); }
  });
  check('project inside the hub home never counts hub commits as file progress', () => {
    sample([]); assert.deepEqual(drain(plan('', home)), []);
    write(join(home, 'events.jsonl'), 'hub commit'); assert.deepEqual(drain(plan('', home)), []);
  });
  const huge = join(root, 'huge'); fs.mkdirSync(huge);
  for (let i = 0; i < 1500; i++) old(join(huge, 'f' + String(i).padStart(4, '0')));
  check('large directories are read incrementally within per-thread budget', () => {
    sample([]); assert.deepEqual(sample(plan('', huge)), []);
    assert(reads <= 256); assert(stats <= 256); assert(openHandles > 0);
    assert(api.states.get('t').files.size <= 256);
    assert.deepEqual(drain(plan('', huge), 16), []);
    assert.equal(api.states.get('t').files.size, 1500);
    write(join(huge, 'f1499'), 'late write');
    assert.equal(drain(plan('', huge), 16).length, 1);
    assert.deepEqual(drain(plan('', huge), 16), []);
  });
  check('inactive plan removal closes partial traversal and removes state', () => {
    sample(plan('', huge)); assert(openHandles > 0);
    assert.deepEqual(sample([]), []); assert.equal(api.states.size, 0); assert.equal(openHandles, 0);
  });
  check('global budget rotates fairly across more than sixteen busy threads', () => {
    const rows = [];
    for (let i = 0; i < 20; i++) rows.push({ thread: 'fair-' + i, cwd: huge, work: '' });
    assert.deepEqual(drain(rows, 5), []);
    assert.equal(api.states.size, 20);
    for (const st of api.states.values()) assert(st.files.size > 0, 'a late thread was starved');
    write(join(huge, 'f0000'), 'fair progress');
    const got = drain(rows, 20);
    assert.equal(new Set(got.map(r => r.thread)).size, 20);
    assert.deepEqual(drain(rows, 20), []);
  });
  check('fingerprint cache stays bounded and overflow changes are observed once', () => {
    sample([]); old(join(project, 'overflow')); sample(plan());
    const state = api.states.get('t');
    while (state.files.size < 32768) state.files.set('/baseline-' + state.files.size, 'same');
    state.files.delete(join(project, 'overflow'));
    state.files.set('/fill-overflow', 'same');
    state.previousOverflow = null;
    assert.deepEqual(drain(plan()), []);
    write(join(project, 'overflow'), 'overflow changed');
    assert.equal(drain(plan()).length, 1);
    assert.deepEqual(drain(plan(), 12), []); assert.equal(state.files.size, 32768);
  });
  check('restart detects writes newer than the durable progress watermark once', () => {
    sample([]);
    const since = clock;
    write(join(project, 'written-during-restart'), 'new progress');
    const rows = [{ thread: 'restarted', cwd: project, work: '', since }];
    const observed = drain(rows);
    assert.equal(observed.length, 1);
    assert.equal(observed[0].thread, 'restarted');
    assert.deepEqual(drain(rows), []);
  });
  check('malformed plans fail closed and an empty valid plan retires state', () => {
    assert.equal(api.sample('{', home), '[]'); assert.equal(api.sample('{}', home), '[]');
    assert.deepEqual(sample([null, {thread: 'bad', cwd: 'relative'}, {cwd: project}]), []);
    assert.equal(api.states.size, 0); assert.equal(openHandles, 0);
  });
} finally {
  api.sample('[]', home);
  fs.rmSync(root, { recursive: true, force: true });
}
