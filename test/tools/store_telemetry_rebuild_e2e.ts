// Run only the store boot probe, never a hub or telemetry sender:
// bun test/tools/store_telemetry_rebuild_e2e.ts BUILT_STORE_UP_PROBE
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { cpSync, mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { Database } from 'bun:sqlite';

const probe = resolve(process.argv[2] ?? '');
assert(process.argv[2], 'provide the built test/native/store_up_probe.bend executable');
const root = mkdtempSync(join(tmpdir(), 'bp-telemetry-rebuild-'));
const hash = (p: string) => createHash('sha256').update(readFileSync(p)).digest('hex');
const event = ($: string, fields: object) => ({ $, ...fields, at: 86400 });
const setting = (key: string, value: string) => event('SettingSet', { key, value });
const work = (id: string) => {
  const mail = event('MailPosted', { id: `mail-${id}`, conv: 'fixture', from: 'fixture', to: 'fixture', kind: 'note', ref: '', want: '', head: '', size: 0 });
  const usage = event('TurnUsage', { thread: id, provider: 'fixture', model: 'fixture', key: `usage-${id}`, input: 100, cread: 20, cwrite: 30, output: 40, reasoning: 10, cost: 0, known: 'irwo', basis: 'fixture' });
  return [
    event('ThreadCreated', { id, project: 'fixture', title: 'fixture', env: 'local', branch: '', worktree: '', provider: 'fixture', model: 'fixture' }),
    event('MessagePosted', { thread: id, msg: `user-${id}`, role: 'user', text: 'fixture' }),
    event('MessagePosted', { thread: id, msg: `assistant-${id}`, role: 'assistant', text: 'fixture' }),
    mail, mail,
    event('RoomPosted', { id: `room-mail-${id}`, room: 'fixture', from: 'fixture', text: 'fixture', hop: 0 }),
    event('BotWoke', { bot: 'fixture', hop: 0 }), usage, usage,
    // Deliberately adversarial durable markers also occur before consent/off:
    // projection must not count or backfill them. This is not a PCB capture test.
    setting(`telemetry.pcb.${id}`, JSON.stringify({ created: 3, ordered: 1, at: 86400 })),
  ];
};
const log = [
  event('ProjectCreated', { id: 'fixture', title: 'fixture', root: '/tmp/fixture' }),
  ...work('before-consent'),
  setting('telemetry.usage.enabled', '1'), setting('telemetry.asked', '1'), setting('telemetry.usage.asked', '1'),
  ...work('before-reset'), setting('telemetry.reset', '1'), setting('telemetry.share', 'off'),
  ...work('while-off'), setting('telemetry.share', 'on'), ...work('after-opt-in'),
].map(x => JSON.stringify(x)).join('\n') + '\n';
const check = (label: string, f: () => void) => { f(); console.log(`ok ${label}`); };
const boot = async (home: string) => {
  const p = Bun.spawn([probe, home], {
    env: { HOME: root, PATH: '/usr/bin:/bin', BEND_NO_TELEMETRY: '1' },
    stdout: 'pipe', stderr: 'pipe',
  });
  let timedOut = false;
  const timer = setTimeout(() => { timedOut = true; p.kill('SIGTERM'); }, 120_000);
  try {
    const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
    assert(!timedOut, 'store probe timed out');
    assert.equal(code, 0, out + err);
    assert(!/^FAIL|^Error:/m.test(out + err), out + err);
    return out + err;
  } finally { clearTimeout(timer); }
};
const tables = (file: string) => {
  const db = new Database(file, { readonly: true });
  try {
    const names = db.query("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'entry_fts%' AND name NOT LIKE 'sqlite_%' ORDER BY name").values().map(r => String(r[0]));
    return Object.fromEntries([
      ...names.map(t => [t, db.query(`SELECT * FROM "${t}"`).values().map(r => JSON.stringify(r)).sort()]),
      ['entry_fts', db.query('SELECT rowid,text FROM entry_fts').values().map(r => JSON.stringify(r)).sort()],
    ]);
  } finally { db.close(); }
};
const state = (home: string) => {
  const db = new Database(join(home, 'store.db'), { readonly: true });
  try {
    const generation = db.query("SELECT v FROM setting WHERE k='telemetry.usage.generation'").get() as { v: string } | null;
    const reset = db.query("SELECT v FROM setting WHERE k='telemetry.reset'").get() as { v: string } | null;
    const usage = db.query("SELECT json FROM obj WHERE domain='usage' AND key='1970-01-02'").get() as { json: string } | null;
    assert(generation && usage, 'generation and daily usage must exist');
    return { generation: generation.v, reset: `${reset?.v ?? '0'}:${generation.v}`, counts: JSON.parse(usage.json) };
  } finally { db.close(); }
};

try {
  const cold = join(root, 'cold'); mkdirSync(cold); writeFileSync(join(cold, 'events.jsonl'), log);
  const logHash = hash(join(cold, 'events.jsonl'));
  await boot(cold);
  const expected = tables(join(cold, 'store.db'));
  const projected = state(cold);
  check('cold replay excludes pre-consent, pre-reset and off activity; deduplicates mail and token reports', () => {
    assert.equal(projected.counts, '1/1/1/2/1/100/20/30/40/1/3/1');
    assert.notEqual(projected.generation, '');
    assert(projected.reset.startsWith('1:'), 'the durable reset must survive replay');
  });
  for (const [codec, kind] of [['7', 'incomplete'], ['6', 'incomplete'], ['6', 'usage-only'], ['6', 'all-projection-missing']]) {
    const home = join(root, `old-${codec}-${kind}`); mkdirSync(home);
    cpSync(join(cold, 'events.jsonl'), join(home, 'events.jsonl'));
    cpSync(join(cold, 'store.db'), join(home, 'store.db'));
    const db = new Database(join(home, 'store.db'));
    db.exec('PRAGMA journal_mode=DELETE');
    db.query("UPDATE meta SET v=? WHERE k='codec'").run(codec);
    if (kind === 'all-projection-missing') db.exec("DELETE FROM setting WHERE k LIKE 'telemetry.%'; DELETE FROM obj WHERE domain='usage'");
    else if (kind === 'usage-only') db.exec("DELETE FROM setting WHERE k LIKE 'telemetry.%'");
    else db.exec("DELETE FROM setting WHERE k='telemetry.usage.generation'; DELETE FROM obj WHERE domain='usage'");
    db.close();
    const oldHash = hash(join(home, 'store.db'));
    const output = await boot(home);
    check(`codec ${codec} ${kind} telemetry cache is retained exactly and rebuilt`, () => {
      assert(!output.includes('upgraded store.db in place'), output);
      const retained = readdirSync(home).filter(n => n.startsWith('store.db.invalid-') && !n.endsWith('-wal') && !n.endsWith('-shm'));
      assert.equal(retained.length, 1);
      assert.equal(hash(join(home, retained[0])), oldHash);
      assert.equal(hash(join(home, 'events.jsonl')), logHash);
      assert.deepEqual(tables(join(home, 'store.db')), expected);
      assert.deepEqual(state(home), projected);
    });
    const before = hash(join(home, 'store.db'));
    await boot(home);
    check(`codec ${codec} ${kind} repaired store reopens without identity or cumulative-counter loss`, () => {
      assert.equal(hash(join(home, 'store.db')), before);
      assert.deepEqual(state(home), projected);
    });
  }
  const parentCold = join(root, 'parent-cold'); mkdirSync(parentCold);
  const parentLog = [
    event('ProjectCreated', { id: 'fixture', title: 'fixture', root: '/tmp/fixture' }),
    ...['parent', 'child'].map(id => event('ThreadCreated', { id, project: 'fixture', title: 'fixture', env: 'local', branch: '', worktree: '', provider: 'fixture', model: 'fixture' })),
    event('TaskDelegated', { task: 'fixture-task', parent: 'parent', child: 'child', depth: 1, provider: 'fixture', model: 'fixture' }),
  ].map(x => JSON.stringify(x)).join('\n') + '\n';
  writeFileSync(join(parentCold, 'events.jsonl'), parentLog);
  await boot(parentCold);
  const parentOld = join(root, 'parent-old'); mkdirSync(parentOld);
  cpSync(join(parentCold, 'events.jsonl'), join(parentOld, 'events.jsonl'));
  cpSync(join(parentCold, 'store.db'), join(parentOld, 'store.db'));
  const parentDb = new Database(join(parentOld, 'store.db'));
  parentDb.query("UPDATE meta SET v='6' WHERE k='codec'").run();
  parentDb.exec("DELETE FROM setting WHERE k LIKE 'thread.parent.%'");
  parentDb.close();
  const parentOutput = await boot(parentOld);
  check('telemetry-empty codec 6 retains the proven parent upgrade in place', () => {
    assert(parentOutput.includes('upgraded store.db in place'), parentOutput);
    assert.deepEqual(tables(join(parentOld, 'store.db')), tables(join(parentCold, 'store.db')));
    assert(!readdirSync(parentOld).some(n => n.startsWith('store.db.invalid-')));
    assert.equal(readFileSync(join(parentOld, 'events.jsonl'), 'utf8'), parentLog);
  });
} finally {
  rmSync(root, { recursive: true, force: true });
  assert(!readdirSync(tmpdir()).includes(root.split('/').at(-1)!));
  console.log('ok isolated store fixtures removed; no hub or sender executed');
}
