const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const source = fs.readFileSync(path.join(__dirname, '../../src/server/effects/live-checkpoint.js'), 'utf8').replace(/^io_eff\(.*$/gm, '');
// a fresh effects module: a hub process started again (nothing in memory)
const load = () => { const effects = vm.createContext({require, Buffer}); vm.runInContext(source, effects); return effects; };
const effects = load();
const root = path.resolve(__dirname, '../../build/checkpoint-test-homes');
fs.mkdirSync(root, {recursive:true});
const record = (thread, text, gen = 123, anchor = 'question') => ({anchor, buffer:{thread, text, gen, trunc:false}});
const texts = (fx, home) => Object.fromEntries(JSON.parse(fx.checkpoint_read(home)).map(r => [r.buffer.thread, r.buffer.text]));

test('live buffers survive reload, remain private, and clear independently', () => {
  const home = fs.mkdtempSync(path.join(root,'stream-'));
  try {
    const first = record('thread/日本語', '日本語 🙂 line 8\n');
    const second = record('other','second stream');
    assert.equal(effects.checkpoint_write(home,JSON.stringify(first)), '');
    assert.equal(effects.checkpoint_write(home,JSON.stringify(second)), '');
    const loaded = JSON.parse(effects.checkpoint_read(home));
    assert.equal(loaded.find(r=>r.buffer.thread===first.buffer.thread).buffer.text,first.buffer.text);
    const file=effects.checkpoint_file(home,first.buffer.thread);
    if (process.platform !== 'win32') assert.equal(fs.statSync(file).mode & 0o777,0o600);
    assert.equal(effects.checkpoint_write(home,JSON.stringify(record(first.buffer.thread,''))), '');
    assert.deepEqual(JSON.parse(effects.checkpoint_read(home)), [second]);
    assert.equal(effects.checkpoint_sync(home,JSON.stringify([record('other','')])), '');
    assert.deepEqual(JSON.parse(effects.checkpoint_read(home)), []);
  } finally {fs.rmSync(home,{recursive:true,force:true});}
});

test('an incomplete temporary checkpoint does not replace the complete buffer', () => {
  const home = fs.mkdtempSync(path.join(root,'partial-'));
  try {
    const r={anchor:'q',buffer:{thread:'t',text:'visible text',gen:1,trunc:false}};
    assert.equal(effects.checkpoint_write(home,JSON.stringify(r)), '');
    fs.writeFileSync(effects.checkpoint_file(home,'t')+'.new','{"broken');
    assert.deepEqual(JSON.parse(effects.checkpoint_read(home)), [r]);
  } finally {fs.rmSync(home,{recursive:true,force:true});}
});

test('a delta is appended, not the reply so far rewritten', () => {
  const home = fs.mkdtempSync(path.join(root,'append-'));
  try {
    assert.equal(effects.checkpoint_append(home,'t','5','first'), 'new');
    assert.equal(effects.checkpoint_write(home,JSON.stringify(record('t','first',5))), '');
    const file = effects.checkpoint_file(home,'t'), size = fs.statSync(file).size;
    assert.equal(effects.checkpoint_append(home,'t','5',' 日本語\n"quoted"'), '');
    assert.equal(fs.statSync(file).size, size + Buffer.byteLength(JSON.stringify(' 日本語\n"quoted"') + '\n'));
    assert.equal(texts(effects,home).t, 'first 日本語\n"quoted"');
    // another stream (a cleared buffer's next gen) starts the file again
    assert.equal(effects.checkpoint_append(home,'t','6','again'), 'new');
    assert.equal(effects.checkpoint_write(home,JSON.stringify(record('t','again',6))), '');
    assert.equal(effects.checkpoint_append(home,'t','6',' and on'), '');
    assert.equal(texts(effects,home).t, 'again and on');
    // a removed file is never appended to blindly
    assert.equal(effects.checkpoint_write(home,JSON.stringify(record('t',''))), '');
    assert.equal(effects.checkpoint_append(home,'t','6','more'), 'new');
  } finally {fs.rmSync(home,{recursive:true,force:true});}
});

test('a hub started again goes on with the journal it restored, minus a cut line', () => {
  const home = fs.mkdtempSync(path.join(root,'restart-'));
  try {
    assert.equal(effects.checkpoint_write(home,JSON.stringify(record('t','one',7))), '');
    assert.equal(effects.checkpoint_append(home,'t','7',' two'), '');
    const file = effects.checkpoint_file(home,'t');
    fs.appendFileSync(file, '" thr');
    const again = load();
    assert.equal(texts(again,home).t, 'one two');
    assert.equal(again.checkpoint_append(home,'t','7',' three'), '');
    assert.equal(texts(again,home).t, 'one two three');
    assert.equal(again.checkpoint_append(home,'t','8',' other gen'), 'new');
  } finally {fs.rmSync(home,{recursive:true,force:true});}
});

test('a long journal is written whole again, and a restored reply keeps the cap', () => {
  const home = fs.mkdtempSync(path.join(root,'long-'));
  try {
    assert.equal(effects.checkpoint_write(home,JSON.stringify(record('t','s',9))), '');
    const piece = 'x'.repeat(65536);
    let answer = '';
    for (let i = 0; i < 80 && answer === ''; i++) answer = effects.checkpoint_append(home,'t','9',piece);
    assert.equal(answer, 'new');
    const r = JSON.parse(effects.checkpoint_read(home))[0];
    assert.equal(r.buffer.text.length, 262144);
    assert.equal(r.buffer.trunc, true);
    assert.equal(r.buffer.gen, 9);
  } finally {fs.rmSync(home,{recursive:true,force:true});}
});

test('at start only the restored journals stay; the first version\'s files are read', () => {
  const home = fs.mkdtempSync(path.join(root,'keep-'));
  try {
    assert.equal(effects.checkpoint_write(home,JSON.stringify(record('a','kept',1))), '');
    assert.equal(effects.checkpoint_write(home,JSON.stringify(record('b','stale',1))), '');
    const legacy = path.join(effects.checkpoint_dir(home), effects.checkpoint_name('c') + '.json');
    fs.writeFileSync(legacy, JSON.stringify(record('c','old format',1)));
    fs.writeFileSync(effects.checkpoint_file(home,'a') + '.new', '{"broken');
    assert.deepEqual(texts(effects,home), {a:'kept', b:'stale', c:'old format'});
    assert.equal(effects.checkpoint_keep(home, JSON.stringify(['a','c'])), '');
    assert.deepEqual(fs.readdirSync(effects.checkpoint_dir(home)).sort(), [effects.checkpoint_name('a') + '.jsonl', effects.checkpoint_name('c') + '.json'].sort());
    // the old file goes once its thread is written in the new form
    assert.equal(effects.checkpoint_write(home,JSON.stringify(record('c','new form',2))), '');
    assert.equal(fs.existsSync(legacy), false);
    assert.deepEqual(texts(effects,home), {a:'kept', c:'new form'});
  } finally {fs.rmSync(home,{recursive:true,force:true});}
});
