// Synthetic buffers only: the actual streaming parser + gzip decoder. No
// filesystem, private archive, hub, credentials or network are opened.
const assert = require('node:assert/strict'), { gzipSync, createGunzip } = require('node:zlib');
const { Readable } = require('node:stream'), { TarStream } = require('./real_store_active_tar.cjs');
const P = require('./real_store_active_policy.cjs');
function header(name, size, type = '0') {
  const h = Buffer.alloc(512); h.write(name); h.write('0000600\0', 100); h.write('0000000\0', 108); h.write('0000000\0', 116);
  h.write(size.toString(8).padStart(11, '0') + '\0', 124); h.write('00000000000\0', 136); h.fill(32, 148, 156);
  h[156] = type.charCodeAt(0); h.write('ustar\0', 257); h.write('00', 263);
  h.write([...h].reduce((a, b) => a + b, 0).toString(8).padStart(6, '0') + '\0 ', 148); return h;
}
function record(key, value) {
  const body = `${key}=${value}\n`; let n = Buffer.byteLength(body) + 2;
  while (Buffer.byteLength(String(n) + ' ' + body) !== n) n = Buffer.byteLength(String(n) + ' ' + body);
  return Buffer.from(String(n) + ' ' + body);
}
function member(name, data, type = '0') { data = Buffer.from(data); return Buffer.concat([header(name, data.length, type), data, Buffer.alloc((512 - data.length % 512) % 512)]); }
const profile = JSON.stringify({ version: 1, profiles: [{ id: '0'.repeat(36), provider: 'claude', label: 'synthetic metadata', epoch: 1, fingerprint: '', store: '' }] });
function archive(pax = true) {
  const chunks = [];
  for (const [name, text] of [['store.db', 'owned sqlite stand-in'], ['events.jsonl', '{}\n'.repeat(300)], ['RECEIPT.json', '{}'], ['providers/accounts-v1/profiles.json', profile]]) {
    if (pax) chunks.push(member('././@PaxHeader', record('mtime', '1793769000.1234567'), 'x'));
    chunks.push(member(name, text));
  }
  chunks.push(Buffer.alloc(1024)); return Buffer.concat(chunks);
}
function collector() {
  const files = new Map(); let current;
  const parser = new TarStream({ begin(name) { current = name; files.set(name, []); }, data(b) { files.get(current).push(Buffer.from(b)); }, end() { current = null; } });
  return { parser, files };
}
function parse(b, step = 17) { const c = collector(); for (let i = 0; i < b.length; i += step) c.parser.push(b.subarray(i, i + step)); return { ...c.parser.finish(), files: c.files }; }
module.exports.run = async function run() {
  let cases = 0;
  for (const step of [1, 17, 511, 512, 513, 2049]) {
    const r = parse(archive(), step); assert.equal(r.members, 4); assert.equal(r.paxHeaders, 4);
    assert.equal(Buffer.concat(r.files.get('providers/accounts-v1/profiles.json')).toString(), profile);
    assert.equal(P.accountMetadata(JSON.parse(profile)).profiles, 1); cases++;
  }
  // Real gzip stream decoding with arbitrary compressed and decoded splits.
  const compressed = gzipSync(archive()), c = collector();
  async function* bytes() { for (let i = 0; i < compressed.length; i += 7) yield compressed.subarray(i, i + 7); }
  for await (const b of Readable.from(bytes()).pipe(createGunzip({ chunkSize: 1024 })))
    for (let i = 0; i < b.length; i += 31) c.parser.push(b.subarray(i, i + 31));
  assert.equal(c.parser.finish().members, 4); cases++;
  const checksum = archive(false); checksum[0] ^= 1; assert.throws(() => parse(checksum)); cases++;
  const padding = archive(false); padding[512 + 'owned sqlite stand-in'.length] = 1; assert.throws(() => parse(padding)); cases++;
  for (const n of [1, 511, 513, archive().length - 1, archive().length - 1024]) { assert.throws(() => parse(archive().subarray(0, n))); cases++; }
  for (const key of ['path', 'linkpath', 'size', 'GNU.sparse.map', 'SCHILY.xattr.user.anything']) {
    const b = Buffer.concat([member('././@PaxHeader', record(key, '../unsafe'), 'x'), archive(false)]); assert.throws(() => parse(b)); cases++;
  }
  for (const type of ['1', '2', 'g', 'L']) { assert.throws(() => parse(Buffer.concat([member('events.jsonl', 'x', type), archive(false)]))); cases++; }
  assert.throws(() => parse(Buffer.concat([member('../events.jsonl', 'x'), archive(false)]))); cases++;
  assert.throws(() => parse(Buffer.concat([member('events.jsonl', 'x'), archive(false)]))); cases++;
  assert.throws(() => parse(Buffer.concat([member('././@PaxHeader', record('mtime', '1'), 'x'), Buffer.alloc(1024)]))); cases++;
  assert.throws(() => parse(Buffer.concat([member('././@PaxHeader', Buffer.from('99 mtime=1\n'), 'x'), archive(false)]))); cases++;
  assert.throws(() => P.accountMetadata({ version: 1, profiles: [{ ...JSON.parse(profile).profiles[0], access_token: 'synthetic-denied' }] })); cases++;
  const short = compressed.subarray(0, compressed.length - 7);
  await assert.rejects(async () => { const c = collector(); for await (const b of Readable.from([short]).pipe(createGunzip())) c.parser.push(b); c.parser.finish(); }); cases++;
  console.log(JSON.stringify({ scope: 'combined-real-store-streamed-tar-gzip', cases, ok: true, realFixtureOpened: false, hubStarted: false }));
};
if (require.main === module) module.exports.run().catch(() => { console.error('FAIL synthetic tar/gzip boundary control'); process.exitCode = 1; });
