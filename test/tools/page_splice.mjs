// test/page_splice_test.bend's cases: a view.page drawn from the last
// view with only its timeline again (thin-splice.bend's Page.splice) must
// be the full render of that page, byte for byte; and the top bar's
// "Design changes" is the whole thread's (8 steps) on the newest page and
// on an old one, never the page's own.
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const lib = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
const names = ['newest page to the older one', 'the older page back to the newest', 'one older page to the next older',
  'with a live row, newest to older', 'the same page again'];

// the value under the top bar's "Design changes" key, or null
function changes(view) {
  const find = (n) => {
    if (!n || typeof n !== 'object') return null;
    if (Array.isArray(n)) { for (const x of n) { const r = find(x); if (r !== null) return r; } return null; }
    const cls = (n.attrs || []).find((a) => a.name === 'class')?.value || '';
    if (n.tag === 'span' && cls === 'kv changes') {
      const v = (n.kids || []).find((k) => (k.attrs || []).some((a) => a.name === 'class' && a.value === 'v'));
      return v?.kids?.[0]?.text ?? null;
    }
    return find(n.kids);
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

for (const [name, i] of [['the newest page counts the thread\'s 8 changes', 4], ['an old page counts the thread\'s 8 changes', 2]]) {
  const got = changes(lib[`c${i}.full`](0n));
  if (got === '8') console.log('ok', name);
  else { failed++; console.log('FAIL', name, 'got', got); }
}
if (failed) process.exit(1);
