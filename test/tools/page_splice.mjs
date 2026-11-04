// test/page_splice_test.bend's cases: a view.page drawn from the last
// view with only its timeline again (thin-splice.bend's Page.splice) must
// be the full render of that page, byte for byte; and the top bar has no
// count of the thread's design changes (8 steps) on any page: the viewer's
// history bar counts them.
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const lib = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
const names = ['newest page to the older one', 'the older page back to the newest', 'one older page to the next older',
  'with a live row, newest to older', 'the same page again'];

// any element of the view whose class is the old top-bar count's
function changesCell(view) {
  const find = (n) => {
    if (!n || typeof n !== 'object') return false;
    if (Array.isArray(n)) return n.some(find);
    const cls = (n.attrs || []).find((a) => a.name === 'class')?.value || '';
    return /(^| )kv changes( |$)/.test(cls) || find(n.kids);
  };
  return find(JSON.parse(view).shown);
}

let failed = 0;
names.forEach((name, i) => {
  const spliced = lib[`c${i}.spliced`](0n), full = lib[`c${i}.full`](0n);
  try {
    assert.notEqual(spliced, '(none)', 'no splice');
    assert.equal(spliced, full);
    console.log('ok', name);
  } catch (e) {
    failed++;
    console.log('FAIL', name, e.message.slice(0, 300));
  }
});

for (const [name, i] of [['the newest page has no design changes count', 4], ['nor an old page', 2]]) {
  if (!changesCell(lib[`c${i}.full`](0n))) console.log('ok', name);
  else { failed++; console.log('FAIL', name); }
}
// the window's counts: older (remaining), newer and total, in rows as its ids
{
  const parse = s => { const [older, newer, total, ...ids] = s.trim().split(' '); return { older: +older, newer: +newer, total: +total, ids }; };
  const a = parse(lib['counts.newest'](0n)), b = parse(lib['counts.older'](0n));
  const checks = [
    ['the newest page has nothing newer', a.newer === 0],
    ['total is older + ids + newer (newest)', a.total === a.older + a.ids.length + a.newer],
    ['total is older + ids + newer (older page)', b.total === b.older + b.ids.length + b.newer],
    ['total is the same from either page', a.total === b.total],
    ['an older page counts the rows above its first', b.newer === a.ids.indexOf(b.ids[0]) && b.newer > 0],
  ];
  for (const [name, ok] of checks) { if (ok) console.log('ok', name); else { failed++; console.log('FAIL', name, JSON.stringify({ a, b })); } }
}
if (failed) process.exit(1);
