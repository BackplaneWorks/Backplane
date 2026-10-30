// Run with the actual emitted pure module, not a JavaScript pager clone:
// bend src/core/components.bend -o /tmp/components-paged.mjs
// bun test/tools/components_pages_test.mjs /tmp/components-paged.mjs [BOARD SCH]
import assert from 'node:assert/strict';
import {readFileSync,statSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const {default:C}=await import(pathToFileURL(resolve(process.argv[2]||'/tmp/components-paged.mjs')));
const bytes=x=>Buffer.byteLength(JSON.stringify(x));
const base={schema:1,source:'/saved/board.kicad_pcb',schematic:'/saved/board.kicad_sch',sourceRevision:'pcb-save',schematicRevision:'sch-save',fingerprint:123,thicknessMm:1.6,components:[]};
const row=i=>({id:`uuid-${i}`,reference:`R${i+1}`,symbol:{items:['µλ漢🙂'.repeat(1000)]},pads:[],graphics:[]});
const call=(data,offset=0,pcb='',sch='')=>JSON.parse(C.page(JSON.stringify(data),BigInt(offset),pcb,sch));
function aggregate(data) {
  let offset=0,pcb='',sch='',rows=[],pages=[];
  do {
    const page=call(data,offset,pcb,sch);assert(!page.error,page.error);
    assert.equal(page.page.offset,offset);assert.equal(page.page.total,data.components.length);
    assert.equal(page.source,data.source);assert.equal(page.sourceRevision,data.sourceRevision);assert.equal(page.schematicRevision,data.schematicRevision);assert.equal(page.fingerprint,data.fingerprint);
    assert(page.components.length<=32);assert(bytes({components:page})+1<=220000,'wrapper and newline fit unchanged response cap');
    assert(bytes(page.components)<=180002,'UTF-8 row budget');
    rows.push(...page.components);pages.push({offset,rows:page.components.length,bytes:bytes({components:page})+1,next:page.page.next,complete:page.page.complete});
    pcb=page.sourceRevision;sch=page.schematicRevision;
    if(page.page.complete){assert.equal(page.page.next,null);assert.equal(rows.length,data.components.length);break;}
    assert.equal(page.page.next,offset+page.components.length);assert(page.page.next>offset);offset=page.page.next;assert(pages.length<=256);
  }while(true);
  assert.deepEqual(rows,data.components,'all source fields, graphics, pads and symbols survive paging exactly');
  return pages;
}
const data={...base,components:Array.from({length:96},(_,i)=>row(i))},pages=aggregate(data);assert(pages.length>3,'Unicode bytes force bounded pages');
assert.deepEqual(call(base).page,{offset:0,next:null,total:0,complete:true});
assert(call(data,1).error,'continuation without revisions');
assert(call(data,1,'wrong','sch-save').error);assert(call(data,1,'pcb-save','wrong').error);
assert(call(data,97,'pcb-save','sch-save').error);assert(call({...data,sourceRevision:''}).error);
assert(call({...data,components:'invalid'}).error);assert(call({...data,schema:2}).error);
assert.equal(call({error:'Saved CAD exceeds the 16 MiB input limit.'}).error,'Saved CAD exceeds the 16 MiB input limit.');
assert(call({...base,components:[{id:'oversized',symbol:'漢'.repeat(70000)}]}).error,'oversized individual rows are never truncated');
assert(call({...base,components:Array.from({length:4097},(_,i)=>({id:`x${i}`}))}).error,'component count is bounded');
aggregate({...base,schematic:'',schematicRevision:'',components:data.components});
const stamp=path=>{const s=statSync(path,{bigint:true});return `${s.dev}:${s.ino}:${s.size}:${s.mtimeNs}:${s.ctimeNs}`;};
if(process.env.COMPONENT_SERVER_MODULE) {
  const {default:S}=await import(pathToFileURL(resolve(process.env.COMPONENT_SERVER_MODULE)));
  const prefix=S['Components.error']('probe').$.replace(/Obj$/,'');
  for(const text of ['a'.repeat(219974),'a'.repeat(219975),'漢'.repeat(70000),'漢'.repeat(74000)]) {
    const obj={$:prefix+'Obj',fields:{$:prefix+'Field',key:'test',value:{$:prefix+'Str',text},tail:{$:prefix+'End'}}};
    const response=S['Components.response'](obj),expected=JSON.stringify({components:{test:text}});
    assert(Buffer.byteLength(response)+1<=220000);
    if(Buffer.byteLength(expected)<=219999)assert.equal(response,expected);else assert(JSON.parse(response).error);
  }
}
if(process.argv[3]) {
  const pcb=resolve(process.argv[3]),sch=process.argv[4]?resolve(process.argv[4]):'',before={pcb:stamp(pcb),sch:sch?stamp(sch):''},start=performance.now();
  const d=JSON.parse(C.inspect(readFileSync(pcb,'utf8'),sch?readFileSync(sch,'utf8'):'',pcb,sch));d.sourceRevision=before.pcb;d.schematicRevision=before.sch;
  const actualPages=aggregate(d),elapsedMs=performance.now()-start;
  assert.equal(stamp(pcb),before.pcb);if(sch)assert.equal(stamp(sch),before.sch);
  const receipt={source:pcb,schematic:sch,revisions:before,components:d.components.length,fullResponseBytes:bytes({components:d})+1,pages:actualPages,elapsedMs,limits:{responseBytes:220000,rowBytes:180000,rowsPerPage:32,components:4096},scope:'Actual emitted pure parser/pager, read-only saved CAD. Server IO/CLI/HTTP aggregation is an integration gate.'};
  if(process.env.COMPONENT_PAGES_RECEIPT)writeFileSync(process.env.COMPONENT_PAGES_RECEIPT,JSON.stringify(receipt,null,2));
  console.log(JSON.stringify(receipt));
}
console.log('ok actual Bend component pages: UTF-8 bounds, exact complete reconstruction, both revision stamps, schema/offset/count guards, empty board, missing schematic and indivisible oversized-row error');
