const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { File } = require('node:buffer');
const source = fs.readFileSync(path.join(__dirname, '../../src/web/draft-attachments.js'), 'utf8');
const modulePromise = import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'));

// Exercise the lifecycle independently of IndexedDB's transaction plumbing.
// The desktop check separately verifies that picked bytes survive a reload.
async function cache(records = []) {
  const { DraftAttachments } = await modulePromise;
  const store = Object.create(DraftAttachments.prototype);
  store.identity = () => 'pair-a';store.now = () => 100;
  store.prune = async () => {};
  store.started = new Map();
  store.restoring = new Map();
  const rows = new Map([['t', records]]);
  store.change = async (thread, update) => { rows.set(thread, update(rows.get(thread) || [])); };
  store.rows = () => rows.get('t') || [];
  return store;
}
const note = () => new File(['unsent notes'], 'notes.txt', { type: 'text/plain' });

test('a stale empty frame cannot delete a newly saved attachment', async () => {
  const store = await cache();
  await store.sync('t', [], () => assert.fail('nothing to restore'), () => true);
  await store.remember('t', '/new-notes.txt', note());
  await store.sync('t', [], () => assert.fail('already restored'), () => true);
  assert.equal(store.rows().length, 1);
  assert.equal(await store.rows()[0].file.text(), 'unsent notes');
});

test('reload uploads the saved bytes and replaces the old reference once', async () => {
  const file = note(), store = await cache([{ path: '/old-notes.txt', file }]);
  let uploads = 0;
  await store.sync('t', [], async (picked, previous) => {
    uploads++;
    assert.equal(picked, file);
    assert.equal(await picked.text(), 'unsent notes');
    await store.remember('t', '/new-notes.txt', picked, previous);
    return '/new-notes.txt';
  }, () => true);
  await store.sync('t', ['/new-notes.txt'], () => assert.fail('duplicate upload'), () => true);
  assert.equal(uploads, 1);
  assert.deepEqual(store.rows().map(r => r.path), ['/new-notes.txt']);
});

test('an attachment the live actor still has is not uploaded twice', async () => {
  const store = await cache([{ path: '/kept.txt', file: note() }]);
  await store.sync('t', ['/kept.txt'], () => assert.fail('duplicate upload'), () => true);
  assert.equal(store.rows().length, 1);
});

test('acknowledged removal preserves a file added after that action', async () => {
  const store = await cache([{ path: '/sent.txt', file: note() }]);
  await store.remember('t', '/later.txt', note());
  await store.forget('t', ['/sent.txt']);
  assert.deepEqual(store.rows().map(r => r.path), ['/later.txt']);
});

test('switching threads retains the file and permits a later retry', async () => {
  const store = await cache([{ path: '/unsent.txt', file: note() }]);
  await store.sync('t', [], () => assert.fail('wrong thread'), () => false);
  assert.equal(store.rows().length, 1);
  assert.equal(store.started.has('t'), false);
});

test('a refused restore retains its bytes for the next connection', async () => {
  const store = await cache([{ path: '/unsent.txt', file: note() }]);
  await store.sync('t', [], async () => undefined, () => true);
  assert.equal(store.rows().length, 1);
  assert.equal(store.started.has('t'), false);
});

test('a fresh actor restores a previously visited thread again', async () => {
  const store = await cache([{ path: '/old.txt', file: note() }]);
  let uploads = 0;
  const upload = async () => { uploads++; return '/restored.txt'; };
  await store.sync('t', [], upload, () => true, 'actor-1');
  await store.sync('t', [], upload, () => true, 'actor-2');
  assert.equal(uploads, 2);
});

test('the whole saved batch owns Send until its last upload has completed', async () => {
  const store = await cache([{ path: '/a.txt', file: note() }, { path: '/b.txt', file: note() }]);
  let release;
  const waiting = new Promise(resolve => { release = resolve; });
  let uploads = 0;
  const restoring = store.sync('t', [], async () => { uploads++; await waiting; return '/new.txt'; }, () => true, 'actor');
  assert.equal(store.pending('t'), true);
  await Promise.resolve();
  release();await restoring;
  assert.equal(uploads, 2);
  assert.equal(store.pending('t'), false);
});


test('raw bytes expire and never cross a pairing identity', async () => {
  const { retained, MAX_AGE } = await modulePromise;
  const record = { identity: 'pair-a', at: 100 };
  assert.equal(retained(record, 'pair-a', 101), true);
  assert.equal(retained(record, 'pair-b', 101), false);
  assert.equal(retained(record, '', 101), false);
  assert.equal(retained(record, 'pair-a', 100 + MAX_AGE), false);
  assert.equal(retained(record, 'pair-a', 99), false);
  assert.equal(retained({ path: '/legacy' }, 'pair-a', 101), false);
});

test('restoring does not extend how long the original file is retained', async () => {
  const store = await cache([{ path: '/old.txt', file: note(), identity: 'pair-a', at: 1 }]);
  await store.remember('t', '/new.txt', note(), '/old.txt');
  assert.equal(store.rows()[0].at, 1);
  await store.forget('t');
  assert.equal(store.rows().length, 0);
});

test('a changed pairing stops a restore batch before its next file', async () => {
  const store = await cache([{ path: '/a.txt', file: note() }, { path: '/b.txt', file: note() }]);
  let uploads = 0;
  await store.sync('t', [], async () => { uploads++;store.identity = () => 'pair-b';return '/new.txt'; }, () => true, 'actor');
  assert.equal(uploads, 1);
});

test('an oversized file is not kept locally', async () => {
  const { MAX_BYTES } = await modulePromise;
  const store = await cache();
  await assert.rejects(store.remember('t', '/big.bin', { size: MAX_BYTES + 1 }));
  assert.equal(store.rows().length, 0);
});

test('unavailable browser storage does not permanently block the live composer', async () => {
  const store = await cache();
  let reads = 0;
  store.change = async () => { reads++;throw Error('storage unavailable'); };
  await store.sync('t', [], () => assert.fail('no bytes'), () => true, 'actor');
  assert.equal(store.pending('t'), false);
  await store.sync('t', [], () => assert.fail('no bytes'), () => true, 'actor');
  assert.equal(reads, 1);
});

test('pruning clears removed projects, old identities, expired files and bytes over the budget', async () => {
  const { DraftAttachments, MAX_BYTES, MAX_AGE } = await modulePromise;
  const store = await cache();
  const rows = [
    { key: 'foreign', identity: 'pair-b', at: 90, file: { size: 1 } },
    { key: 'expired', identity: 'pair-a', at: 100 - MAX_AGE, file: { size: 1 } },
    { key: 'removed', identity: 'pair-a', project: 'gone', at: 90, file: { size: 1 } },
    { key: 'newer', identity: 'pair-a', at: 90, file: { size: MAX_BYTES } },
    { key: 'older', identity: 'pair-a', at: 80, file: { size: 1 } },
  ];
  const deleted = [];
  store.transaction = update => new Promise(resolve => {
    const request = {};
    update({ getAll() { queueMicrotask(() => { request.result = rows;request.onsuccess();resolve(); });return request; },
      delete(key) { deleted.push(key); } });
  });
  await DraftAttachments.prototype.prune.call(store, 'gone');
  assert.deepEqual(deleted.sort(), ['expired', 'foreign', 'older', 'removed']);
});

test('a restore that finishes after its saved record expired cannot renew its retention', async () => {
  const store = await cache();
  await store.remember('t', '/new.txt', note(), '/expired.txt');
  assert.equal(store.rows().length, 0);
});

test('restore keeps the saved project boundary before the fresh view is drawn', async () => {
  const store = await cache([{ path: '/old.txt', file: note(), at: 1, project: 'p1' }]);
  await store.remember('t', '/new.txt', note(), '/old.txt', '');
  assert.equal(store.rows()[0].project, 'p1');
});
