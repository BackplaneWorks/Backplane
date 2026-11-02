const assert = require('node:assert/strict');
const { test } = require('node:test');

test('sent previews bridge asynchronous persistence without gaps or duplicates', async () => {
const { ThinHost } = await import(process.env.THIN_HOST_MODULE || '../../src/web/thin-host.js');
const { ThinViews, VIEW_LIMITS } = await import('../../src/web/thin.js');

const clone = value => structuredClone(value);
const timeline = kids => ({ type: 'element', tag: 'div', key: 'timeline', attrs: [{ name: 'id', value: 'timeline' }], kids });
const user = id => ({ type: 'element', tag: 'bp-memo', key: id, attrs: [{ name: 'fn', value: 'msg' }, { name: 'who', value: 'u' }], kids: [] });
const sending = () => ({ type: 'element', tag: 'div', key: '', attrs: [{ name: 'class', value: 'msg user sending' }], kids: [] });
function setup(route) {
  const sent = [], commands = [], frames = [], feedback = [], settled = [];
  let clock = 0;
  const initial = { sel: 'a', rid: 10, drafts: { a: 'same message' }, outbox: {}, open: {}, info: {}, ...(route ? { route } : {}) };
  const h = new ThinHost({ hub: 'h', now: () => clock, cid: 'fixture', session: () => clone(initial),
    target: s => s.route ? { kind: 'bot', id: s.route.bot } : { kind: 'thread', id: s.sel }, counter() {}, send: r => sent.push(r),
    prepare: s => s, choice: (s, action, value) => ({ ...s, ...(action === 'select' ? { sel: value, route: undefined } : {}),
      drafts: action === 'draft' ? { ...s.drafts, [s.sel]: value } : s.drafts }),
    local() {}, sessionChanged() {}, decode: x => x, command: c => commands.push(c),
    view: (_v, detail) => frames.push({ ids: detail.window.ids, pending: h.pendingSends(h.session.sel) }),
    state() {}, anchor: () => ({ id: '', offset: 0 }), ack: s => s, blocked() {}, warning() {},
    previewSend: text => !!text, sendFeedback: (...args) => feedback.push(args),
  }, ThinViews);
  const reply = (request, ids = [], cmds = []) => ({ t: 'reply', id: request.id, ok: true, result: {
    version: 1, hub: 'h', generation: '3', epoch: 'e', requestRevision: request.p.requestRevision,
    target: clone(request.p.target), viewId: request.p.viewId || 'v', viewRevision: '1', renderer: 'web', remoteReady: true,
    session: clone(h.session), shown: timeline(ids.map(user)),
    window: { ids: ids.map((_, i) => String(i + 1)), older: '', hasMore: false, anchor: { id: '', offset: 0 }, partial: [] }, cmds,
  } });
  function send(msg = 'm1', method = route ? 'bots.say' : 'turn.start') {
    h.request('send', '', ok => settled.push(ok));
    const request = sent.at(-1), rpc = { id: h.session.rid + 1, m: method, p: { thread: 'a', bot: 'b', text: request.p.session.drafts.a, msg } };
    const actionReply = reply(request, [], [{ type: 'send', text: JSON.stringify(rpc) }, { type: 'scroll' }]);
    actionReply.result.session.rid = rpc.id;
    return { request, rpc, actionReply };
  }
  function refresh(ids = []) {
    h.update();const request = sent.at(-1);
    assert.equal(request.m, 'view.update');h.receive(reply(request, ids));
  }
  h.greeting({ t: 'view', version: 1, renderer: 'web', generation: '3', epoch: 'e', limits: VIEW_LIMITS });
  h.receive(reply(sent.at(-1)));
  return { h, sent, commands, frames, feedback, settled, reply, send, refresh, elapse: () => { clock += 31000;h.expire(); } };
}
let checks = 0;
function check(name, run) { run();console.log('ok ' + name);checks++; }

for (const route of [undefined, { kind: 'bot-tab', bot: 'b', tab: 'chat' }]) {
  check(`${route ? 'bot' : 'thread'} preview bridges action, ordinary ACK and delayed saved picture`, () => {
    const f = setup(route), send = f.send();
    assert.equal(f.h.session.drafts.a, '');
    f.h.receive(send.actionReply);
    assert.equal(f.h.pendingSends('a').length, 1, 'action reply precedes persistence; preview must remain');
    assert.deepEqual(f.settled, [true]);
    f.refresh([]);
    assert.equal(f.h.pendingSends('a').length, 1, 'an older server picture must retain the preview');
    f.h.ack({ t: 'reply', id: send.rpc.id, ok: true });
    assert.equal(f.h.pendingSends('a').length, 1, 'RPC acceptance alone does not display the saved message');
    assert.equal(f.h.pendingSends('a')[0].accepted, true);
    f.refresh(['m1']);
    assert.equal(f.h.pendingSends('a').length, 0);
    assert.deepEqual(f.frames.at(-1), { ids: ['1'], pending: [] }, 'store row numbers differ from rendered message keys');
  });
}
check('saved picture may arrive before the ordinary ACK', () => {
  const f = setup(), send = f.send();f.h.receive(send.actionReply);f.refresh(['m1']);
  assert.equal(f.h.pendingSends('a').length, 0);
  f.h.ack({ t: 'reply', id: send.rpc.id, ok: true });
  assert.equal(f.h.pendingSends('a').length, 0);
});
check('identical sends reconcile by message id rather than text', () => {
  const f = setup(), first = f.send();f.h.receive(first.actionReply);
  f.h.request('draft', 'same message');const second = f.send('m2');f.h.receive(second.actionReply);
  assert.equal(f.h.pendingSends('a').length, 2);
  f.refresh(['m1']);assert.equal(f.h.pendingSends('a').length, 1);
  f.refresh(['m1']);assert.equal(f.h.pendingSends('a').length, 1);
  f.refresh(['m1', 'm2']);assert.equal(f.h.pendingSends('a').length, 0);
});
check('ordinary refusal removes the preview and restores it before newer typing once', () => {
  const f = setup(), send = f.send();f.h.receive(send.actionReply);f.h.request('draft', 'new text');
  f.h.ack({ t: 'reply', id: send.rpc.id, ok: false, error: 'refused' });
  assert.equal(f.h.pendingSends('a').length, 0);assert.equal(f.h.session.drafts.a, 'same message\n\nnew text');
  f.h.ack({ t: 'reply', id: send.rpc.id, ok: false });assert.equal(f.h.session.drafts.a, 'same message\n\nnew text');
});
check('action without a send command restores the capture immediately', () => {
  const f = setup(), send = f.send();f.h.receive(f.reply(send.request));
  assert.equal(f.h.pendingSends('a').length, 0);assert.equal(f.h.session.drafts.a, 'same message');
});
check('failed action restores capture alongside newer typing', () => {
  const f = setup(), send = f.send();f.h.request('draft', 'new text');
  f.h.receive({ t: 'reply', id: send.request.id, ok: false, error: 'refused' });
  assert.equal(f.h.pendingSends('a').length, 0);assert.equal(f.h.session.drafts.a, 'same message\n\nnew text');
});
check('timed out unissued action restores its text', () => {
  const f = setup();f.send();f.elapse();
  assert.equal(f.h.pendingSends('a').length, 0);assert.equal(f.h.session.drafts.a, 'same message');
});
check('another conversation cannot retire a captured send', () => {
  const f = setup(), send = f.send();f.h.receive(send.actionReply);
  f.h.request('select', 'b');f.h.receive(f.reply(f.sent.at(-1), ['m1']));
  assert.equal(f.h.pendingSends('b').length, 0);assert.equal(f.h.pendingSends('a').length, 1);
  f.h.request('select', 'a');f.h.receive(f.reply(f.sent.at(-1), ['m1']));
  assert.equal(f.h.pendingSends('a').length, 0);
});
check('retired picture does not consume its preview', () => {
  const f = setup(), send = f.send();f.h.receive(send.actionReply);
  f.h.update();const old = f.sent.at(-1);
  f.h.request('select', 'b');f.h.receive(f.reply(old, ['m1']));
  assert.equal(f.h.pendingSends('a').length, 1);
});
check('identified server outbox bubble replaces local preview before persistence', () => {
  const f = setup(), send = f.send();
  send.actionReply.result.session.outbox = { [send.rpc.id]: JSON.stringify(send.rpc) };
  send.actionReply.result.shown = timeline([sending()]);
  f.h.receive(send.actionReply);
  assert.equal(f.h.pendingSends('a').length, 0);
  f.h.request('draft', 'new text');
  f.h.ack({ t: 'reply', id: send.rpc.id, ok: false, error: 'refused' });
  assert.equal(f.h.session.drafts.a, 'same message\n\nnew text', 'refusal restores a capture already handed to a server bubble');
});
check('an outbox receipt without a displayed bubble cannot remove the preview', () => {
  const f = setup(), send = f.send();
  send.actionReply.result.session.outbox = { [send.rpc.id]: JSON.stringify(send.rpc) };
  f.h.receive(send.actionReply);assert.equal(f.h.pendingSends('a').length, 1);
});
check('an anonymous bubble without its outbox receipt cannot remove the preview', () => {
  const f = setup(), send = f.send();send.actionReply.result.shown = timeline([sending()]);
  f.h.receive(send.actionReply);assert.equal(f.h.pendingSends('a').length, 1);
});
console.log(JSON.stringify({ pass: true, checks, scope: 'Production ThinHost/ThinViews send lifecycle; deterministic delayed replies, no live data or provider.' }));
});
