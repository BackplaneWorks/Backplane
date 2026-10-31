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
