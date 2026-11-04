'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),vm=require('node:vm');
const {EventEmitter}=require('node:events'),{PassThrough}=require('node:stream');
function fixture(t,fail){
  const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-launch-failure-'));t.after(()=>fs.rmSync(home,{recursive:true,force:true}));
  const child=new EventEmitter();child.stderr=new PassThrough();child.unref=()=>{};
  const module={exports:{}};
  const ctx={module,Buffer,AbortSignal,Date,setTimeout,process:{...process,platform:'linux'},require:n=>n==='node:child_process'?{
    execFile(){},spawn(_binary,_args,opts){assert.equal(opts.stdio[2],'pipe');queueMicrotask(()=>fail(child));return child;}
  }:require(n)};
  vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../service.cjs'),'utf8'),ctx);
  const service=new module.exports.Service({home,binary:path.join(home,'missing-service'),web:home});service.discover=async()=>null;
  return service;
}
test('a service failure includes its final stderr and exit code',async t=>{
  const service=fixture(t,c=>{c.emit('exit',1);c.stderr.write('EPERM: symlink events.jsonl');c.emit('close',1,null)});
  await assert.rejects(service.start(),/Service exited \(1\):\nEPERM: symlink events.jsonl/);
});
test('spawn errors retain the native failure',async t=>{
  const service=fixture(t,c=>c.emit('error',Error('spawn ENOENT /packaged/service')));
  await assert.rejects(service.start(),/Cannot start the packaged service: spawn ENOENT/);
});
test('startup diagnostics retain only a bounded tail',async t=>{
  const service=fixture(t,c=>{c.stderr.write('x'.repeat(20000)+'final error');c.emit('close',2,null)});
  await assert.rejects(service.start(),e=>e.message.length<8300&&e.message.endsWith('final error'));
});
