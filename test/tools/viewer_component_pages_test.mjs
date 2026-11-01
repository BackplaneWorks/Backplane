import assert from 'node:assert/strict';
import {ViewerRpc} from '../../src/web/viewer-rpc.js';
const all=Array.from({length:5},(_,i)=>({id:`saved-uuid-${i}`,reference:`R${i}`}));
function page(offset=0) {
  const components=all.slice(offset,offset+2),next=offset+components.length;
  return {schema:1,source:'/saved/board.kicad_pcb',schematic:'/saved/board.kicad_sch',sourceRevision:'pcb-r1',schematicRevision:'sch-r1',fingerprint:'source-hash',thicknessMm:1.6,components,page:{offset,total:all.length,next:next===all.length?null:next,complete:next===all.length}};
}
function endpoint(transform=x=>x,afterReply=()=>{}) {
  const sent=[];
  const rpc=new ViewerRpc(request=>{
    sent.push(request);
    queueMicrotask(()=>{
      rpc.receive({t:'reply',id:request.id,ok:true,result:{components:transform(page(request.p.offset||0),sent.length)}});
      afterReply(rpc,sent.length);
    });
  });
  return {rpc,sent};
}
{
  const {rpc,sent}=endpoint(),data=await rpc.components('thread-a');
  assert.deepEqual(data.components,all);assert.equal(data.page.complete,true);assert.equal(data.page.next,null);
  assert.deepEqual(sent.map(x=>x.p),[{thread:'thread-a'},{thread:'thread-a',offset:2,sourceRevision:'pcb-r1',schematicRevision:'sch-r1'},{thread:'thread-a',offset:4,sourceRevision:'pcb-r1',schematicRevision:'sch-r1'}]);
  assert.equal(rpc.pending.size,0);
}
for(const [label,change]of [
  ['saved board revision',d=>{d.sourceRevision='pcb-r2';}],
  ['schematic revision',d=>{d.schematicRevision='sch-r2';}],
  ['canonical source',d=>{d.source='/other/board.kicad_pcb';}],
  ['fingerprint',d=>{d.fingerprint='changed';}],
  ['schema',d=>{d.schema=2;}],
  ['skipped cursor',d=>{d.page.offset++;}],
  ['changed total',d=>{d.page.total++;}],
  ['duplicate UUID',d=>{d.components[0].id=all[0].id;}],
  ['empty nonterminal page',d=>{d.components=[];}],
  ['lying final page',d=>{d.page.next=null;d.page.complete=true;}],
  ['looping cursor',d=>{d.page.next=d.page.offset;}],
]){
  const {rpc,sent}=endpoint((d,n)=>{if(n===2){d=structuredClone(d);change(d);}return d;});
  await assert.rejects(rpc.components('thread-a'),undefined,label);assert.equal(sent.length,2,label);assert.equal(rpc.pending.size,0);
}
{
  const {rpc,sent}=endpoint(d=>d,(r,n)=>{if(n===1)r.reset();});
  await assert.rejects(rpc.components('thread-a'),/Connection changed/);assert.equal(sent.length,1,'reset between pages cannot start another hub read');
}
{
  const owner=new AbortController();
  const {rpc,sent}=endpoint(d=>d,(r,n)=>{if(n===1)owner.abort();});
  await assert.rejects(rpc.components('disposed-viewer',{signal:owner.signal}),{name:'AbortError'});
  assert.equal(sent.length,1,'disposing a viewer between page replies cannot start another read');
  assert.equal(rpc.pending.size,0);
}
{
  const {rpc}=endpoint(d=>({...d,page:{...d.page,total:4097}}));await assert.rejects(rpc.components('thread-a'),/count exceeds/);
}
{
  const {rpc}=endpoint(d=>({...d,oversized:'x'.repeat(8*1024*1024)}));await assert.rejects(rpc.components('thread-a'),/8 MiB/);
}
{
  const {rpc}=endpoint(()=>({...page(),components:[],page:{offset:0,total:0,next:null,complete:true}}));assert.deepEqual((await rpc.components('empty-board')).components,[]);
}
for (const failingPage of [1, 2]) {
  const message = 'One component exceeds the bounded page budget; metadata cannot be truncated.';
  const {rpc,sent}=endpoint((data,n)=>n===failingPage?{error:message}:data);
  await assert.rejects(rpc.components('saved-board'),error=>error.message===message,'a page error retains the server cause');
  assert.equal(sent.length,failingPage);assert.equal(rpc.pending.size,0);
}
console.log('ok production component-page reader: complete-only authority, exact source/revision chain, cursor/total/UUID/schema guards, reset between pages, empty board and count/byte bounds');

const fragmentedRows=[{id:'before'},
  ...Object.entries({id:'large',pads:[1,2],symbol:{items:['saved symbol']}}).map(([key,value],index)=>({id:'large',fragment:{fields:{[key]:value},index,total:3,complete:index===2}})),{id:'after'}];
function fragments(change=()=>{}) {
  const sent=[];
  const rpc=new ViewerRpc(request=>{
    sent.push(request);queueMicrotask(()=>{
      const offset=request.p.offset||0,part=structuredClone(fragmentedRows.slice(offset,offset+2)),next=offset+part.length;
      const d={...page(),componentEncoding:'field-fragments-v1',componentCount:3,components:part,page:{offset,total:5,next:next===5?null:next,complete:next===5}};
      change(d,sent.length);
      rpc.receive({t:'reply',id:request.id,ok:true,result:{components:d}});
    });
  });
  return {rpc,sent};
}
{
 const {rpc}=fragments();const data=await rpc.components('saved-board');
 assert.deepEqual(data.components,[{id:'before'},{id:'large',pads:[1,2],symbol:{items:['saved symbol']}},{id:'after'}]);
 assert.equal(data.page.total,3,'logical count survives extra transport records');
}
for(const [label,change]of [
 ['identity',d=>{d.components[0].id='other';}],
 ['field index',d=>{d.components[0].fragment.index=0;}],
 ['field count',d=>{d.components[0].fragment.total=4;}],
 ['early completion',d=>{d.components[0].fragment.complete=true;}],
 ['missing final completion',d=>{d.components[1].fragment.complete=false;}],
 ['repeated field',d=>{d.components[0].fragment.fields={id:'large'};}],
 ['prototype field',d=>{d.components[0].fragment.fields=JSON.parse('{"__proto__":{}}');}],
 ['array fields',d=>{d.components[0].fragment.fields=[];}],
 ['interleaved component',d=>{d.components[0]={id:'other'};}],
 ['encoding changes',d=>{d.componentEncoding='unsupported';}],
 ['component count changes',d=>{d.componentCount=2;}],
]) {
 const {rpc,sent}=fragments((d,n)=>{if(n===2)change(d);});
 await assert.rejects(rpc.components('saved-board'),undefined,label);assert.equal(sent.length,2,label);
}
{
 const {rpc}=fragments(d=>{d.componentCount=4;});await assert.rejects(rpc.components('saved-board'),/incomplete/,'a forged logical count cannot publish');
}
{
 const {rpc}=fragments((d,n)=>{if(n===1){d.components[1].fragment.fields={reference:'U2'};}});
 await assert.rejects(rpc.components('saved-board'),/identity changed/,'fragmented row must retain its saved UUID field');
}
{
 let rpc;let sent;
 ({rpc,sent}=fragments((d,n)=>{if(n===1)queueMicrotask(()=>rpc.reset());}));
 await assert.rejects(rpc.components('saved-board'),/Connection changed/);
 assert.equal(sent.length,1,'a reset with an unfinished field aggregate starts no further read');
}
{
 const owner=new AbortController(),{rpc,sent}=fragments((d,n)=>{if(n===1)queueMicrotask(()=>owner.abort());});
 await assert.rejects(rpc.components('saved-board',{signal:owner.signal}),{name:'AbortError'});
 assert.equal(sent.length,1,'disposing an unfinished field aggregate starts no further read');
}
console.log('ok complete-only field fragments: exact reconstruction across pages, identity/order/count/field guards, duplicate and unsafe field rejection');
