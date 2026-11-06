// Run ONLY on Boat. No installed binary, live home, log, token or agent is used.
// The fixture is a small isolated hub around production planning and IO sinks.
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, symlinkSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFileSync } from 'node:child_process';
const [fixture, mode = 'fixed'] = process.argv.slice(2);
assert.equal(process.env.BACKPLANE_HEAVY_BOAT_SANDBOX, '1');
const dir = mkdtempSync(join(tmpdir(), 'bp-thread-folder-http-'));
const root = join(dir, 'project'), home = join(dir, 'hub'), cwd = join(dir, 'cwd'), outside = join(dir, 'outside');
for (const p of [root, home, cwd, outside, join(root, 'etc'), join(root, 'empty')]) mkdirSync(p, { recursive: true });
writeFileSync(join(root, 'note.txt'), 'inside\n'); writeFileSync(join(root, 'etc/note.txt'), 'subfolder\n');
writeFileSync(join(outside, 'model.step'), 'synthetic outside model');
writeFileSync(join(outside, 'note.txt'), 'outside\n'); writeFileSync(join(cwd, 'cwd.txt'), 'original\n');
mkdirSync(join(cwd, 'mech')); writeFileSync(join(cwd, 'mech/cwd.step'), 'fixture');
symlinkSync(outside, join(root, 'escape')); symlinkSync(join(outside, 'note.txt'), join(root, 'escape.txt'));
const git = (...args: string[]) => execFileSync('git', args, { cwd, encoding: 'utf8' }).trim();
git('init', '-q'); git('config', 'user.name', 'Fixture'); git('config', 'user.email', 'fixture@example.invalid');
git('add', '.'); git('commit', '-qm', 'fixture'); const initial = git('rev-parse', 'HEAD');
const listener = Bun.listen({ hostname: '127.0.0.1', port: 0, socket: { data() {} } });
const port = listener.port; listener.stop(true); assert(port >= 3799);
const child = Bun.spawn(['bun', resolve(fixture)], { cwd, env: { ...process.env, FILES_TEST_ROOT: root, FILES_TEST_HOME: home, FILES_TEST_PORT: String(port), BACKPLANE_HOME: home, BACKPLANE_PEERS: '', BACKPLANE_NO_UPDATE: '1' }, stdout: 'pipe', stderr: 'pipe' });
let output = '', errors = '';
const collect = async (stream: ReadableStream<Uint8Array>, error = false) => { for await (const bytes of stream) { const text = new TextDecoder().decode(bytes); if (error) errors += text; else output += text; } };
const readers = [collect(child.stdout), collect(child.stderr, true)];
const report: any[] = [];
try {
  const until = Date.now() + 30000;
  while (!output.includes('THREAD_FOLDER_HTTP_READY')) {
    if (child.exitCode !== null || Date.now() > until) throw Error('fixture failed: ' + output + errors);
    await Bun.sleep(25);
  }
  const request = async (route: string, values: Record<string, string>, auth = true) => {
    const r = await fetch(`http://127.0.0.1:${port}${route}?${new URLSearchParams(values)}`, { headers: auth ? { Authorization: 'Bearer paired-fixture' } : {}, signal: AbortSignal.timeout(15000) });
    return { status: r.status, body: await r.text() };
  };
  const rpc = async (method: string, thread: string, path = '', extra: Record<string, string> = {}) => {
    const r = await request('/rpc', { method, thread, path, ...extra }); assert.equal(r.status, 200, r.body); return JSON.parse(r.body);
  };
  assert.equal((await request('/rpc', { method: 'files.dir', thread: 'missing' }, false)).status, 401);
  for (const thread of ['missing', '', 'deleted']) {
    for (const path of ['', 'etc', '..', '../etc']) {
      const r = await rpc('files.dir', thread, path);
      report.push({ method: 'files.dir', thread, path, ok: r.ok, root: r.result?.fmRoot === '' ? '<empty>' : r.result?.fmRoot ? '<project>' : undefined, listing: !!r.result?.fmEntries });
      if (mode === 'fixed') { assert.equal(r.ok, false); assert(!r.result?.fmEntries); }
      if (mode === 'before' && path === '' && thread !== 'deleted') { assert.equal(r.ok, true); assert.equal(r.result.fmRoot, ''); assert(r.result.fmEntries.includes('etc')); }
    }
    writeFileSync(join(cwd, 'cwd.txt'), 'dirty read fixture\n');
    for (const method of ['files.list', 'git.status', 'git.diff', 'mech.list', 'mech.render', 'boards.list', 'history.refs', 'kicad.open', 'kicad.export10', 'review.bom', 'review.checks', 'term.open', 'turn.start']) {
      // For non-file/Git sinks the adapter reports the production dispatch plan;
      // it never starts an agent, terminal, KiCad, renderer or release job.
      const r = await rpc(method, thread); report.push({ method, thread, ok: r.ok, cwdStatus: String(r.result?.status ?? '').includes('cwd.txt'), cwdMech: String(r.result?.mech ?? '').includes('cwd.step') });
      if (mode === 'fixed') assert.equal(r.ok, false, method + ' ' + thread);
      if (mode === 'before' && method === 'git.status' && thread !== 'deleted') assert(String(r.result.status).includes('cwd.txt'));
      if (mode === 'before' && method === 'mech.list' && thread !== 'deleted') assert(String(r.result.mech).includes('cwd.step'));
    }
    for (const route of ['/view/get', '/files/get', '/files/zip']) {
      const r = await request(route, { thread, path: route.endsWith('zip') ? '' : 'note.txt' }); report.push({ route, thread, status: r.status });
      if (mode === 'fixed' || thread !== 'deleted') assert(r.status >= 400, route + thread);
    }
    const owner = thread || 'empty-owner-check';
    const ref = 'refs/backplane/checkpoints/' + Buffer.from(owner).toString('base64url') + '/turn/0';
    if (thread) git('update-ref', ref, initial);
    writeFileSync(join(cwd, 'cwd.txt'), 'changed\n');
    const reverted = await rpc('git.revert', thread);
    const cwdReverted = readFileSync(join(cwd, 'cwd.txt'), 'utf8') === 'original\n';
    report.push({ method: 'git.revert', thread, ok: reverted.ok, cwdReverted });
    if (mode === 'fixed') { assert.equal(reverted.ok, false); assert.equal(cwdReverted, false); }
    if (mode === 'before' && thread === 'missing') assert.equal(cwdReverted, true, 'baseline revert writes synthetic hub cwd');
    writeFileSync(join(cwd, 'cwd.txt'), 'original\n'); if (thread) git('update-ref', '-d', ref);
    const cp = await request('/checkpoint', { thread }); const cpr = JSON.parse(cp.body);
    const cwdCheckpoint = !!git('for-each-ref', '--format=%(refname)', 'refs/backplane/checkpoints/');
    report.push({ method: 'internal checkpoint', thread, ok: cpr.ok, cwdCheckpoint });
    if (mode === 'fixed') { assert.equal(cpr.ok, false); assert.equal(cwdCheckpoint, false); }
    for (const r of git('for-each-ref', '--format=%(refname)', 'refs/backplane/checkpoints/').split('\n').filter(Boolean)) git('update-ref', '-d', r);
    const headBeforeAct = git('rev-parse', 'HEAD');
    writeFileSync(join(cwd, 'cwd.txt'), 'commit fixture ' + thread + '\n');
    const act = await rpc('git.act', thread, '', { action: 'commit' });
    const cwdCommitted = git('rev-parse', 'HEAD') !== headBeforeAct;
    report.push({ method: 'git.act', thread, ok: act.ok, cwdCommitted });
    if (mode === 'fixed') { assert.equal(act.ok, false); assert.equal(cwdCommitted, false); }
    if (mode === 'before' && thread !== 'deleted') assert.equal(cwdCommitted, true);
  }
  for (const thread of ['t', 'archived']) {
    for (const path of ['', 'etc', 'empty']) assert.equal((await rpc('files.dir', thread, path)).ok, true, thread + path);
    for (const path of ['..', '../outside', '/etc', 'escape', 'etc/../../outside']) assert.equal((await rpc('files.dir', thread, path)).ok, false, path);
    for (const route of ['/view/get', '/files/get']) {
      const valid = await request(route, { thread, path: 'note.txt' }); assert.equal(valid.status, 200); assert.equal(valid.body, 'inside\n');
      for (const path of ['../outside/note.txt', 'escape/note.txt', 'escape.txt']) assert((await request(route, { thread, path })).status >= 400, route + path);
    }
    if (mode === 'fixed') {
      assert(!(await rpc('files.dir', thread)).result.fmEntries.includes('escape'));
      for (const path of ['../outside/model.step', join(outside, 'model.step'), 'escape/model.step', 'C:/outside/model.step', 'C:\\outside\\model.step'])
        assert.equal((await rpc('mech.render', thread, path)).ok, false, path);
    }
  }
  for (const thread of ['missing', '', 'deleted']) {
    const plot = await rpc('kicad.watch', thread, '', { kind: 'board' });
    report.push({ method: 'kicad.watch', thread, unrooted: true, ok: plot.ok });
    if (mode === 'fixed') assert.equal(plot.ok, false);
    const imageRelative = await request('/img', { thread, path: 'cwd.txt' });
    report.push({ route: '/img', thread, relative: true, status: imageRelative.status, cwdRead: imageRelative.body === readFileSync(join(cwd, 'cwd.txt'), 'utf8') });
    if (mode === 'fixed') assert.equal(imageRelative.status, 404);
    const r = await request('/img', { thread, path: join(outside, 'note.txt') });
    report.push({ route: '/img', thread, status: r.status, independentAbsolutePath: r.body === 'outside\n' });
  }
  console.log(JSON.stringify({ mode, report }, null, 2));
  console.log('ok isolated production folder boundary HTTP matrix ' + mode);
} finally { child.kill(); await child.exited; await Promise.all(readers); rmSync(dir, { recursive: true, force: true }); }
