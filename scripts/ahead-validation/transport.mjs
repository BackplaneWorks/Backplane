// Browser-free replay of the production transport controller. Synthetic hub
// replies replace IO; selection/retirement/paging/deduplication stay real.
import assert from 'node:assert/strict';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const root=resolve(process.argv[2]||'.');
const {ThinHost}=await import(pathToFileURL(resolve(root,'src/web/thin-host.js')));
const {ThinViews,VIEW_LIMITS}=await import(pathToFileURL(resolve(root,'src/web/thin.js')));
function fixture(initial={}){
  let session={sel:'A',drafts:{A:'draft A',B:'draft B'},rid:1};
  const sent=[],views=[],commands=[],states=[];
  const api={hub:'https://synthetic',cid:'fixture',now:()=>1,session:()=>session,
    target:s=>({kind:'thread',id:s.sel}),choice:(s,a,v)=>a==='select'?{...s,sel:v}:s,
    prepare:(s,p)=>({...p,rid:s.rid}),counter(){},send:r=>sent.push(structuredClone(r)),
    local(){},ack:s=>s,sessionChanged:s=>session=s,decode:n=>n,command:c=>commands.push(c),
    state:s=>states.push(s),view:n=>views.push(n),anchor:()=>({id:'row',offset:0}),blocked(){},
    readingMark:()=> 'source',warning(){}};
  const host=new ThinHost(api,ThinViews,2**47);
  const greeting={t:'view',version:1,renderer:'web',generation:'7',epoch:'synthetic',limits:{...VIEW_LIMITS}};
  const reply=(r=sent.at(-1),extra={})=>({t:'reply',id:r.id,ok:true,result:{version:1,hub:api.hub,generation:'7',epoch:'synthetic',
    requestRevision:r.p.requestRevision,target:r.p.target,viewId:r.p.viewId||'view-'+r.p.target.id,viewRevision:'1',remoteReady:true,
    renderer:'web',shown:{type:'text',text:'SYNTHETIC '+r.p.target.id},session:structuredClone(r.p.session||session),cmds:[],
    window:{ids:['row'],older:'older',hasMore:true,newer:0,anchor:{id:'row',offset:0},partial:[]},...extra}});
  host.greeting(greeting);host.receive(reply(undefined,initial));
  return {host,sent,views,commands,states,reply,session:()=>session};
}
const checks={};
const start=performance.now();
for(const [name,test] of Object.entries({
  pillIntent(){const f=fixture();assert(f.host.page());assert(f.host.newest(),'a click while paging is retained');
    f.host.receive(f.reply());assert.equal(f.sent.at(-1).p.cursor,'','newest request follows older reply');},
  restoredOlder(){const f=fixture({window:{ids:['old'],older:'older',hasMore:true,newer:6,anchor:{id:'old',offset:0},partial:[]}});
    assert.equal(f.host.atNewest(),false,'empty local trail is not proof of newest source');},
  unreadBoundary(){const f=fixture();f.host.away('A','first');f.host.away('A','second');assert.equal(f.host.awayOf('A'),'first');},
  staleReplies(){const f=fixture();f.host.update();const old=f.reply();f.host.request('select','B');const fresh=f.reply();
    f.host.receive(fresh);const count=f.views.length;f.host.receive(old);f.host.receive(fresh);
    assert.equal(f.views.length,count,'late and duplicate replies cannot repaint');assert.equal(f.session().sel,'B');
    assert.equal(f.session().drafts.A,'draft A');assert.equal(f.session().drafts.B,'draft B');},
  errorSelection(){const f=fixture();f.host.page();const r=f.sent.at(-1);f.host.receive({t:'reply',id:r.id,ok:false,error:{code:'STALE_CURSOR',message:'synthetic'}});
    f.host.request('select','B');f.host.receive(f.reply());assert.equal(f.session().sel,'B','view error leaves selection usable');},
})){try{test();checks[name]={pass:true};}catch(e){checks[name]={pass:false,reason:e.message};}}
const result={approach:'transport-simulator',pass:Object.values(checks).every(x=>x.pass),checks,seconds:(performance.now()-start)/1000,
  scope:'actual ThinHost/ThinViews, synthetic IO/session adapter; no DOM or service'};
console.log(JSON.stringify(result));if(!result.pass)process.exitCode=1;
