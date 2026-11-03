import assert from 'node:assert/strict';
import { SecretSave } from '../../src/web/secret-save.js';
let now = 0;
const dummy = 'DUMMY-SECRET-BOX-123456';
const build = packed => JSON.stringify({ rpc: JSON.stringify({ id: 0, m: 'secret.set', p: { value: packed } }) });
const saves = new SecretSave(build, () => now);
let sent = saves.begin(dummy, 'hub-a', 'bot:bub', '{"id":"fixture","epoch":1}');
assert.equal(sent.rpc.p.accountBinding, '{"id":"fixture","epoch":1}');
assert(sent.id >= 2 ** 44 && sent.id < 2 ** 45, 'secure IDs cannot collide with ImprovementsRpc');
assert.equal(sent.rpc.p.value, dummy);
assert(!JSON.stringify([...saves.pending]).includes(dummy), 'only metadata waits for ACK');
assert.equal(saves.begin(dummy, 'hub-a', 'bot:bub').busy, true);
assert.equal(saves.receive('hub-b', { t: 'reply', id: sent.id, ok: true }), null);
assert.equal(saves.receive('hub-a', { t: 'reply', id: sent.id + 1, ok: true }), null);
assert.equal(saves.receive('hub-a', { t: 'reply', id: sent.id, ok: false, error: 'vault is locked' }).value, 'vault is locked');
assert.equal(saves.deadline(), null);
sent = saves.begin(dummy, 'hub-a', 'bot:bub');
assert.equal(saves.receive('hub-a', { t: 'reply', id: sent.id, ok: true }).action, 'secret-sent');
assert.equal(saves.receive('hub-a', { t: 'reply', id: sent.id, ok: true }), null);
saves.begin(dummy, 'hub-a', 'bot:bub');
assert.equal(saves.expire().length, 0);now = 15000;
assert.match(saves.expire()[0].value, /did not confirm/);
saves.begin(dummy, 'hub-a', 'bot:bub');
assert.match(saves.cancel('hub-a')[0].value, /Disconnected/);
assert.equal(saves.pending.size, 0);
assert.match(new SecretSave(() => { throw Error(dummy); }).begin(dummy, 'h', 'b').error, /could not be sent/);
assert(!new SecretSave(() => { throw Error(dummy); }).begin(dummy, 'h', 'b').error.includes(dummy));
assert.equal(new SecretSave(() => JSON.stringify({ error: 'Paste the secret' })).begin('', 'h', 'b').error, 'Paste the secret');
console.log('ok secure Save: ACK, refusal, build failure, timeout, disconnect, duplicate and stale reply; pending metadata has no value');

// The actual browser adapter: no optimistic close, safe exception text, and
// an offline redraw keeps its inline error. Navigation cannot close a new box.
const { SecretSaveHost } = await import('../../src/web/secret-save.js');
const calls=[], sentRPCs=[];let scope='bot:bub', note='', connected=true;
const host=new SecretSaveHost({build,scope:()=>scope,binding:()=>'{"id":"fixture","epoch":1}',
  send:rpc=>{if(!connected)throw Error(dummy);sentRPCs.push(rpc);},dispatch:(...args)=>calls.push(args),note:text=>note=text});
host.save(dummy);assert.equal(calls.length,0);assert.equal(sentRPCs[0].p.accountBinding,'{"id":"fixture","epoch":1}');
host.receive({t:'reply',id:sentRPCs[0].id,ok:false,error:'vault is locked'});
assert.equal(note,'vault is locked');note='redrawn';host.paint();assert.equal(note,'vault is locked');
host.save(dummy);scope='thread:other';host.receive({t:'reply',id:sentRPCs[1].id,ok:true});assert.equal(calls.length,1);
scope='bot:bub';connected=false;host.save(dummy);assert.match(note,/could not be sent/);assert(!note.includes(dummy));
console.log('ok browser adapter: captured owner, ACK-only close, inline refusal, offline redraw and navigation');
