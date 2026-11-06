// Records what presenter (bp-policy) answers to the thread-row actions, as fixtures for native/host/src/commands/actions.rs.
// usage: bun test/commands/actions-fixture.mjs BP_POLICY_BIN PORT > test/commands/fixtures/actions.json   (a finished binary, fixture log only)
import {spawn} from 'node:child_process';import net from 'node:net';
const [bin,port]=process.argv.slice(2);
const p=spawn('sh',['-c',`ulimit -s 1048576; exec ${bin} ${port}`],{stdio:['ignore','pipe','inherit']});
await new Promise(r=>p.stdout.once('data',r));
const s=net.connect(Number(port),'127.0.0.1');await new Promise(r=>s.once('connect',r));
let buf='',w=null;s.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);w?.(l)}});
const ask=l=>new Promise(r=>{w=r;s.write(l+'\n')});
const log=[{$:'ProjectCreated',id:'p1',title:'proj',root:'/work/proj',at:1700000000},
 {$:'ThreadCreated',id:'t1',project:'p1',title:'One',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:1700000001},
 {$:'ThreadCreated',id:'t2',project:'p1',title:'Two',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:1700000002}];
await ask('F '+JSON.stringify({t:'log',since:0,origin:'1',version:'x',info:{},items:log}));
let session={sel:'',rid:1,drafts:{},open:{},adding:false,panel:false,info:{}};
const base={hub:'h',cid:'c',target:{kind:'recent',id:''},nativeSidebar:0,epoch:'1',generation:'1',viewId:'v1',requestRevision:1,version:1};
const steps=[['noop','x'],['row-menu','t2'],['row-menu','t2'],['proj-menu','p1'],['menu-close',''],['row-menu','t1'],['row-pin','t1'],['row-unpin','t1'],['row-settle','t2'],['row-unsettle','t2'],
 ['row-snooze','t2|1h'],['row-snooze','t2|tomorrow'],['row-snooze','t2|off'],['row-menu','t1'],['row-delete','t1'],['row-archive','t1'],['row-unarchive','t1'],
 ['row-rename','t2'],['rename-no','t2'],['row-rename','t2'],['rename-save','t2'],['rename-save','t1\u001fNew name'],['rename-save','t1\u001f   '],['rename-save',''],['row-menu','t2'],['new-thread','p1'],['new-thread','']];
// the selected thread's own actions (the header's buttons, the keys): the thread is the target's
const tsteps=[['pin',''],['unpin',''],['settle',''],['unsettle',''],['archive',''],['unarchive',''],['snooze','1h'],['snooze','tomorrow'],['snooze','off'],['row-menu','t1'],['settle','']];
const out=[];
for(const [action,value] of steps){
  const r=JSON.parse(await ask('V '+JSON.stringify({...base,session,action,value})));
  out.push({action,value,session_in:session,session:r.session,cmds:r.cmds,nextTarget:r.nextTarget??null});
  if(r.session)session=r.session;
}
for(const [action,value] of tsteps){
  const target={kind:'thread',id:'t1'};
  const r=JSON.parse(await ask('V '+JSON.stringify({...base,target,session,action,value})));
  out.push({action,value,target,session_in:session,session:r.session,cmds:r.cmds,nextTarget:r.nextTarget??null});
  if(r.session)session=r.session;
}
console.log(JSON.stringify({log,titles:{t1:'One',t2:'Two'},steps:out}));
p.kill();process.exit(0);
