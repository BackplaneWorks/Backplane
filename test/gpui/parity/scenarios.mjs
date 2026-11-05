// Scenarios: deterministic client programs. renderer: mobile only (phones, GPUI); the web page is not a target.
import {connect,httpProbe,attempt,sleep} from './lib.mjs';
const until=async(f,ms=20000)=>{const end=Date.now()+ms;for(;;){const v=await f();if(v)return v;if(Date.now()>end)return null;await sleep(50)}};
const threadsOf=r=>(r?.result?.shown?.projects||[]).flatMap(p=>p.threads||[]);
const fixtureThread=r=>threadsOf(r).find(t=>/^Fixture long/.test(t.title||t.label||''))||threadsOf(r)[0];

export const SCENARIOS={
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
};
