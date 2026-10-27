// node trial.mjs PORT TOKEN HOME ROOT : fill a hub.sh hub with a workspace
// that looks used: five projects, about twenty threads of 1-5 exchanges,
// three long ones (24-36), replies with tool calls, and some threads pinned,
// settled, snoozed or archived. Run through `hub.sh trial NAME`.
import { readFileSync } from 'node:fs';
import { connect } from './rpc.mjs';

const [port, token, home, root] = process.argv.slice(2);
const events = () => readFileSync(`${home}/events.jsonl`, 'utf8');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const hub = await connect(port, token);
const projectId = (dir) => {
  const title = dir.split('/').pop();
  const m = [...events().matchAll(/"\$":"ProjectCreated","id":"([^"]*)","title":"([^"]*)"/g)].find((x) => x[2] === title);
  if (!m) throw new Error(`no project ${title}`);
  return m[1];
};
// turns of thread t that have ended (any state but running)
const ends = (t) => events().split(`"$":"TurnChanged","thread":"${t}","state":"`).slice(1)
  .filter((s) => !s.startsWith('running')).length;

// what a person asks; the last word tags the stand-in's reply
const asks = [
  'Can you check the decoupling near the regulator? tools:2 decoupling',
  'Walk me through the power tree from the input jack. power',
  'Are the differential pairs length matched? tools:3 pairs',
  'Move the test points to the bottom side. tools:1 testpoints',
  'What does DRC still complain about? tools:2 drc',
  'Summarise what changed since this morning. summary',
  'Is the ground pour stitched well enough? tools:1 stitching',
  'Pick a cheaper part for the 10 uF caps. caps',
  'Check the silkscreen for overlaps. tools:2 silk',
  'Why is the ratsnest still showing two nets? nets',
];
let k = 0;
const ask = () => asks[k++ % asks.length];

const plan = [
  ['proj', 'Route the USB-C connector', 30, 'pin'],
  ['proj', 'Check decoupling on U3', 3],
  ['proj', 'BOM cleanup', 2, 'archive'],
  ['proj', 'Silkscreen fixes', 1, 'settle'],
  ['proj', 'Why does DRC flag the via?', 4],
  ['multi', 'Alpha: power tree review', 24],
  ['multi', 'Beta: swap the MCU', 3],
  ['multi', 'Compare the alpha and beta stackups', 2],
  ['amp', 'Bias the tube stage', 5],
  ['amp', 'Lower the hum', 2, 'settle'],
  ['amp', 'Grounding plan', 3],
  ['sensor-node', 'Bring-up notes', 36, 'pin'],
  ['sensor-node', 'I2C pull-ups', 2],
  ['sensor-node', 'Battery life estimate', 3],
  ['sensor-node', 'Antenna keepout', 1, 'settle'],
  ['sensor-node', 'Firmware pin map', 2],
  ['psu', 'Buck converter layout', 4],
  ['psu', 'Thermal relief question', 1, 'settle'],
  ['psu', 'Output ripple', 2, 'snooze'],
];

for (const dir of ['amp', 'sensor-node', 'psu']) await hub.call('project.add', { path: `${root}/${dir}` });
const threads = [];
for (const [dir, title, n, then] of plan) {
  const { thread } = await hub.call('thread.create', { project: projectId(dir), title });
  threads.push({ thread, title, n, then });
}

// each thread's exchanges in order; at most 6 threads at once
const run = async (t) => {
  for (let i = 1; i <= t.n; i++) {
    const before = ends(t.thread);
    await hub.call('turn.start', { thread: t.thread, text: ask(), msg: `trial-${t.thread}-${i}`, mode: '' });
    const until = Date.now() + 60000;
    while (ends(t.thread) <= before) {
      if (Date.now() > until) throw new Error(`${t.title}: turn ${i} never ended`);
      await sleep(200);
    }
  }
};
const todo = [...threads];
await Promise.all(Array.from({ length: 6 }, async () => { for (let t; (t = todo.shift());) await run(t); }));

for (const t of threads) {
  // the stand-in's answers may have retitled it
  await hub.call('thread.rename', { thread: t.thread, title: t.title });
  if (t.then === 'pin') await hub.call('thread.pin', { thread: t.thread });
  // a finished turn settles its thread by itself; only the planned ones stay settled
  await hub.call(t.then === 'settle' ? 'thread.settle' : 'thread.unsettle', { thread: t.thread });
  if (t.then === 'archive') await hub.call('thread.archive', { thread: t.thread });
  if (t.then === 'snooze') await hub.call('thread.snooze', { thread: t.thread, for: '1h' });
}
for (const t of threads) console.log(`${t.thread}\t${t.n}\t${t.then ?? ''}\t${t.title}`);
hub.close();
process.exit(0);
