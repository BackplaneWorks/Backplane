// Combined-gate control: REAL emitted bridge + real Bend projection fixtures.
// No service, native app, device, cache file, installation or UI runtime claim.
import fs from 'node:fs';import vm from 'node:vm';import assert from 'node:assert/strict';
import {cbor,plain} from '../../src/web/wire.js';
const [bundlePath,fixturePath]=process.argv.slice(2);
if(!bundlePath||!fixturePath)throw Error('usage: node test/tools/native_thin_emitted_307265.mjs BUNDLE FIXTURE_JSON_OR_LOG');
const source=fs.readFileSync(bundlePath,'utf8'),raw=fs.readFileSync(fixturePath,'utf8');
let fixture;try{fixture=JSON.parse(raw);}catch{for(const line of raw.trim().split('\n').reverse()){try{fixture=JSON.parse(line);break;}catch{}}}
fixture = fixture && { ...fixture, open: fixture.open || fixture.shown, message: fixture.message?.message || fixture.message, search: fixture.search?.search || fixture.search };
assert(fixture?.open&&fixture.message&&fixture.search,'actual Bend fixture open/message/search required');
const target=fixture.target||{kind:'thread',id:'t'},hub='h';
assert.equal(target.kind,'thread');assert.equal(target.id,'t');
const head=(m,n)=>n<24?Buffer.from([(m<<5)|n]):n<256?Buffer.from([(m<<5)|24,n]):n<65536?Buffer.from([(m<<5)|25,n>>8,n&255]):Buffer.from([(m<<5)|26,n>>>24,(n>>>16)&255,(n>>>8)&255,n&255]);
const encode=v=>v===null?Buffer.from([246]):typeof v==='boolean'?Buffer.from([v?245:244]):typeof v==='number'?(v>4294967295?Buffer.concat([Buffer.from([199]),encode(String(v))]):head(0,v)):typeof v==='string'?Buffer.concat([head(3,Buffer.byteLength(v)),Buffer.from(v)]):Array.isArray(v)?Buffer.concat([head(4,v.length),...v.map(encode)]):Buffer.concat([head(5,Object.keys(v).length),...Object.entries(v).flatMap(([k,x])=>[encode(k),encode(x)])]);
let checks=0;const checked=(yes,message)=>{assert(yes,message);checks++;};
function client(epoch='1'){
 const context=vm.createContext({console,Date});vm.runInContext(source,context,{timeout:10000,filename:bundlePath});
 const call=(method,...args)=>{context.__args=args;context.__method=method;return vm.runInContext('Backplane[__method](...__args)',context,{timeout:10000});};
 const out=(method,...args)=>JSON.parse(call(method,...args));
 const dictionaries=JSON.parse(call('headerKeys'));checked(dictionaries.keys.length>0&&dictionaries.words.length>0,'real emitted dictionaries required');
 const requests=o=>o.cmds.filter(c=>c.type==='send').map(c=>plain(cbor(new Uint8Array(Buffer.from(c.data,'base64')),dictionaries.keys,dictionaries.words)));
 const recv=(message,tag=epoch,quiet=false)=>out('recv',hub,encode(message).toString('base64'),tag,quiet,false);
 return{call,out,requests,recv,epoch};
}
const greeting=fixture.greeting||{t:'view',version:1,renderer:'mobile',generation:'1',epoch:'origin',limits:{replyBytes:262144,entries:32,recent:24,searchResults:25,searchCandidates:128,chunkBytes:65536}};
function start(c,drafts={}){c.call('start','native-emitted-test',JSON.stringify(drafts));c.call('hubs',[hub]);c.call('local','mailroom-renderer','ios-v1');c.call('connection',hub,c.epoch,'view-v1');const selected=c.out('act','select','h|t');checked(c.requests(selected).length===0,'pre-greeting selection has no effects');}
function begin(c){const o=c.recv(greeting),requests=c.requests(o);checked(o.sync.some(s=>s.phase==='presenting'&&s.mode==='view-v1'&&s.remoteReady===false),'presenting emitted before current request');checked(requests.length===1&&requests[0].m==='view.open','only bounded view.open on startup');checked(requests[0].p.target.id==='t','actual selected target');return requests[0];}
function respond(c,request,payload,quiet=false){
 const result={...payload,version:1,hub,generation:greeting.generation,epoch:greeting.epoch,target:request.p.target,requestRevision:request.p.requestRevision,viewId:request.p.viewId||'fixture-view',viewRevision:payload.viewRevision||'1',renderer:'mobile'};
 return c.recv({t:'reply',id:request.id,ok:true,result},c.epoch,quiet);
}
{
 const p=client();start(p);const first=begin(p);respond(p,first,fixture.open);
 const changed={t:'view.changed',version:1,generation:greeting.generation,epoch:greeting.epoch,viewId:'fixture-view',target:first.p.target,viewRevision:'2'};
 const update=p.requests(p.recv(changed))[0];
 checked(p.requests(p.out('projectSearchQuery',1,1,'older')).length===0,'busy query waits for current presentation');
 checked(p.requests(p.out('projectSearchQuery',1,2,'')).length===0,'empty query replaces old pending query');
 const resumed=p.requests(respond(p,update,fixture.open));
 checked(resumed.length===1&&resumed[0].m==='view.action'&&resumed[0].p.action==='proj-find-q'&&resumed[0].p.value==='','actual emitted bridge sends latest full empty query once');
}
{
 const p=client();start(p);const first=begin(p);respond(p,first,fixture.open);
 const query=p.requests(p.out('projectSearchQuery',1,1,''))[0];
 checked(query?.p.action==='proj-find-q','actual emitted empty full query requests filter');
 checked(p.requests(p.out('projectSearchGo',1,1,'','h|missing')).length===0,'actual emitted Go waits its accepted query');
 const result=respond(p,query,fixture.open);
 checked(!p.requests(result).some(r=>r.p.action==='proj-go'),'actual emitted missing project cannot invoke creating fallback');
 p.out('projectSearchEnd',2);
 checked(p.requests(p.out('projectSearchQuery',1,3,'stale')).length===0,'actual emitted scope tombstone refuses old edit');
}
{
 const p=client();start(p);const first=begin(p);respond(p,first,fixture.open);
 checked(p.requests(p.out('viewerAction',hub,'h|other',1,'board')).length===0,'emitted viewer rejects stale native owner');
 const out=p.out('viewerAction',hub,'h|t',2,'board'),request=p.requests(out)[0];
 checked(request?.p.action==='view'&&request.p.value==='board','emitted viewer requests exact owned source');
 checked(!out.sync.some(s=>s.viewerIntent!==undefined),'invocation is not viewer acceptance');
 const accepted=respond(p,request,fixture.open,true);
 checked(accepted.screen===null&&accepted.sync.some(s=>s.phase==='presented'&&s.viewerIntent===2),'quiet accepted viewer receipt survives without decoded picture');
}
for(const quiet of [false,true]) {
 const p=client();p.call('publicationAck');start(p);const first=begin(p);respond(p,first,fixture.open);
 const changed={t:'view.changed',version:1,generation:greeting.generation,epoch:greeting.epoch,viewId:'fixture-view',target:first.p.target,viewRevision:'2'};
 const update=p.requests(p.recv(changed))[0];checked(update?.m==='view.update','actual emitted change requests bounded update');
 checked(p.requests(p.recv({...changed,viewRevision:'3'})).length===0,'actual emitted newer change coalesces');
 const picture=respond(p,update,{...fixture.open,viewRevision:'1'},quiet);
 checked(p.requests(picture).length===0&&!picture.sync.some(x=>x.phase==='presenting'),'actual emitted accepted picture precedes follow-up');
 checked(quiet ? picture.screen===null : !!picture.screen?.thread,'actual emitted current picture draw honors quiet');
 checked(p.requests(p.out('pulse')).length===0,'actual emitted unacknowledged picture prevents follow-up');
 if(quiet){const flushed=p.out('flush',false);checked(!!flushed.screen?.thread&&p.requests(flushed).length===0,'actual emitted flush publishes withheld picture without retiring it');}
 p.call('published');const follow=p.requests(p.out('pulse'));checked(follow.length===1&&follow[0].m==='view.update','actual emitted acknowledgment schedules one follow-up');
 const completed=respond(p,follow[0],{...fixture.open,viewRevision:'3'});checked(p.requests(completed).length===0,'actual emitted newest revision ends refresh');
}
const c=client();start(c,{'h|t':'private draft','h|other':'other draft'});const open=begin(c);
const ancillary=[{t:'info',info:{pair:'','pair.qr':''}},{t:'hb'},
 {t:'info',info:{pair:'sanitized-fixture-link','pair.qr':'sanitized-fixture-qr'}}];
const pendingDeadline=c.out('pulse').deadline, pendingCompact=c.call('savePresentation');
for(const frame of ancillary){const inert=c.recv(frame);
 checked(inert.inert===true&&inert.screen===null&&inert.sync.length===0&&c.requests(inert).length===0,'actual inert ancillary before reply neither fails nor draws');
 checked(inert.deadline===pendingDeadline&&c.call('savePresentation')===pendingCompact,'actual inert ancillary preserves original timeout and local choices');}
checked(!JSON.stringify(open).includes('private draft')&&!JSON.stringify(open).includes('other draft'),'read startup carries no thread drafts');
const ready=respond(c,open,fixture.open);
checked(ready.sync.some(s=>s.phase==='presented'&&s.remoteReady===true&&s.requestRevision===open.p.requestRevision),'only exact current reply presents');
checked(ready.screen?.thread?.id===hub+'|'+target.id,'actual Bend native selected screen visible');
checked(ready.screen.thread.entries.length<=33,'bounded historical+live entries');
checked(ready.screen.thread.entries.some(e=>e.text.length>0),'real bounded current text');
checked(c.requests(ready).length===0,'presentation never starts canonical history');
for(const frame of ancillary){const inert=c.recv(frame);
 checked(inert.inert===true&&inert.screen===null&&inert.sync.length===0&&c.requests(inert).length===0,'actual inert ancillary after reply cannot revoke ready or redraw');}
const local=c.out('quiet','draft','newer local draft');checked(c.requests(local).length===0,'draft has no wire request');checked(local.cmds.some(x=>x.type==='keep'&&x.thread==='h|t'&&x.text==='newer local draft'),'local draft keep scoped');
const compact=c.call('savePresentation');const choices=JSON.parse(compact);checked(choices.version===1&&choices.hubs.length===1,'bounded compact blob');checked(!compact.includes('newer local draft')&&!compact.includes('other draft')&&!compact.includes('outbox')&&!compact.includes('requestRevision'),'compact excludes private state/authority');
const part=fixture.open.window.partial.find(p=>p.id===fixture.message.id);checked(!!part,'actual fixture exposes explicit current partial cursor');
const more=c.out('act','thin-message-more',JSON.stringify({entryId:part.id,cursor:part.cursor}));const moreRequests=c.requests(more);checked(moreRequests.length===1&&moreRequests[0].m==='view.message'&&moreRequests[0].p.cursor==='','first explicit More starts bounded full-body assembly');checked(!more.sync.some(s=>s.phase==='presenting'||s.phase==='presented'),'message read never changes readiness');
const completed=respond(c,moreRequests[0],{message:fixture.message,remoteReady:false,cmds:[]});const body=completed.screen?.thread?.entries.find(e=>e.id===part.id);checked(body?.text===fixture.message.text&&body?.partial?.complete===true,'exact complete actual Bend message chunk displayed');checked(c.requests(completed).length===0,'no automatic next message read');
c.out('act','find-open','search');const searched=c.out('act','find-q',fixture.search.query),searchRequests=c.requests(searched);checked(searchRequests.length===1&&searchRequests[0].m==='view.search','existing Find issues explicit bounded hub search');checked(searched.sync.length===0,'search issues no readiness transition');const found=respond(c,searchRequests[0],{search:fixture.search,remoteReady:false,cmds:[]});checked(found.sync.length===0&&found.screen?.find?.mode==='search','actual Find result has no readiness authority');checked(found.screen.find.rows.length<=26,'search result rows bounded plus explicit More');
const stale=c.recv({t:'reply',id:open.id,ok:true,result:fixture.open},'stale-native-epoch');checked(stale.sync.length===0&&c.requests(stale).length===0,'stale native epoch has no effects');
const reopened=client('2');reopened.call('start','native-emitted-test','{}');const loaded=reopened.out('loadPresentation',compact);checked(loaded.screen===null&&loaded.cmds.length===0&&loaded.sync.length===0,'compact restore is choices-only');reopened.call('hubs',[hub]);reopened.call('local','mailroom-renderer','ios-v1');reopened.call('connection',hub,'2','view-v1');reopened.call('act','select','h|t');const request=begin(reopened);checked(request.p.target.id==='t','process fresh VM preserves actual selected target');const fresh=respond(reopened,request,fixture.open);checked(fresh.screen?.thread?.id===hub+'|'+target.id,'fresh actual emitted view after compact reopen');
console.log(JSON.stringify({pass:true,checks,bundle:bundlePath,fixture:fixturePath,scope:'Actual emitted Bend bridge + actual pure server fixtures; no native app/service/device/UI or startup-timing acceptance'}));
