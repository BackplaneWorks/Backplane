// The viewer reads kept hub-wide (src/server/effects/viewer_kept.js): what a
// thread's views are given at once is only that thread's, only for the folder
// it works in, the newest answer, gone once dropped, bounded; a read ahead is
// claimed once while on its way and again only when what is kept is old.
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
const effects = new Map();
const code = fs.readFileSync('src/server/effects/viewer_kept.js', 'utf8').replace(/CID\(([^)]+)\)/g, '"$1"');
const unlist = (xs) => { const o = {};for (; xs.$ === 'Con'; xs = xs.tail.tail) o[xs.head] = xs.tail.head;return o; };
new Function('io_eff', code)((name, fn) => effects.set(name, fn));
const put = (...a) => effects.get('ViewerKept.put')(...a), drop = (...a) => effects.get('ViewerKept.drop')(...a);
const raw = (t, cwd, since = '') => unlist(effects.get('ViewerKept.info')(t, cwd, since)), claim = (...a) => effects.get('ViewerKept.claim')(...a);
// everything kept for a thread (a view given nothing yet), without the number it comes with
const info = (t, cwd) => { const o = raw(t, cwd);delete o['kept.seq:' + t];return o; };

put('t1', '/p', 'fm.kept:t1\n', 'd\t0\tsrc\n');
put('t1', '/p', 'fm.kept:t1\nsrc', 'f\t3\ta.c\n');
put('t2', '/q', 'fm.kept:t2\n', 'f\t1\tother\n');
assert.deepEqual(info('t1', '/p'), { 'fm.kept:t1\n': 'd\t0\tsrc\n', 'fm.kept:t1\nsrc': 'f\t3\ta.c\n' }, 'a thread is given its own folders');
assert.deepEqual(info('t2', '/q'), { 'fm.kept:t2\n': 'f\t1\tother\n' }, 'never another thread\'s');
assert.deepEqual(info('t3', '/p'), {}, 'a thread with nothing kept is given nothing');
put('t1', '/p', 'fm.kept:t1\nsrc', 'f\t4\tb.c\n');
assert.equal(info('t1', '/p')['fm.kept:t1\nsrc'], 'f\t4\tb.c\n', 'the newest listing replaces the kept one');
put('t1', '/p', 'fm.kept:t1\nempty', '');
assert.equal(info('t1', '/p')['fm.kept:t1\nempty'], '', 'an empty folder is kept as empty');
drop('t1', 'fm.kept:t1\nsrc');
assert.equal(info('t1', '/p')['fm.kept:t1\nsrc'], undefined, 'a folder that can no longer be listed is dropped');
assert.deepEqual(info('t1', '/elsewhere'), {}, 'a thread now working in another folder is given nothing');
assert.deepEqual(info('t1', '/p'), {}, 'and what was kept for the old folder is gone');

// a view is given only what changed after the number it was last given
put('t8', '/p', 'fm.kept:t8\n', 'd\t0\tsrc\n');
const first = raw('t8', '/p'), seq = first['kept.seq:t8'];
assert.ok(seq && first['fm.kept:t8\n'] === 'd\t0\tsrc\n', 'a view given nothing yet gets all, and a number');
assert.deepEqual(raw('t8', '/p', seq), {}, 'nothing changed since: nothing');
put('t8', '/p', 'fm.kept:t8\nsrc', 'f\t1\tnew\n');
const next = raw('t8', '/p', seq);
assert.deepEqual(Object.keys(next).sort(), ['fm.kept:t8\nsrc', 'kept.seq:t8'], 'only what changed, with the new number');
assert.ok(Number(next['kept.seq:t8']) > Number(seq));
put('t2', '/q', 'fm.kept:t2\nx', '');
assert.deepEqual(raw('t8', '/p', next['kept.seq:t8']), {}, 'another thread changing gives this one nothing');
assert.deepEqual(Object.keys(raw('t8', '/p', '999999999')).length, 3, 'a number this process never gave: all of it');

// claims: one read on its way at a time, again once what is kept is older than asked
assert.equal(claim('t5', '/p', 'fm.kept:t5\n', '30000'), '1', 'nothing kept: read it');
assert.equal(claim('t5', '/p', 'fm.kept:t5\n', '30000'), '', 'a read on its way is not asked twice');
put('t5', '/p', 'fm.kept:t5\n', '');
assert.equal(claim('t5', '/p', 'fm.kept:t5\n', '30000'), '', 'kept just now: not read again');
assert.equal(claim('t5', '/p', 'fm.kept:t5\n', '0'), '1', 'kept longer ago than asked: read again');
assert.equal(claim('', '/p', 'fm.kept:\n', '0'), '', 'no thread: nothing');

// bounds: 64 keys a thread, the least recent out
for (let i = 0; i < 70; i++) put('t6', '/p', 'fm.kept:t6\nd' + i, 'x');
const keys = Object.keys(info('t6', '/p'));
assert.equal(keys.length, 64, 'a thread keeps 64 folders at most');
assert.ok(!keys.includes('fm.kept:t6\nd0') && keys.includes('fm.kept:t6\nd69'), 'the oldest go first');
// a value too big to keep is not kept (and drops what was)
put('t7', '/p', 'diff.kept:t7', 'small');
put('t7', '/p', 'diff.kept:t7', 'x'.repeat(2 * 1024 * 1024));
assert.equal(info('t7', '/p')['diff.kept:t7'], undefined, 'an answer too big to keep drops the kept one');
// 256 threads at most
for (let i = 0; i < 300; i++) put('n' + i, '/p', 'k', 'v');
assert.deepEqual(info('n0', '/p'), {}, 'the least recent threads go first');
assert.deepEqual(info('n299', '/p'), { k: 'v' });
console.log('viewer_kept_test: ok');
