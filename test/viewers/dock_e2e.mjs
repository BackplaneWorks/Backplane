// From the worktree: . scripts/lib-bptmp.sh; HUB=<built hub> RIG=$(mktemp -d) bun test/viewers/dock_e2e.mjs
import assert from 'node:assert/strict';
import {appendFileSync, copyFileSync, mkdirSync, mkdtempSync, writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {randomUUID} from 'node:crypto';
import {Client, makeProject, startHub} from './rig.mjs';

assert(process.env.RIG && process.env.HUB, 'set RIG to a scratch directory and HUB to the built hub');
const rig = mkdtempSync(join(process.env.RIG, 'dock-'));
const port = Number(process.env.PORT || 3843);
const projects = ['board', 'other'].map(name => join(rig, name));
for (const path of projects) {
  const git = makeProject(path);
  writeFileSync(join(path, 'ecc83-pp.kicad_pro'), '{}');
  // Intentional unavailable models exercise subscriptions without meshing six boards.
  for (let i = 1; i <= 5; i++) writeFileSync(join(path, 'model' + i + '.kicad_pcb'), 'invalid');
  mkdirSync(join(path, 'mech'), {recursive: true});
  copyFileSync(new URL('../files/fixtures/mesh/bracket.step', import.meta.url), join(path, 'mech/bracket.step'));
  git('add', '.');
  git('commit', '-qm', 'model sources');
}

const hub = await startHub({bin: process.env.HUB, rig, port});
class Desktop extends Client {
  cid = randomUUID();
  view(method, extra) {
    return super.view(method, {cid: this.cid, client: {kind: 'desktop'}, tiles: true, ...extra});
  }
  async select(id) {
    this.target = {kind: 'thread', id};
    this.session.sel = id;
    this.viewId = '';
    return this.view('view.open');
  }
}
const clients = [];
async function connect() {
  const client = new Desktop(port, hub.token);
  clients.push(client);
  await client.connect();
  return client;
}
function checked(reply) {
  assert.equal(reply.ok, true, JSON.stringify(reply.error));
  return reply.result;
}
async function watchSet(c, reply) {
  const commands = checked(reply).cmds.filter(cmd => cmd.type === 'send').map(cmd => JSON.parse(cmd.text));
  const sets = commands.filter(cmd => cmd.m === 'kicad.watch.set');
  assert.equal(sets.length, 1, 'expected one authoritative watch set for ' + JSON.stringify({layout: c.thread().viewer.layout, commands: commands.map(cmd => cmd.m), notes: reply.result.cmds.filter(cmd => cmd.type === 'say')}));
  async function answer(id) {
    const match = frame => frame.t === 'reply' && frame.id === id;
    return checked(c.frames.findLast(match) || await c.wait(match));
  }
  const {id, p} = sets[0];
  for (const write of commands.filter(cmd => cmd.m === 'setting.set')) await answer(write.id);
  const result = await answer(id);
  return {keys: result.tiles, sources: p.sources, root: p.root};
}
const add = (c, kind, source) => c.action('layout-add', JSON.stringify({kind, side: 'right', source}), 0);
const tiles = c => Object.values(c.thread().viewer.tiles);
async function refused(c, promise, code, layout) {
  const result = checked(await promise);
  assert(result.cmds.some(cmd => cmd.type === 'say' && cmd.code === code), code);
  assert.deepEqual(c.thread().viewer.layout, layout, 'refusal changed layout');
}

try {
  let c = await connect();
  for (const path of projects) checked(await c.rpc('project.add', {path}));
  const home = checked(await c.view('view.open')).shown;
  const [first, other] = projects.map(path => home.projects.find(p => p.root === path).threads[0].id.split('|').pop());
  checked(await c.select(first));
  checked(await c.action('view', 'board', 0));
  await watchSet(c, await add(c, 'schematic'));
  const original = await watchSet(c, await add(c, '3d'));
  const byKind = Object.fromEntries(tiles(c).map(t => [t.kind, t.id]));
  const expected = kind => kind + '|' + original.root + '|';
  assert.deepEqual(original.keys[byKind['3d']].slice().sort(), [expected(0), expected(2)]);
  assert.deepEqual(original.keys[byKind.schematic], [expected(1)]);
  assert.deepEqual(original.keys[byKind.board], [expected(0)]);
  const saved = c.thread().viewer.layout;

  // A new connection has no session drafts. Selection must restore the saved project layout.
  c = await connect();
  const restored = await watchSet(c, await c.select(first));
  // Project memory stores the arrangement; a new session focuses its first leaf.
  assert.deepEqual(c.thread().viewer.layout, {...saved, focus: byKind.board, maximized: null});
  assert.deepEqual(restored.keys, original.keys);
  checked(await c.action('layout-focus', JSON.stringify({tile: byKind['3d']}), 0));
  const retry = await watchSet(c, await c.action('view', 'retry', 0));
  assert.deepEqual(retry.keys, original.keys);
  assert(retry.sources.some(src => src.force));
  assert(retry.sources.every(src => !!src.force === (src.tile === byKind['3d'])));

  // The desktop follows a key with a plain watch. A subsequent source change must
  // still reach this connection on its unfocused schematic subscription.
  const revision = f => f.t === 'plotrevision' && f.key === expected(1) && f.revision;
  const before = c.frames.findLast(revision) || await c.wait(revision, 30000);
  checked(await c.rpc('kicad.watch', {kind: '3d', root: original.root, path: ''}));
  const changed = c.wait(f => revision(f) && f.revision !== before.revision, 30000);
  appendFileSync(join(original.root, 'ecc83-pp.kicad_sch'), '\n');
  await changed;
  const closed = await watchSet(c, await c.action('layout-close', JSON.stringify({tile: byKind.schematic}), 0));
  const remaining = {...original.keys};
  delete remaining[byKind.schematic];
  assert.deepEqual(closed.keys, remaining);
  assert.equal(c.thread().viewer.layout.focus, byKind['3d']);

  checked(await c.select(other));
  checked(await c.action('view', 'schematic', 0));
  assert.deepEqual((await watchSet(c, await c.select(first))).keys, remaining);

  const mech = await watchSet(c, await add(c, 'mech', 'mech/bracket.step'));
  const part = tiles(c).find(tile => tile.kind === 'mech');
  assert(mech.keys[part.id].some(key => key.endsWith('bracket.step')));
  // Retry is session-rate-limited, including retries on a different tile.
  const retryAt = Number(c.session.drafts['@view.retry.at'] || 0);
  await Bun.sleep(Math.max(0, retryAt + 1001 - Date.now()));
  const retried = await watchSet(c, await c.action('view', 'retry', 0));
  assert.deepEqual(retried.keys, mech.keys);
  assert(retried.sources.some(src => src.tile === part.id && src.force));
  const boardRevision = f => f.t === 'plotrevision' && f.key === expected(0) && f.revision;
  const priorBoard = c.frames.findLast(boardRevision) || await c.wait(boardRevision, 30000);
  checked(await c.rpc('kicad.watch', {kind: '3d', root: original.root, path: join(original.root, 'mech/bracket.step')}));
  const nextBoard = c.wait(f => boardRevision(f) && f.revision !== priorBoard.revision, 30000);
  appendFileSync(join(original.root, 'ecc83-pp.kicad_pcb'), '\n');
  await nextBoard;

  while (tiles(c).length < 8) {
    const count = tiles(c).length;
    checked(await add(c, 'files'));
    assert.equal(tiles(c).length, count + 1);
  }
  await refused(c, add(c, 'files'), 'tooManyTiles', c.thread().viewer.layout);

  checked(await c.select(other));
  checked(await c.action('view', '3d', 0));
  let keys;
  for (let i = 1; i <= 5; i++) {
    keys = (await watchSet(c, await add(c, '3d', 'model' + i + '.kicad_pcb'))).keys;
  }
  assert.equal(new Set(Object.values(keys).flat()).size, 12);
  await refused(c, add(c, 'schematic'), 'tooManySources', c.thread().viewer.layout);
  console.log('Dock restore, watch membership, thread switch and both limits passed.');
} finally {
  for (const client of clients) client.close();
  hub.stop();
}
