'use strict';
const assert=require('node:assert/strict'),{test}=require('node:test');
const {argumentsFor,journalCheck,hostTarget,windowsProcessCommand}=require('./release-smoke.cjs');
test('Windows identity probe directly reads the captured PID with all three witnesses',()=>{
  const command=windowsProcessCommand(1234);
  assert(command.includes('[System.Diagnostics.Process]::GetProcessById(1234)'));
  assert(command.includes('pid=$p.Id')&&command.includes('birth=$p.StartTime.ToUniversalTime()')&&command.includes('executable=$p.MainModule.FileName'));
  assert(command.includes("$ErrorActionPreference='Stop'")&&command.includes('catch { exit 2 }'));
  assert(!/CimInstance|Win32_Process|hub.lock/.test(command));
});
test('Windows identity probe refuses missing, non-native and injectable PID values',()=>{
  for(const pid of [undefined,null,0,-1,1.5,'1234','1; exit 0'])assert.throws(()=>windowsProcessCommand(pid),/Captured native PID/);
});
test('help and native host identity do not launch a payload',()=>{
  assert.equal(argumentsFor(['--help']),null);
  assert.equal(hostTarget(),`${process.platform==='win32'?'windows':process.platform}-${process.arch}`);
});
test('relative paths and duplicate options cannot select a stage/home',()=>{
  assert.throws(()=>argumentsFor(['--stage','relative','--out','relative']),/Absolute/);
  assert.throws(()=>argumentsFor(['--stage','/x','--stage','/y']),/duplicate/);
});
test('the smoke journal guard rejects every send/wake path and enabling automation',()=>{
  const before=Buffer.from(JSON.stringify({$:'SettingSet',key:'mail.on',value:'off'})+'\n');
  const after=event=>Buffer.concat([before,Buffer.from(JSON.stringify(event)+'\n')]);
  assert.equal(journalCheck(before,before).noSendOrAgentTurn,true);
  for(const kind of ['MailPosted','RoomPosted','MessagePosted','RunSet','BotWoke','TurnQueued'])assert.throws(()=>journalCheck(before,after({$:kind})),/send or wake/);
  for(const key of ['mail.on','telemetry.share'])assert.throws(()=>journalCheck(before,after({$:'SettingSet',key,value:'on'})),/enabled/);
  assert.throws(()=>journalCheck(before,Buffer.from('bad')),/prefix/);
});
