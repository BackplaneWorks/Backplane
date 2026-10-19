// Current production bridge + synthetic events only. Returned RPCs stay inert.
// bun test/tools/ios_lifetime_state.ts BRIDGE OUT
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import vm from 'node:vm';
import { encode } from './ios_test_cbor.ts';

const [bridge, out] = process.argv.slice(2);
if (!bridge || !out) throw Error('BRIDGE OUT');
const bytes = readFileSync(bridge);
const context = vm.createContext({ console });
vm.runInContext('Date.now = () => 1792389000000;', context);
vm.runInContext(bytes.toString('utf8'), context, { filename: bridge });
const B = context.Backplane;
assert.ok(B && typeof B.save === 'function', 'production Backplane bridge');
let discardedCommands = 0;
const answer = (text: string) => {
  const value = JSON.parse(text);
  discardedCommands += Array.isArray(value.cmds) ? value.cmds.length : 0;
  return value;
};
const key = 'fixture:3897', id = 'lifetime-thread';
const events = [
  { $: 'ProjectCreated', id: 'lifetime-project', title: 'Synthetic lifetime project', root: '/synthetic', at: 1 },
  { $: 'ThreadCreated', id, project: 'lifetime-project', title: 'Synthetic lifetime thread', env: 'local', branch: '', worktree: '', provider: 'claude', model: '', at: 2 },
  { $: 'MessagePosted', thread: id, id: 'lifetime-message', role: 'user', text: 'Synthetic lifetime text 日本語', at: 3 },
];
answer(B.start('isolated-lifetime-fixture', '{}'));
answer(B.hubs([key]));
answer(B.online(key, true, false));
const frame: number[] = [];
encode({ t: 'log', origin: 'synthetic-lifetime-origin', more: false, boot: false, items: events,
  info: { machines: JSON.stringify({ self: { name: 'Synthetic machine' } }) } }, frame);
answer(B.recv(key, Buffer.from(frame).toString('base64'), false, false));
const selected = answer(B.act('select', `${key}|${id}`));
assert.equal(selected.screen?.thread?.id, `${key}|${id}`, 'synthetic canonical thread selected');
assert.ok(selected.screen.thread.entries.length > 0, 'synthetic message rendered');
const state = B.save();
assert.ok(typeof state === 'string' && state.length > 0);
const restored = answer(B.load(state));
assert.equal(restored.screen?.thread?.id, `${key}|${id}`, 'current-type kept state restores selection');
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, state);
const sha = (value: Uint8Array | string) => createHash('sha256').update(value).digest('hex');
const metadata = { fixture: 'original synthetic ProjectCreated/ThreadCreated/MessagePosted', events: events.length,
  selectedThread: `${key}|${id}`, bridgeSha256: sha(bytes), stateSha256: sha(state),
  commandsDiscarded: discardedCommands, rpcExecution: false };
writeFileSync(`${out}.metadata.json`, JSON.stringify(metadata, null, 2) + '\n');
console.log(JSON.stringify(metadata));
