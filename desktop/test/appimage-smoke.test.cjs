'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const {argumentsFor,wrapperWitness,linuxSandbox,journalCheck}=require('./appimage-smoke.cjs');
test('exact wrapper and digest are required',()=>{
  assert.throws(()=>argumentsFor(['--stage','/stage','--out','/never-existing-evidence']),/Use/);
  assert.throws(()=>argumentsFor(['--stage','/stage','--out','/never-existing-evidence','--appimage','relative','--appimage-sha256','a'.repeat(64)]),/Absolute/);
  assert.throws(()=>argumentsFor(['--stage','/stage','--out','/never-existing-evidence','--appimage','/a.AppImage','--appimage-sha256','unknown']),/SHA256/);
});
test('wrapper parser retains exact supplied executable',()=>{
  const v=argumentsFor(['--stage','/stage','--out','/never-existing-evidence','--appimage','/retained.AppImage','--appimage-sha256','a'.repeat(64)]);
  assert.equal(v.appimage,'/retained.AppImage');assert.equal(v.appimageSha256,'a'.repeat(64));
});
test('actual wrapper runtime child is required, extracted stage cannot substitute',()=>{
  assert.throws(()=>wrapperWitness('/a.AppImage','/owned/extracted','/stage/runtime/backplane-desktop','/owned'),/AppRun/);
  assert.throws(()=>wrapperWitness('/a.AppImage','/elsewhere/extracted','/elsewhere/extracted/usr/lib/backplane/runtime/backplane-desktop','/owned'),/escaped/);
  assert.throws(()=>wrapperWitness('/a.AppImage',undefined,'/exe','/owned'));
});
test('runtime witness binds only the owned AppImage child path',()=>{
  const p='/owned/extracted/usr/lib/backplane/runtime/backplane-desktop';
  assert.equal(wrapperWitness('/a.AppImage','/owned/extracted',p,'/owned').actualExecutable,p);
});
test('existing sandbox and no-send journal checks are retained',()=>{
  assert.equal(typeof linuxSandbox,'function');
  const before=Buffer.from('');
  assert.throws(()=>journalCheck(before,Buffer.from(JSON.stringify({$: 'MessagePosted'})+'\n')),/send or wake/);
  assert.throws(()=>journalCheck(before,Buffer.from(JSON.stringify({$: 'SettingSet',key:'telemetry.share',value:'on'})+'\n')),/enabled/);
});
