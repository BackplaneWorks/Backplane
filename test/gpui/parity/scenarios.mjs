// Scenarios: deterministic client programs. renderer: mobile only (phones, GPUI); the web page is not a target.
import {connect,httpProbe,attempt,sleep,privateLeaks} from './lib.mjs';
const until=async(f,ms=20000)=>{const end=Date.now()+ms;for(;;){const v=await f();if(v)return v;if(Date.now()>end)return null;await sleep(50)}};
const threadsOf=r=>(r?.result?.shown?.projects||[]).flatMap(p=>p.threads||[]);
const fixtureThread=r=>threadsOf(r).find(t=>/^Fixture long/.test(t.title||t.label||''))||threadsOf(r)[0];

export const SCENARIOS={
  // private lines: the log holds DeviceRegistered/DeviceDropped and ride.*, launch.input.*, fallback.input.* values (hubs.mjs privateLines).
  // No frame to any client may carry them: not the join's `log`, not a resumed `changes` stream, not a live write, not a view.
  async private({hub,rec}){
    const a=await connect({hub,rec,name:'full',profile:'raw'});await a.settle(900);
    const log=a.inbox.find(x=>x.t==='log');
    const b=await connect({hub,rec,name:'resumed',profile:'raw',query:`since=3&origin=${log?.origin}`});await b.settle(900);
    const th='t1700000002-3';
    await a.rpc('setting.set',{key:'ride.'+th,value:'PRIVLIVE-0006 a live riding write'});
    await a.rpc('setting.set',{key:'launch.input.'+th,value:'PRIVLIVE-0007 a live input'});
    await a.settle(700);
    const g=await connect({hub,rec,name:'gpui',profile:'gpui'});if(!g.failed){await g.vcall('view.open');await g.vcall('view.update');await g.settle(500)}
    await a.end(500);await b.end(500);if(!g.failed)await g.end(500);
    const leaks=privateLeaks(rec.frames);
    rec.push('private','out','check',null,{privateLeaks:leaks});
  },
  // 1. HTTP + raw join
  async join({hub,rec}){
    await httpProbe(hub,'/hello',rec,'http');
    const c=await connect({hub,rec,name:'raw',profile:'raw'});await c.settle(800);c.send({id:1,m:'ping',p:{}});await c.end(600);
  },
  // 2. bad auth and bad input
  async badauth({hub,rec}){
    for(const [name,mode] of [['notoken','none'],['wrong','wrong'],['badchars','badchars'],['empty-after-eq','empty']])await attempt(hub,rec,name,mode);
    const c=await connect({hub,rec,name:'junk',profile:'raw'});await c.settle(500);
    c.sendRaw(new Uint8Array([0xff,0xfe,0x00]));await c.settle(500);
    c.send({id:2,m:'no.such.method',p:{}});await c.settle(300);
    c.send({id:3,m:'view.open',p:{version:1}});await c.settle(300);
    await c.end(600);
    await httpProbe(hub,'/nope',rec,'http');
  },
  // 3/4. a full mobile-renderer session: open, project, select, draft, send, turn, deltas
  async session(prof,{hub,rec}){
    const c=await connect({hub,rec,name:prof,profile:prof});if(c.failed)return;
    const raw=await connect({hub,rec,name:prof+'-raw',profile:'raw'});
    let r=await c.vcall('view.open');
    r=await c.vcall('view.update');
    const pr=await raw.rpc('project.add',{path:hub.projectDir});
    await until(async()=>{const x=await c.poll();return threadsOf(x).length>0},15000);
    c.resync();
    r=await c.vcall('view.update');
    const t=fixtureThread(r);
    await c.vcall('view.action',{action:'select',value:t?t.id:''});
    await c.settle(1000);
    r=await c.vcall('view.update');
    const tid=c.view.target.id;
    c.view.session.drafts[tid]='hello parity';
    await c.vcall('view.action',{action:'draft',value:'hello parity'});
    await c.vcall('view.action',{action:'send',value:''});
    // wait for the stand-in's turn to finish
    await until(async()=>{const x=await c.poll();const th=x?.result?.shown?.thread;return th&&th.state==='idle'&&(th.entries||[]).some(e=>e.kind==='assistant')},30000);
    c.resync();await c.vcall('view.update');await c.settle(600);
    // segmented / paged history: ask for older pages while the window says there are some
    for(let i=0;i<3;i++){const w=c.view.last?.window;if(!w||!w.older)break;await c.vcall('view.page',{cursor:w.older});}
    if(c.P.delta){ // the end check: what the ops rebuild equals a whole view asked for with no offer
      await c.settle(1500);const a=await c.vcall('view.update');const bodyA=JSON.stringify({shown:c.view.last.shown,window:c.view.last.window});
      c.view.noOffer=true;const w=await c.vcall('view.update');c.view.noOffer=false;
      const bodyB=JSON.stringify({shown:w.result.shown,window:w.result.window});
      rec.push(prof,'out','check',null,{deltaRebuildEqualsWhole:bodyA===bodyB,offered:!!a?.result?.viewDelta,ops:!!a?.result?.viewDelta?.ops});
    }
    await c.end(1500);await raw.end(500);
  },
  // 5. reconnect: resume from the log position the client already has
  async reconnect({hub,rec}){
    const a=await connect({hub,rec,name:'first',profile:'raw'});await a.settle(800);
    const log=a.inbox.find(x=>x.t==='log');const since=(log?.since||0)+(log?.items?.length||0);const origin=log?.origin;
    await a.end(500);
    const b=await connect({hub,rec,name:'writer',profile:'raw'});await b.settle(300);
    await b.rpc('project.add',{path:hub.projectDir+'-2'});await b.settle(500);
    const c=await connect({hub,rec,name:'resumed',profile:'raw',query:`since=${since}&origin=${origin}`});await c.settle(900);
    await b.end(500);await c.end(800);
  },
  // 8. the Settings requests (docs/gpui/LANES.md, lane settings): setting.set and its refusals, the telemetry choice (Off, a stale On, a retry), the
  //    secret box (a planted value must come back in no frame), zone report/set, diagnostics, models.refresh, update.check, app.update,
  //    improvements.context/report and accounts.default. Fixed values only; the vault uses its key file (BACKPLANE_VAULT_KEYRING=file).
  async settings({hub,rec}){
    const c=await connect({hub,rec,name:'raw',profile:'raw'});await c.settle(800);
    const call=async(m,p)=>{await c.rpc(m,p);await c.settle(150)};
    for(const [k,v] of [['onboard.done','1'],['onboard.uber','1'],['sidebar.view','active'],['peer.own.x','1'],['account.thread.t1.claude','{}'],['accounts.default','x'],['onboard.done','']])await call('setting.set',{key:k,value:v});
    await call('telemetry.choice',{value:'on',expected:'stale',decision:'d1',popup:false});
    await call('telemetry.choice',{value:'off',expected:'',decision:'d2',popup:false});
    await call('telemetry.choice',{value:'off',expected:'',decision:'d2',popup:false});
    await call('setting.set',{key:'telemetry.share',value:'on'});
    await call('secret.list',{});
    await call('secret.set',{name:'Parity Secret',value:'PARITY-planted-9f3k27',purpose:'parity'});
    await call('secret.set',{name:'x',value:'short'});
    await call('secret.set',{name:'',value:'PARITY-planted-9f3k27'});
    await call('setting.set',{key:'note.parity',value:'a note PARITY-planted-9f3k27 in a setting'});
    await call('secret.list',{});await call('secret.dismiss',{name:'parity-secret'});await call('secret.del',{name:'parity-secret'});await call('secret.del',{name:'parity-secret'});await call('secret.list',{});
    for(const z of ['Europe/Helsinki','not a zone','Europe/Helsinki'])await call('zone.report',{zone:z});
    for(const z of ['Europe/Helsinki','Mars/Olympus','a b','auto'])await call('zone.set',{zone:z});
    await call('diagnostics.status',{});await call('diagnostics.control',{action:'bogus'});await call('diagnostics.control',{action:'off'});await call('diagnostics.control',{action:'on'});await call('diagnostics.control',{action:'clear'});
    await call('models.refresh',{});await call('update.check',{});await call('app.update',{});
    await call('improvements.context',{sourceThread:'missing'});await call('improvements.report',{thread:'missing'});
    await call('accounts.default',{provider:'claude',profile:'',epoch:0});await call('accounts.default',{provider:'claude',profile:'nope',epoch:1});await call('accounts.default',{provider:'grok',profile:'',epoch:0});
    await c.end(800);
  },
  // 7. a thread id that names no folder (unknown, empty, deleted) on files, git, mech and the like (post-#960: every one refuses, none schedules a job)
  async unresolved({hub,rec}){
    const c=await connect({hub,rec,name:'raw',profile:'raw'});await c.settle(800);
    const THREADS=['missing','','t1700000003-4'];
    const CALLS=[['files.dir',{path:''}],['files.dir',{path:'etc'}],['files.dir',{path:'..'}],['files.list',{}],['git.status',{}],['git.diff',{}],['mech.list',{}],['boards.list',{}],['history.refs',{}],['review.bom',{}],['review.checks',{}]];
    for(const th of THREADS)for(const [m,x] of CALLS)await c.rpc(m,{thread:th,...x});
    for(const th of THREADS)for(const route of ['/view/get','/files/get','/files/zip'])await httpProbe(hub,`${route}?thread=${encodeURIComponent(th)}&path=note.txt`,rec,'raw',{auth:true});
    await c.end(800);
  },
  // 8. the thread, project and queue requests (commands lane): every flag, order, tree and queue event, and their refusals
  async commands({hub,rec}){
    const c=await connect({hub,rec,name:'raw',profile:'raw'});await c.settle(800);
    const T1='t1700000001-2',T2='t1700000002-3',P='p1700000000-1';
    const r=await c.rpc('thread.create',{project:P,title:'Made by parity'});const A=r?.thread;
    const r2=await c.rpc('thread.create',{project:P,env:'local',parent:T1});const B=r2?.thread;
    await c.rpc('thread.create',{project:P,parent:'ghost'});
    await c.rpc('thread.create',{project:P,creationKey:'0123456789abcdef0123456789abcdef'});
    await c.rpc('thread.create',{project:P,creationKey:'0123456789abcdef0123456789abcdef'});
    await c.rpc('thread.create',{project:P,creationKey:'short'});
    for(const m of ['thread.pin','thread.unpin','thread.settle','thread.unsettle'])await c.rpc(m,{thread:T1});
    await c.rpc('thread.rename',{thread:T1,title:'  Renamed by parity  '});await c.rpc('thread.rename',{thread:T1,title:'   '});
    for(const p of [{for:'1h'},{for:'off'},{until:4102444800}])await c.rpc('thread.snooze',{thread:T1,...p});
    await c.rpc('thread.reorder',{section:'active',thread:T2,ids:[T2,T1,A]});await c.rpc('thread.reorder',{section:'pinned',thread:A,ids:[A,T2]});await c.rpc('thread.reorder',{section:'settled',thread:A,ids:[]});
    await c.rpc('project.reorder',{project:P,ids:[P]});
    await c.rpc('thread.detach',{thread:'ghost'});await c.rpc('thread.reparent',{thread:'ghost',parent:T1});await c.rpc('thread.reparent',{thread:T2,parent:T1});await c.rpc('thread.reparent',{thread:T1,parent:T2});await c.rpc('thread.detach',{thread:T2});
    for(const [m,p] of [['queue.edit',{msg:'q1',text:'edited'}],['queue.remove',{msg:'q1'}],['queue.move',{msg:'q1',to:1}]])await c.rpc(m,{thread:T2,...p});
    await c.rpc('thread.modes',{thread:T1,runtime:'auto',interaction:'plan',model:'opus'});await c.rpc('thread.effort',{thread:T1,effort:'high'});await c.rpc('thread.provider',{thread:T1,provider:'claude'});await c.rpc('thread.provider',{thread:T1,provider:'codex',model:'gpt-x',effort:'high'});await c.rpc('thread.provider',{thread:T1,provider:'claude'});await c.rpc('queue.steer',{thread:T2,msg:'nope'});await c.rpc('queue.resume',{thread:T2});await c.rpc('fs.list',{path:hub.projectDir});await c.rpc('fs.mkdir',{path:hub.projectDir+'/made'});await c.rpc('fs.mkdir',{path:hub.projectDir+'/made'});await c.rpc('project.create',{path:'  '});await c.rpc('project.new',{path:hub.projectDir+'/fresh'});const F=(await c.rpc('thread.fork',{thread:T2}))?.thread;await c.rpc('thread.back',{thread:F||'x'});await c.rpc('thread.back',{thread:T1});await c.rpc('queue.steer',{thread:F||'x',msg:'handoff:'+T2});await c.rpc('thread.fork',{thread:'ghost'});await c.rpc('thread.snooze',{thread:T1,for:'tomorrow'});await c.rpc('thread.archive',{thread:A});await c.rpc('thread.unarchive',{thread:A});await c.rpc('thread.delete',{thread:B});
    await c.rpc('project.rename',{project:P,title:' Parity project '});await c.rpc('project.rename',{project:P,title:' '});
    await c.rpc('project.move',{project:P,path:'relative'});
    const pa=await c.rpc('project.add',{path:hub.projectDir+'-2'});
    await c.rpc('project.remove',{project:'p-unknown'});
    await c.settle(1200);await c.end(800);
  },
};
