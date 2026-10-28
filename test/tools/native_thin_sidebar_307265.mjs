import assert from 'node:assert/strict';

// Actual native host adapter, with synthetic metadata only. The server tests
// separately establish canonical membership, HMAC and pre-render row budgets.
export function registerSidebarChecks(check, harness, validate, validateAction) {
  const clone = value => JSON.parse(JSON.stringify(value));
  const control = (hub, suffix = 'next') => ({ label: 'Next', action: 'native-sidebar-page', value: hub + '|sealed-' + suffix });
  const pager = (hub, total, offset = 0, limit = total, extra = false) => ({ version: 1, total, offset, limit,
    selectedOutside: extra, previous: offset ? control(hub, 'previous') : null,
    next: Math.min(limit, total - offset) < total - offset ? control(hub) : null });
  const row = (hub, id, depth = 0) => ({ id: hub + '|' + id, title: 'Synthetic', state: 'idle', ago: '', pinned: false,
    lead: [], trail: [], depth });
  function picture(x, hub = 'h') {
    const shown = x.screen(hub);
    shown.projects = [{ id: hub + '|project', title: 'Synthetic', root: '', machine: '', open: false, archOpen: false,
      threads: [row(hub, 'a')], snoozedShelf: 'Snoozed', snoozed: [], shelf: 'Settled 20', settled: [],
      archived: [], value: hub + '|arch:project', settledCount: 20, archivedCount: 10,
      threadsPage: pager(hub, 1), snoozedPage: null, settledPage: null, archivedPage: null }];
    shown.active = []; shown.legacy = [];
    shown.nativeSidebar = 1;
    shown.sidebarPages = [{ version: 1, hub, projects: pager(hub, 1), active: null, legacy: null }];
    return { shown, overlay: { key: hub, name: 'Synthetic hub', j: { ...clone(shown), nativeSidebar: 1 } } };
  }
  const updateBoth = (p, change) => { change(p.shown); change(p.overlay.j); return p; };
  const input = hub => ({ hub });

  check('native sidebar accepts closed zero-row shelves with truthful nonzero counts', () => {
    const x = harness(), p = picture(x);
    assert(validate(p, input('h'), true));
    assert.equal(p.shown.projects[0].settled.length, 0);
    assert.equal(p.shown.projects[0].settledCount, 20);
    assert.equal(p.shown.projects[0].archivedCount, 10);
  });
  check('native sidebar old schema fallback remains independent of opt-in', () => {
    const x = harness(), p = { shown: x.screen(), overlay: { j: x.screen() } };
    assert(validate(p, input('h'), false)); assert(validate(p, input('h'), true));
  });
  check('native sidebar rejects unsolicited, partial and malformed per-hub wire negotiation markers', () => {
    const x = harness(); assert(!validate(picture(x), input('h'), false));
    for (const mutate of [p => delete p.overlay.j.nativeSidebar, p => delete p.shown.nativeSidebar,
      p => p.overlay.j.nativeSidebar = '1', p => delete p.overlay.j.sidebarPages,
      p => p.overlay.j.sidebarPages.push(clone(p.overlay.j.sidebarPages[0]))]) {
      const p = picture(x); mutate(p); assert(!validate(p, input('h'), true));
    }
  });
  check('native sidebar rejects malformed or missing counts instead of legacy fallback', () => {
    const x = harness();
    for (const value of [-1, 1.5, '20', null, Number.MAX_SAFE_INTEGER + 1]) {
      const p = updateBoth(picture(x), s => s.projects[0].settledCount = value);
      assert(!validate(p, input('h'), true));
    }
    for (const key of ['settledCount', 'archivedCount', 'settledPage']) {
      const p = updateBoth(picture(x), s => delete s.projects[0][key]); assert(!validate(p, input('h'), true));
    }
  });
  check('native sidebar rejects hidden rows or hidden pagers on a closed shelf', () => {
    const x = harness();
    for (const change of [s => s.projects[0].settled = [row('h', 'hidden')],
      s => s.projects[0].settledPage = pager('h', 20, 0, 1),
      s => s.projects[0].archived = [row('h', 'archived')]])
      assert(!validate(updateBoth(picture(x), change), input('h'), true));
  });
  check('native sidebar exact ranges require both appropriate traversal controls', () => {
    const x = harness();
    const p = updateBoth(picture(x), s => { s.projects[0].threads = [row('h', 'b'), row('h', 'c')];
      s.projects[0].threadsPage = pager('h', 6, 2, 2); });
    assert(validate(p, input('h'), true));
    for (const change of [s => s.projects[0].threads.pop(), s => s.projects[0].threadsPage.previous = null,
      s => s.projects[0].threadsPage.next = null, s => s.projects[0].threadsPage.offset = 6,
      s => s.projects[0].threadsPage.limit = 0, s => s.projects[0].threadsPage.total = 0])
      assert(!validate(updateBoth(clone(p), change), input('h'), true));
  });
  check('native sidebar selected exception is one real selected row and can preserve grouping position', () => {
    const x = harness();
    const p = updateBoth(picture(x), s => { s.projects[0].threads = [row('h', 'other'), row('h', 'a')];
      s.projects[0].threadsPage = pager('h', 4, 0, 1, true); });
    assert(validate(p, input('h'), true));
    assert(validate(updateBoth(clone(p), s => s.projects[0].threads.reverse()), input('h'), true));
    for (const change of [s => s.projects[0].threads[1].id = 'h|wrong',
      s => s.projects[0].threads[0].id = 'h|a']) assert(!validate(updateBoth(clone(p), change), input('h'), true));
  });
  check('native sidebar descendant rows share the global 48-row budget', () => {
    const x = harness();
    const p = updateBoth(picture(x), s => { s.projects[0].threads = Array.from({ length: 25 }, (_, i) => row('h', 'tree' + i, i ? 4 : 0));
      s.projects[0].threadsPage = pager('h', 25); s.projects[0].snoozed = Array.from({ length: 23 }, (_, i) => row('h', 'snoozed' + i));
      s.projects[0].snoozedPage = pager('h', 23); });
    assert(validate(p, input('h'), true));
    updateBoth(p, s => { s.projects[0].snoozed.push(row('h', 'overflow')); s.projects[0].snoozedPage = pager('h', 24); });
    assert(!validate(p, input('h'), true));
  });
  check('native sidebar project capacity cannot use the generic 48-row limit', () => {
    const x = harness(); const p = picture(x);
    updateBoth(p, s => s.sidebarPages[0].projects.limit = 9); assert(!validate(p, input('h'), true));
  });
  check('native sidebar rejects wrong owner, actions, oversized controls and projection disagreement', () => {
    const x = harness();
    for (const change of [s => s.sidebarPages[0].hub = 'other', s => s.projects[0].threads[0].id = 'other|a',
      s => { s.projects[0].threadsPage = pager('h', 2, 0, 1); s.projects[0].threadsPage.next.action = 'send'; },
      s => { s.projects[0].threadsPage = pager('h', 2, 0, 1); s.projects[0].threadsPage.next.value = 'other|sealed'; },
      s => { s.projects[0].threadsPage = pager('h', 2, 0, 1); s.projects[0].threadsPage.next.value = 'h|' + 'x'.repeat(2049); }])
      assert(!validate(updateBoth(picture(x), change), input('h'), true));
    const p = picture(x); p.overlay.j.projects[0].settledCount++; assert(!validate(p, input('h'), true));
  });
  check('native sidebar metadata cannot authorize an earlier in-flight unoffered reply', () => {
    const x = harness(); x.h.hubs(['h']); x.h.connection('h', '1', 'view-v1');
    const req = x.greeting().cmds[0].request; x.h.nativeSidebar = true;
    const out = x.answer(req.id, picture(x)); assert.equal(out.sync.at(-1).phase, 'failed');
  });
  check('native sidebar malformed reply rejects the entire accompanying command batch', () => {
    const x = harness(); x.h.nativeSidebar = true; x.open(); x.h.request('h', 'send', '');
    const req = x.h.drain().cmds[0].request, p = picture(x); p.overlay.j.projects[0].archivedCount = -1;
    const out = x.answer(req.id, { ...p, cmds: [{ type: 'keep', thread: 'h|a', text: '' }] });
    assert.equal(out.sync.at(-1).phase, 'failed'); assert.equal(x.calls.command, 0); assert.deepEqual(out.cmds, []);
  });
  check('native sidebar other-hub paging preserves foreground, selection and local drafts', () => {
    const x = harness(); x.h.nativeSidebar = true; x.open('a'); x.open('b');
    const s = x.h.states.get('b'); x.h.update(s); const req = x.h.drain().cmds[0].request;
    const p = picture(x, 'b'); updateBoth(p, screen => screen.projects[0].threadsPage = pager('b', 2, 0, 1));
    x.answer(req.id, p, 'b'); x.h.current = 'a';
    s.session.drafts.a = 'Synthetic local draft';
    s.session.info['fab.open:a'] = '0'; s.session.outbox.pending = 'Synthetic pending output';
    const before = clone(s.session);
    assert(x.h.sidebarPage('b|sealed-next')); const out = x.h.drain();
    assert.equal(out.cmds.length, 1); assert.equal(out.cmds[0].hub, 'b');
    assert.equal(out.cmds[0].request.p.action, 'native-sidebar-page');
    assert.equal(x.h.current, 'a'); assert.deepEqual(s.session, before);
    assert.equal(x.h.sidebarPage('b|sealed-next'), false); // No duplicate queued behind this read.
    x.h.draftAt('b', 'a', 'Synthetic newer IME'); x.h.drain();
    const changed = picture(x, 'b');
    updateBoth(changed, screen => { screen.projects[0].threads = [row('b', 'next-row'), row('b', 'a')];
      screen.projects[0].threadsPage = pager('b', 2, 1, 1, true); });
    changed.session = clone(out.cmds[0].request.p.session);
    Object.assign(changed.session.drafts, { '@boot': '', '@sync': '1', '@mech.at': '', '@solid.at': '', '@desk': '',
      '@presentation': '', '@bot': '', '@room': '', '@remote': '', '@btab': '' });
    changed.session.info['fab.open:a'] = '0'; changed.session.outbox = clone(before.outbox);
    x.answer(out.cmds[0].request.id, changed, 'b');
    assert.equal(s.ready, true); assert.equal(x.h.current, 'a'); assert.equal(s.session.sel, before.sel);
    assert.deepEqual(s.session.route, before.route); assert.deepEqual(s.session.open, before.open);
    assert.equal(s.session.drafts.a, 'Synthetic newer IME');
    assert.equal(s.session.info['fab.open:a'], '0'); assert.deepEqual(s.session.outbox, before.outbox);
    assert.equal(s.shown.projects[0].threads[0].id, 'b|next-row');
  });
  check('native sidebar paging replies reject commands, redirects and changed user choices atomically', () => {
    const corrupt = [
      reply => reply.cmds = [{ type: 'send', text: JSON.stringify({ id: 77, m: 'turn.start', p: {} }) }],
      reply => reply.cmds = [{ type: 'keep', thread: 'b|a', text: '' }],
      reply => reply.nextTarget = { kind: 'thread', id: 'other' },
      reply => reply.session.sel = 'other', reply => reply.session.route = { kind: 'bot-tab', bot: 'bot', tab: 'chat' },
      reply => reply.session.drafts.a = 'Changed draft', reply => reply.session.drafts.foreign = 'Foreign draft',
      reply => reply.session.open.settings = true, reply => reply.session.adding = true,
      reply => reply.session.panel = true, reply => reply.session.wantNew = true,
      reply => reply.session.info['fold:other'] = '1', reply => reply.session.info.privatePages = '1',
      reply => reply.session.info['fab.open:a'] = '1', reply => reply.session.info['fab.buy:a'] = 'Synthetic new choice',
      reply => reply.session.outbox.other = 'queued', reply => reply.session.rid++,
      reply => reply.session.drafts['@presentation'] = 'changed', reply => reply.session.drafts['@btab'] = 'chat',
      reply => reply.session.drafts['@bot'] = 'other', reply => reply.session.drafts['@room'] = 'other',
    ];
    for (const mutate of corrupt) {
      const x = harness(); x.h.nativeSidebar = true; x.open('a'); x.open('b');
      const s = x.h.states.get('b'); x.h.update(s); let req = x.h.drain().cmds[0].request;
      const offered = picture(x, 'b'); updateBoth(offered, screen => screen.projects[0].threadsPage = pager('b', 2, 0, 1));
      x.answer(req.id, offered, 'b'); x.h.current = 'a';
      s.session.drafts.a = 'Synthetic existing draft';
      assert(x.h.sidebarPage('b|sealed-next')); req = x.h.drain().cmds[0].request;
      const before = clone(s.session), displayed = clone(x.h.client.links.get('b').shown), overlay = clone(s.overlay);
      const reply = { ...picture(x, 'b'), session: clone(req.p.session), cmds: [] }; mutate(reply);
      const result = x.answer(req.id, reply, 'b');
      assert.equal(result.sync.at(-1)?.phase, 'failed');
      assert.deepEqual(result.cmds, []); assert.equal(x.calls.command, 0);
      assert.equal(x.h.current, 'a'); assert.deepEqual(s.session, before);
      assert.deepEqual(s.overlay, overlay); assert.deepEqual(x.h.client.links.get('b').shown, displayed);
    }
  });
  check('native sidebar normalizes selectors only from captured target and route', () => {
    for (const target of [{ kind: 'thread', id: 'a' }, { kind: 'bot', id: 'bot' }, { kind: 'room', id: 'room' }]) {
      const x = harness(), p = picture(x), session = { sel: 'a', drafts: { a: 'Synthetic draft' }, open: {}, adding: false,
        panel: false, wantNew: false, info: {}, outbox: {}, rid: 1, ...(target.kind === 'bot' ? { route: { kind: 'bot-tab', bot: 'bot', tab: 'chat' } } : {}) };
      const input = { action: 'native-sidebar-page', hub: 'h', target, session };
      const result = { ...p, session: clone(session), cmds: [], remoteReady: true };
      Object.assign(result.session.drafts, { '@presentation': '', '@bot': target.kind === 'bot' ? 'bot' : '',
        '@room': target.kind === 'room' ? 'room' : '', '@remote': '', '@btab': target.kind === 'bot' ? 'chat' : '' });
      assert(validateAction(result, input));
      for (const field of ['@presentation', '@bot', '@room', '@remote', '@btab']) {
        const wrong = clone(result); wrong.session.drafts[field] = 'unrelated'; assert(!validateAction(wrong, input));
      }
      const changed = clone(result); changed.session.drafts['@f.edit'] = 'Changed'; assert(!validateAction(changed, input));
      const known = clone(result); known.session.drafts['@f.edit'] = 'Existing actor value';
      assert(validateAction(known, input, { drafts: { '@f.edit': 'Existing actor value' }, info: {} }));
      assert(!validateAction(changed, input, { drafts: { '@f.edit': 'Existing actor value' }, info: {} }));
    }
  });
  check('native sidebar cannot issue invented, stale, disconnected or unpaired controls', () => {
    for (const state of ['invented', 'unready', 'disconnect', 'forget']) {
      const x = harness(); x.h.nativeSidebar = true; x.open(); x.h.update(x.h.states.get('h'));
      const req = x.h.drain().cmds[0].request, p = picture(x);
      updateBoth(p, screen => screen.projects[0].threadsPage = pager('h', 2, 0, 1)); x.answer(req.id, p);
      if (state === 'unready') x.h.states.get('h').ready = false;
      if (state === 'disconnect') x.h.disconnect('h');
      if (state === 'forget') x.h.forgetHub('h');
      assert.equal(x.h.sidebarPage(state === 'invented' ? 'h|not-offered' : 'h|sealed-next'), false);
      assert.deepEqual(x.h.drain().cmds, []);
    }
  });
}
