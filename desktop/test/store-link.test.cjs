'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),vm=require('node:vm');
// Execute the production effect, with the real filesystem. The Windows CI
// invokes this as a standard user after proving file symlinks are forbidden.
function effect(platform=process.platform,files=fs){
  const ctx={process:{platform},require:n=>n==='fs'?files:require(n)};
  const source=fs.readFileSync(path.join(__dirname,'../../src/server/effects/storeio.js'),'utf8');
  vm.createContext(ctx);vm.runInContext(source.slice(0,source.indexOf('function store_sync(')),ctx);
  return ctx.store_temp;
}
test('Windows rebuild reads the same log through a hardlink and unlink leaves the source intact',t=>{
  const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-log 日本語 '));t.after(()=>fs.rmSync(home,{recursive:true,force:true}));
  const log=path.join(home,'events.jsonl');fs.writeFileSync(log,'original\n');
  const files=Object.create(fs);files.symlinkSync=()=>{throw Error('Windows symlink forbidden')};
  const temp=effect('win32',files)(home),linked=path.join(temp,'events.jsonl');
  assert.equal(fs.lstatSync(linked).isSymbolicLink(),false);assert.equal(fs.statSync(log).nlink,2);
  assert.equal(fs.statSync(linked).ino,fs.statSync(log).ino);
  fs.appendFileSync(log,'next\n');assert.equal(fs.readFileSync(linked,'utf8'),'original\nnext\n');
  fs.unlinkSync(linked);fs.rmdirSync(temp);assert.equal(fs.readFileSync(log,'utf8'),'original\nnext\n');
});
test('POSIX rebuild retains its symlink behavior',{skip:process.platform==='win32'},t=>{
  const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-log-'));t.after(()=>fs.rmSync(home,{recursive:true,force:true}));
  fs.writeFileSync(path.join(home,'events.jsonl'),'keep');const temp=effect()(home);
  assert(fs.lstatSync(path.join(temp,'events.jsonl')).isSymbolicLink());
});
test('failed link creation preserves the log and removes only its empty rebuild directory',t=>{
  const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-log-'));t.after(()=>fs.rmSync(home,{recursive:true,force:true}));
  fs.writeFileSync(path.join(home,'events.jsonl'),'keep');const files=Object.create(fs);
  files.linkSync=()=>{throw Object.assign(Error('hardlink denied'),{code:'EACCES'})};
  assert.throws(()=>effect('win32',files)(home),/hardlink denied/);
  assert.deepEqual(fs.readdirSync(home),['events.jsonl']);assert.equal(fs.readFileSync(path.join(home,'events.jsonl'),'utf8'),'keep');
});
