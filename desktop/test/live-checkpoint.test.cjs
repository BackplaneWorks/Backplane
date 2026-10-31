const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const source = fs.readFileSync(path.join(__dirname, '../../src/server/effects/live-checkpoint.js'), 'utf8').replace(/^io_eff\(.*$/gm, '');
const effects = vm.createContext({require, Buffer});
vm.runInContext(source, effects);
const root = path.resolve(__dirname, '../../build/checkpoint-test-homes');
fs.mkdirSync(root, {recursive:true});

test('live buffers survive reload, remain private, and clear independently', () => {
  const home = fs.mkdtempSync(path.join(root,'stream-'));
  try {
    const record = (thread,text) => ({anchor:'question',buffer:{thread,text,gen:123,trunc:false}});
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
    assert.equal(effects.checkpoint_sync(home,'[]'), '');
    assert.deepEqual(JSON.parse(effects.checkpoint_read(home)), []);
  } finally {fs.rmSync(home,{recursive:true,force:true});}
});

test('an incomplete temporary checkpoint does not replace the complete buffer', () => {
  const home = fs.mkdtempSync(path.join(root,'partial-'));
  try {
    const record={anchor:'q',buffer:{thread:'t',text:'visible text',gen:1,trunc:false}};
    assert.equal(effects.checkpoint_write(home,JSON.stringify(record)), '');
    fs.writeFileSync(effects.checkpoint_file(home,'t')+'.new','{"broken');
    assert.deepEqual(JSON.parse(effects.checkpoint_read(home)), [record]);
  } finally {fs.rmSync(home,{recursive:true,force:true});}
});
