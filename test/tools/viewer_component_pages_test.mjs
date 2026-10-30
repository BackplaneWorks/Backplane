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
console.log('ok production component-page reader: complete-only authority, exact source/revision chain, cursor/total/UUID/schema guards, reset between pages, empty board and count/byte bounds');
