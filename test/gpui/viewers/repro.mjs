// Step 1: Diff / Review / a board on a hub started as the person's (no --policy-bin). Prints what each view request answers.
import {startHub, Client, makeProject, changeProject, sleep} from './rig.mjs';
const BIN = process.env.HUB, RIG = process.env.RIG, PORT = +(process.env.PORT || 3841);
const git = makeProject(RIG + '/proj'); changeProject(RIG + '/proj');
const hub = await startHub({bin: BIN, rig: RIG, port: PORT});
console.log('hub cmdline:', hub.cmd, '| children:', JSON.stringify(hub.children), '|', hub.agentsEnv);
const c = new Client(PORT, hub.token); await c.connect();
try {
  await c.view('view.open'); await sleep(500);
  const add = await c.rpc('project.add', {path: RIG + '/proj'}); console.log('project.add', add.ok);
  await sleep(800); await c.update();
  const id = await c.selectFirstThread(); console.log('thread', id, c.last.ok ? 'ok' : JSON.stringify(c.last).slice(0, 300));
  for (const [a, v] of [['diff', ''], ['view', 'board'], ['view', 'review'], ['rv-page', 'bom']]) {
    const r = await c.action(a, v); const u = await c.update(800);
    const t = c.thread();
    console.log(a, v, '->', r.ok ? 'ok' : JSON.stringify(r.error), '| update', u.ok ? 'ok' : JSON.stringify(u.error), '| diff', t?.diff ? JSON.stringify(t.diff).slice(0, 200) : t?.diff, '| viewer.open', t?.viewer?.open);
  }
} catch (e) { console.log('ERR', e.message) } finally { c.close(); hub.stop(); }
console.log(require('node:fs').readFileSync(RIG + '/hub.log', 'utf8').split('\n').filter(l => /not ported|PRESENT|refus/i.test(l)).slice(0, 8).join('\n'));
