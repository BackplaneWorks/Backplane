'use strict';
// Generated launcher, stub probe/stat/app only. No Electron, service, namespace
// change, privilege or host security policy is involved.
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),os=require('node:os'),path=require('node:path');
const {spawnSync}=require('node:child_process');
const {linuxLauncher}=require('../produce-release.cjs');
function fixture(t,probeExit){
 const root=fs.mkdtempSync(path.join(os.tmpdir(),"Backplane launcher 日本語 ' $() "));
 t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
 const bin=path.join(root,'tools'),runtime=path.join(root,'runtime');
 fs.mkdirSync(bin);fs.mkdirSync(runtime);
 for(const [name,source]of [['dirname','/usr/bin/dirname'],['sed','/bin/sed']])fs.symlinkSync(source,path.join(bin,name));
 if(probeExit!==undefined)fs.writeFileSync(path.join(bin,'unshare'),
  '#!/bin/sh\nprintf "probe:%s\\n" "$@" >> "$TRACE"\nexit '+probeExit+'\n',{mode:0o755});
 fs.writeFileSync(path.join(bin,'stat'),'#!/bin/sh\nprintf "%s\\n" "$SANDBOX_META"\n',{mode:0o755});
 fs.writeFileSync(path.join(runtime,'chrome-sandbox'),'stub helper, never executed\n');
 fs.writeFileSync(path.join(runtime,'backplane-desktop'),
  '#!/bin/sh\nprintf "app:%s\\n" "$@" >> "$TRACE"\nexit 23\n',{mode:0o755});
 const launcher=path.join(root,'backplane-open'),trace=path.join(root,'trace');
 fs.writeFileSync(launcher,linuxLauncher(),{mode:0o755});
 return {root,launcher,trace,helper:path.join(runtime,'chrome-sandbox'),env:{PATH:bin,TRACE:trace,SANDBOX_META:'1000:1000:755'}};
}
const linux={skip:process.platform!=='linux'};
const run=f=>spawnSync('/bin/sh',[f.launcher],{env:f.env,encoding:'utf8'});
test('usable user namespaces start the app with exact Unicode/metacharacter arguments',linux,t=>{
 const f=fixture(t,0),r=spawnSync('/bin/sh',[f.launcher,'--home','/tmp/private 日本語','literal $(no command)'],{env:f.env,encoding:'utf8'});
 assert.equal(r.status,23);assert.equal(r.stderr,'');
 assert.equal(fs.readFileSync(f.trace,'utf8'),'probe:--user\nprobe:--map-root-user\nprobe:/bin/true\napp:--home\napp:/tmp/private 日本語\napp:literal $(no command)\n');
});
test('denied namespaces and an unprivileged helper fail before Electron; quoted remedy names exact absolute helper',linux,t=>{
 const f=fixture(t,1),r=run(f);
 assert.equal(r.status,1);assert(!fs.readFileSync(f.trace,'utf8').includes('app:'));assert.equal(r.stdout,'');
 const command=r.stderr.split('\n').find(line=>line.startsWith('sudo '));assert(command);
 // Parse the printed command with harmless sudo stub: never run chown/chmod.
 const sudo=path.join(f.root,'tools/sudo');fs.writeFileSync(sudo,'#!/bin/sh\nprintf "%s\\n" "$@" >> "$REMEDY_TRACE"\n',{mode:0o755});
 const recorded=path.join(f.root,'remedy');const parsed=spawnSync('/bin/sh',['-c',command],{env:{...f.env,REMEDY_TRACE:recorded},encoding:'utf8'});
 assert.equal(parsed.status,0);assert.equal(fs.readFileSync(recorded,'utf8'),'chown\nroot:root\n'+f.helper+'\nchmod\n4755\n'+f.helper+'\n');
});
test('root:root mode4755 regular helper permits launch when namespaces are denied or unshare missing',linux,t=>{
 for(const probe of [1,undefined]){const f=fixture(t,probe);f.env.SANDBOX_META='0:0:4755';assert.equal(run(f).status,23);assert(fs.readFileSync(f.trace,'utf8').includes('app:'));}
});
test('wrong owner group or helper mode never substitutes for unavailable namespaces',linux,t=>{
 for(const meta of ['1000:0:4755','0:1000:4755','0:0:755','0:0:4777','0:0:6755','']){
  const f=fixture(t,1);f.env.SANDBOX_META=meta;assert.equal(run(f).status,1);assert(!fs.readFileSync(f.trace,'utf8').includes('app:'));
 }
});
test('missing or symlink helper and missing probe fail closed',linux,t=>{
 for(const linked of [false,true]){const f=fixture(t);fs.unlinkSync(f.helper);if(linked)fs.symlinkSync('backplane-desktop',f.helper);f.env.SANDBOX_META='0:0:4755';assert.equal(run(f).status,1);assert(!fs.existsSync(f.trace));}
});
test('launcher syntax is valid and no privileged command or sandbox bypass is executed',linux,()=>{
 assert.equal(spawnSync('/bin/sh',['-n'],{input:linuxLauncher(),encoding:'utf8'}).status,0);
 const commands=linuxLauncher().split('\n').filter(line=>/^(?:sudo |.*exec )/.test(line));
 assert.deepEqual(commands,['  exec "$BP_ROOT/runtime/backplane-desktop" "$@"','  exec "$BP_ROOT/runtime/backplane-desktop" "$@"']);
 assert(!linuxLauncher().includes('--no-sandbox'));
});
