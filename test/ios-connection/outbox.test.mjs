// Exercises the actual native thin host without a real hub or credentials.
import assert from 'node:assert/strict';
import { NativeThinHost307265 as Host } from '../../src/mobile/thin-native-host-307265.js';

const host = () => new Host({ now: () => 1000, command: (hub, command) => ({ hub, ...command }),
  session: () => ({ rid: 1, sel: '', drafts: {}, open: {}, info: {} }),
  target: () => ({ kind: 'recent', id: '' }) }, 4000000000000);
const queue = (h, hub, msg) => h.outboxKeep(hub, { id: 1, m: 'turn.start', p: { msg, thread: 'fixture', text: 'fixture' } });
const sends = h => h.drain().cmds.filter(c => c.type === 'send');

{
  const h = host();
  h.hubs(['a', 'b']); h.connection('a', 'old', 'view-v1');
  h.state('a').session.sel = 'old-private-choice';
  queue(h, 'a', 'a1'); queue(h, 'b', 'b1');
  h.disconnect('a'); h.outboxFlush('a');
  assert.equal(sends(h).length, 1, 'ordinary network reconnect retains the same hub outbox');
  h.enqueue('a', { type: 'keep', thread: 'a|fixture', text: 'old' }, { type: 'jump', id: 'old' });
  h.enqueue('b', { type: 'keep', thread: 'b|fixture', text: 'other' });
  h.forgetHub('a'); h.hubs(['a', 'b']); h.outboxFlush('a');
  const remaining = h.drain().cmds;
  assert.deepEqual(remaining, [{ type: 'keep', thread: 'b|fixture', text: 'other' }], 'retirement discards every old command but keeps other hubs');
  h.connection('a', 'new', 'view-v1');
  assert.equal(h.state('a').session.sel, '', 'same-address re-pair discards old presentation choices');
  assert.equal(h.receive('a', { t: 'hb' }, 'old'), false, 'late old-epoch frame is rejected');
  h.outboxFlush('b');
  assert.equal(sends(h).length, 1, 'forgetting one hub preserves another hub outbox');
}
{
  const h = host();
  h.hubs(['a']); queue(h, 'a', 'a1'); h.hubs([]); h.hubs(['a']); h.outboxFlush('a');
  assert.equal(sends(h).length, 0, 'unpair followed by re-pair retires the outbox');
}
console.log('PASS native outbox session isolation and reconnect');
