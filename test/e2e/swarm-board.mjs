// A swarm with a busy board on a rig hub (test/e2e/gpui-i18n-rig.sh up en), for looking at the swarm page and its sidebar row in a real window.
// Stand-in agents only: the members are the claude stand-in, and each of their board posts is a real room_post MCP call it makes ("mcpcall:").
//   bun test/e2e/swarm-board.mjs R       R: the rig's folder (its env, home/token). Prints the room's id.
import {readFileSync} from 'node:fs';
import {sock, sleep, waitFor} from './lib/hubrows.mjs';
const R = process.argv[2]; if (!R) { console.error('usage: swarm-board.mjs RIG_DIR'); process.exit(2) }
const env = Object.fromEntries(readFileSync(R + '/env', 'utf8').split(/[;\n]/).map(s => s.trim()).filter(Boolean).map(s => s.split('=')));
const c = await sock({token: readFileSync(R + '/home/token', 'utf8').trim()}, Number(env.PORT));
const call = async (m, p) => { const r = await c.rpc(m, p, 30000); if (!r.ok) { console.error(m, JSON.stringify(r)); process.exit(1) } return r.result };
const made = await call('swarm.create', {project: 'p1', topic: 'Encoder debounce', goal: 'A debounced encoder button with no missed or double steps',
  members: [{provider: 'claude', model: 'opus', role: 'designer'}, {provider: 'claude', model: 'sonnet', role: 'critic'}, {provider: 'claude', model: 'haiku', role: 'tester'}],
  seed: 'The encoder button in src/input.rs double-steps on fast turns. Agree a fix, then make it.'});
const room = made.room; console.log('room', room);
const board = () => call('swarm.get', {room});
const members = (await board()).members;
const th = name => members.find(m => m.name === name).thread;
// each member's first turn (the brief and the seed) ends before it is asked to post
await waitFor(async () => (await board()).members.every(m => m.state === 'idle'), 60000, 500);
const post = async (name, args, want) => {
  const before = (await board()).posts.length;
  await call('message.send', {thread: th(name), text: 'mcpcall:' + JSON.stringify({name: 'room_post', arguments: {room, ...args}})});
  await waitFor(async () => (await board()).posts.length > before + (want ?? 0), 30000, 300);
  await waitFor(async () => (await board()).members.find(m => m.name === name).state === 'idle', 30000, 300);
  await sleep(400);
};
await post('designer', {text: 'Idea: sample both encoder lines every 1 ms and run a 4-state Gray-code machine; only a full detent counts as a step.', tags: ['idea']});
await post('critic', {text: '@designer a 1 ms tick alone misses bursts at full speed. Take the edge interrupt and confirm in the tick, or we drop steps.', tags: ['critique']});
await call('swarm.post', {room, text: 'Keep the timer at 1 ms, the MCU is busy elsewhere. @tester, can you write the fast-turn test first?'});
await sleep(600);
await post('designer', {text: 'Taking the state machine.', tags: ['claim'], files: ['src/input.rs']});
await post('tester', {text: 'Writing the fast-turn test; it needs input.rs too.', tags: ['claim'], files: ['src/input.rs', 'tests/encoder.rs']});
await post('tester', {text: 'Question: what counts as fast? I will take 600 rpm with 24 detents.', tags: ['question']});
console.log(JSON.stringify((await board()).claims));
c.ws.close(); process.exit(0);
