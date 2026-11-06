// What else answers PRESENTER_UNAVAILABLE on a helper-less hub: the other tabs and pages, each pressed on a fresh view, then updated (their reads arrive with the update).
import {startHub, Client, makeProject, sleep} from './rig.mjs';
const BIN = process.env.HUB, RIG = process.env.RIG, PORT = +(process.env.PORT || 3841), P = RIG + '/proj';
makeProject(P); const hub = await startHub({bin: BIN, rig: RIG, port: PORT});
const c = new Client(PORT, hub.token); await c.connect();
const say = (n, r, u) => console.log(n.padEnd(22), r.ok ? 'ok     ' : 'REFUSED ' + (r.error?.code || JSON.stringify(r.error)), '| update', u.ok ? 'ok' : 'REFUSED ' + (u.error?.code || ''));
try {
  await c.view('view.open'); await c.rpc('project.add', {path: P}); await sleep(800); await c.update(); const id = await c.selectFirstThread();
  for (const [a, v] of [['view', 'files'], ['view', 'mech'], ['mech', ''], ['view', 'split'], ['fm', ''], ['settings', ''], ['view', 'board'], ['view', '']]) {
    const r = await c.action(a, v, 900); const u = await c.update(900); say(a + ' ' + v, r, u);
    // a refused view leaves the next one on the old session: start each from the thread again
    if (!r.ok || !u.ok) { c.target = {kind: 'thread', id}; c.session.sel = id; c.viewId = ''; await c.view('view.open'); }
  }
} finally { c.close(); hub.stop(); }
