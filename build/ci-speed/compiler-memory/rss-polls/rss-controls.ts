import assert from 'node:assert/strict';
import * as B from './compiler/bend2/bend.ts';
import {TermStore} from './compiler/bend2/offload.ts';

const GiB = 1024 ** 3;
const book = B.book_nil();
B.parse_book(book, '/public/', 'def a() -> U32:\n  1\ndef b() -> U32:\n  2\ndef c() -> U32:\n  3\n', '', {});
for (const name of book.order) {
  const def = book.tlds[name] as B.Def;
  def.e = B.term_lower(def.v!);
}
const store = new TermStore();
const results: Record<string, unknown> = {};

function probe(names: string[], levels: number[]) {
  const originalMemory = process.memoryUsage;
  const originalGC = Bun.gc;
  let samples = 0;
  const collections: number[] = [];
  process.memoryUsage = Object.assign(() => {
    assert.ok(samples < levels.length, 'unexpected RSS probe');
    return {rss: levels[samples++], heapUsed: 0, heapTotal: 0,
      external: 0, arrayBuffers: 0};
  }, originalMemory);
  Bun.gc = ((sync?: boolean) => {
    assert.equal(sync, true);
    collections.push(samples);
    return 0;
  }) as typeof Bun.gc;
  try {
    store.replace(book, names);
    assert.equal(samples, levels.length, 'missing initial/new-definition RSS probe');
    return {samples, collections};
  } finally {
    process.memoryUsage = originalMemory;
    Bun.gc = originalGC;
  }
}

// A growth threshold after the second new definition must collect immediately,
// resample collectedAt, and then poll the third definition independently.
const fresh = probe(['a', 'b', 'c'], [GiB, 1.25 * GiB, 1.75 * GiB, GiB, 1.25 * GiB]);
assert.deepEqual(fresh, {samples: 5, collections: [3]});
results.new_definitions_and_collection_reset = fresh;
const a = book.tlds.a;
const bytes = store.bytes;
const body = B.term_key((book.tlds.a as B.Def).e!);

// Repeated accepted prefix names preserve identity and storage. They still
// take the initial boundary sample, with no allocation-driven polls or GC.
const known = probe(Array(4096).fill('a'), [GiB]);
assert.deepEqual(known, {samples: 1, collections: []});
assert.equal(book.tlds.a, a);
assert.equal(store.bytes, bytes);
assert.equal(B.term_key((book.tlds.a as B.Def).e!), body);
results.known_prefix_4096 = known;

B.parse_book(book, '/public/', 'def d() -> U32:\n  4\n', '', {});
const d = book.tlds.d as B.Def;
d.e = B.term_lower(d.v!);
const mixed = probe([...Array(4096).fill('a'), 'd', 'd'], [GiB, 1.75 * GiB, GiB]);
assert.deepEqual(mixed, {samples: 3, collections: [2]});
assert.notEqual(book.tlds.d, d);
assert.ok(store.bytes > bytes);
results.new_after_known_prefix_and_duplicate = mixed;

// A new object installed at an existing name must be stored and polled.
book.tlds.a = {...(book.tlds.d as B.Def), n: 'a', T: book.tlds.a.T,
  v: (book.tlds.d as B.Def).v, e: (book.tlds.d as B.Def).e};
const replacedObject = book.tlds.a;
const replaced = probe(['a'], [GiB, GiB]);
assert.deepEqual(replaced, {samples: 2, collections: []});
assert.notEqual(book.tlds.a, replacedObject);
results.new_object_at_existing_name = replaced;
console.log(JSON.stringify({kind: 'RSS admission boundary controls, synthetic readings',
  real_gc_overridden_only_inside_test: true, results}));
