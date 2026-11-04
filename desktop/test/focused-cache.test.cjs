'use strict';
const { test } = require('node:test'), assert = require('node:assert/strict');
const scope = { origin:'https://fixture', cid:'device', identity:'a'.repeat(64), bundle:'/host.hash.js', language:'ja', selected:'A' };
const text = value => ({ nodeType:3, nodeValue:value });
function el(tag, attrs={}, kids=[]) {
 return {nodeType:1,tagName:tag.toUpperCase(),childNodes:kids,hidden:false,
  getAttribute:name=>attrs[name]??null,hasAttribute:name=>Object.hasOwn(attrs,name),
  classList:{contains:name=>(attrs.class||'').split(' ').includes(name)},getBoundingClientRect:()=>({top:0,bottom:600})};
}
async function picture(body='CURRENT 日本語 👩‍💻 é') {
 const m=await import('../../src/web/focused-cache.js');
 const row=el('div',{class:'msg assistant','data-id':'actual'},[
  el('div',{class:'body'},[text(body)]),el('button',{'data-action':'send'},[text('ACTION')]),
  el('input',{value:'DRAFT'}),el('img',{'data-owner':'OWNER',src:'SECRET'}),el('iframe',{src:'GUEST'}),
  el('div',{class:'sending'},[text('PENDING')]),el('div',{class:'live'},[text('LIVE')]),
  el('a',{href:'secret-file','data-action':'open'},[text('link text')])]);
 const timeline=el('div',{},[row]);Object.assign(timeline,{children:[row],clientHeight:600,scrollHeight:600,scrollTop:0});
 const record=m.captureFocused({main:el('main'),timeline,title:el('span',{class:'title','data-action':'rename'},[text('Actual title')]),scope,epoch:'actual-epoch',shellAttrs:[{name:'class',value:'app picked'},{name:'style',value:'--text-scale:125'},{name:'data-owner',value:'BAD'}],now:100});
 return{m,record,timeline};
}
test('capture keeps actually rendered committed Unicode but strips actions, draft, pending and guest refs',async()=>{
 const {m,record}=await picture();assert(m.validFocused(record,scope));const encoded=JSON.stringify(record);
 for(const value of ['CURRENT 日本語 👩‍💻 é','Actual title','link text']) assert(encoded.includes(value));
 for(const value of ['ACTION','DRAFT','PENDING','LIVE','GUEST','OWNER','SECRET','secret-file','data-action','href','src','cmds','session','since'])assert(!encoded.includes(value),value);
});
test('a multi-megabyte committed message yields a bounded, marked tail rather than an empty cache',async()=>{
 const {m,record}=await picture('OLD'.repeat(800000)+' 日本語 TAIL');assert(record);assert(record.clipped);assert(m.validFocused(record,scope));
 assert(Buffer.byteLength(JSON.stringify(record))<=m.VIEW_LIMIT);assert(JSON.stringify(record).includes('日本語 TAIL'));assert(!JSON.stringify(record).includes('�'));
});
test('origin, profile, credential rotation, bundle, language and selection are all distinct partitions',async()=>{
 const {m,record}=await picture();for(const key of Object.keys(scope))assert(!m.validFocused(record,{...scope,[key]:key==='identity'?'b'.repeat(64):'other'}),key);
 assert(!m.validFocused(record,{...scope,identity:''}));
});
test('cache payload validation refuses executable nodes, actions and malformed shells',async()=>{
 const {m,record}=await picture();for(const change of [r=>r.main.attrs.push({event:'click',action:'send'}),r=>r.main.attrs.push({name:'data-owner',value:'x'}),r=>r.main.tag='iframe',r=>r.shellAttrs.push({name:'style',value:'background:url(secret)'}),r=>r.main.kids.push({type:'element',tag:'script',key:'',attrs:[],kids:[]})]){
  const r=structuredClone(record);change(r);assert(!m.validFocused(r,scope));
 }
});
test('cookie marker accepts one exact public hex tag and no duplicates or credential alias',async()=>{
 const {m}=await picture();assert.equal(m.cacheIdentity(`bp_token=secret; bp_cache_v1=${scope.identity}`),scope.identity);
 for(const value of ['bp_token='+scope.identity,'bp_cache_v1=bad',`bp_cache_v1=${scope.identity}; bp_cache_v1=${scope.identity}`])assert.equal(m.cacheIdentity(value),'');
});
test('merge replaces only the displayed main and leaves the fresh shell state untouched',async()=>{
 const {m,record}=await picture();const shell={type:'element',tag:'div',key:'app',attrs:[{name:'class',value:'app'}],kids:[{type:'element',tag:'main',key:'',attrs:[],kids:[]}]};
 const fresh=JSON.stringify(shell),merged=m.mergeFocused(shell,record);assert.equal(JSON.stringify(shell),fresh);assert.deepEqual(merged.kids[0].kids,record.main.kids);assert.equal(merged.kids[0].attrs.at(-1).value,'true');assert.equal(merged.attrs[0].value,'app picked');
 assert.equal(m.mergeFocused(shell,{...record,shellAttrs:[]}).attrs[0].value,'app');
});
test('bounded capture excludes noncommitted rows even when they are the most recent',async()=>{
 const {m,record,timeline}=await picture();const pending=el('div',{class:'msg user sending','data-id':'pending'},[text('NEVER SAVED')]);timeline.children.push(pending);
 const next=m.captureFocused({main:el('main'),timeline,title:el('span',{},[text('title')]),scope,epoch:'epoch'});assert.equal(next.rows,1);assert(!JSON.stringify(next).includes('NEVER SAVED'));
});
test('missing identity never opens the persisted-view database',async()=>{
 const {m}=await picture();const cache=new m.FocusedCache({indexedDB:{open(){throw Error('must not open');}}});assert.equal(await cache.get({...scope,identity:''}),null);
});
test('a delayed cache lookup cannot paint under a newer selection or credential',async()=>{
 const {m,record}=await picture();for(const change of [{selected:'B'},{identity:'b'.repeat(64)}]){
  let current=scope,release,shown=0;const view=new m.FocusedView({scope:()=>current,cache:{get:()=>new Promise(r=>release=r)},hide(){},view(){shown++;}});
  const p=view.restore();current={...scope,...change};release(record);assert.equal(await p,false);assert.equal(shown,0);
 }
});
test('fresh remote content overtakes an outstanding cache lookup without cached cursor or draft adoption',async()=>{
 const {m,record}=await picture();let release,shown=0;const api={scope:()=>scope,cache:{get:()=>new Promise(r=>release=r)},hide(){},view(){shown++;}};
 const view=new m.FocusedView(api),p=view.restore();view.remote('actual-new-epoch');release(record);assert.equal(await p,false);assert.equal(shown,0);assert.equal(view.record,null);
});
test('an epoch mismatch hides and removes the old persisted picture',async()=>{
 const {m,record}=await picture();let removed,hidden=0,shown=0;const view=new m.FocusedView({scope:()=>scope,cache:{get:async()=>record,remove:async r=>{removed=r;}},hide(){hidden++;},view(){shown++;}});
 assert.equal(await view.restore(),true);assert.equal(shown,1);view.remote('changed-epoch');assert.equal(removed,record);assert.equal(view.record,null);assert.equal(hidden,2);
});
test('cache restore never writes draft storage and a new local draft remains independent',async()=>{
 const {m,record}=await picture();const draft={text:'new unsent 日本語'};const view=new m.FocusedView({scope:()=>scope,cache:{get:async()=>record},hide(){},view(){}});
 await view.restore();assert.equal(draft.text,'new unsent 日本語');assert.equal(Object.hasOwn(record,'drafts'),false);assert.equal(Object.hasOwn(record,'session'),false);
});
// The web's own live row is a static pending bubble (View.pending.node, the
// product's static pending contract: docs/active-threads-load.md). This shim is
// the other stream row capture accepts: msg assistant live, body child, and NO
// data-id. A memo key is not a DOM data-id attribute.
function normalLive(body) { return el('div',{class:'msg assistant live'},[el('div',{class:'body'},[text(body)])]); }
test('normal live DOM shape matches the renderer rather than assuming a committed-message data-id',()=>{
 const fs=require('node:fs'),path=require('node:path');const source=fs.readFileSync(path.resolve(__dirname,'../../src/web/view.bend'),'utf8');
 const live=source.slice(source.indexOf('def View.live.n.loc('),source.indexOf('def View.live.n(',source.indexOf('def View.live.n.loc(')));
 assert(live.includes('View.pending.node(lc, key)'));
 const pending=source.slice(source.indexOf('def View.pending.node('),source.indexOf('def View.pending.rows(',source.indexOf('def View.pending.node(')));
 assert(pending.includes('D.At{"class", "msg assistant pending"}'));assert(pending.includes('D.At{"class", "body"}'));assert(!pending.includes('data-id'));
 assert.equal(normalLive('x').hasAttribute('data-id'),false);
});
test('the static pending bubble the renderer draws stays out of a saved picture',async()=>{
 const {m,timeline}=await picture();timeline.children.push(el('div',{class:'msg assistant pending',role:'status'},[el('div',{class:'body'},[text('STATIC PENDING')])]));
 const record=m.captureFocused({main:el('main'),timeline,title:el('span',{},[text('title')]),scope,epoch:'actual'});
 assert.equal(record.rows,1);assert(!JSON.stringify(record).includes('STATIC PENDING'));
});
test('a stream-only normal assistant row persists current partial Unicode as a readonly picture',async()=>{
 const {m,timeline}=await picture();timeline.children=[normalLive('途中の返答 👩‍💻 é CURRENT')];
 const record=m.captureFocused({main:el('main'),timeline,title:el('span',{},[text('Streaming thread')]),scope,epoch:'actual'});
 assert(record,'stream-only conversation must not be an empty cache');assert(m.validFocused(record,scope));assert.equal(record.rows,1);assert.equal(record.partial,true);
 const encoded=JSON.stringify(record);assert(encoded.includes('途中の返答 👩‍💻 é CURRENT'));assert(!encoded.includes('assistant live'));assert(!encoded.includes('data-id'));assert(!encoded.includes('session'));assert(!encoded.includes('cmds'));
});
test('committed plus partial assistant text survives while pending send, asks, draft and controls are stripped',async()=>{
 const {m,timeline}=await picture();const live=normalLive('CURRENT PARTIAL 日本語');
 live.childNodes.push(el('button',{'data-action':'retry'},[text('ACTION')]),el('input',{value:'DRAFT'}),el('div',{class:'asks'},[text('APPROVAL')]),el('div',{class:'sending'},[text('PENDING')]));
 timeline.children.push(live,el('div',{class:'msg user sending','data-id':'pending'},[text('UNSENT')]),el('div',{class:'msg user live'},[text('INVALID LIVE USER')]));
 const record=m.captureFocused({main:el('main'),timeline,title:el('span',{},[text('title')]),scope,epoch:'actual'});
 assert.equal(record.rows,2);assert.equal(record.partial,true);const encoded=JSON.stringify(record);
 assert(encoded.includes('CURRENT PARTIAL 日本語'));assert(encoded.includes('CURRENT 日本語 👩‍💻 é'));
 for(const forbidden of ['ACTION','DRAFT','APPROVAL','PENDING','UNSENT','INVALID LIVE USER','data-action'])assert(!encoded.includes(forbidden),forbidden);
});
test('a multi-megabyte partial stream preserves the current UTF8 tail within the same hard cap',async()=>{
 const {m,timeline}=await picture();timeline.children=[normalLive('OLD'.repeat(800000)+' 進行中 👩‍💻 é TAIL')];
 const record=m.captureFocused({main:el('main'),timeline,title:el('span',{},[text('title')]),scope,epoch:'actual'});
 assert(record);assert(record.clipped);assert(record.partial);assert(m.validFocused(record,scope));assert(Buffer.byteLength(JSON.stringify(record))<=m.VIEW_LIMIT);
 assert(JSON.stringify(record).includes('進行中 👩‍💻 é TAIL'));assert(!JSON.stringify(record).includes('�'));
});
test('saved status dots are inactive and phase Working labels cannot claim present activity',async()=>{
 const {m,timeline}=await picture();const row=timeline.children[0];row.childNodes.push(el('span',{class:'indicator-dot active',title:'Working'},[text('STALE WORKING')]),el('div',{class:'phase active'},[text('STALE PHASE')]),text('User literally wrote Working'));
 const record=m.captureFocused({main:el('main'),timeline,title:el('span',{},[text('title')]),scope,epoch:'actual'}),encoded=JSON.stringify(record);
 assert(m.validFocused(record,scope));assert(encoded.includes('indicator-dot inactive'));assert(!encoded.includes('STALE WORKING'));assert(!encoded.includes('STALE PHASE'));assert(encoded.includes('User literally wrote Working'));
 const bad=structuredClone(record);bad.main.attrs.push({name:'class',value:'dot active'});assert(!m.validFocused(bad,scope));
});
test('same-selection cache refresh keeps an already visible readonly picture through a cache miss',async()=>{
 const {m,record}=await picture();let next=record,hidden=0;const view=new m.FocusedView({scope:()=>scope,cache:{get:async()=>next},hide(){hidden++;},view(){}});
 await view.restore();next=null;const before=hidden;assert.equal(await view.restore(),true);assert.equal(hidden,before);assert.equal(view.record,record);
});
test('a late credential change clears a retained picture instead of painting into the new partition',async()=>{
 const {m,record}=await picture();let current=scope,next=Promise.resolve(record),release,hidden=0;const view=new m.FocusedView({scope:()=>current,cache:{get:()=>next},hide(){hidden++;},view(){}});
 await view.restore();next=new Promise(r=>release=r);const pending=view.restore();current={...scope,identity:'b'.repeat(64)};release(record);assert.equal(await pending,false);assert.equal(view.record,null);assert.equal(hidden,2);
});
test('bounded live projection: selected normal assistant synthetic id is partial readonly text, not a committed id',async()=>{
 const {m,timeline}=await picture();timeline.children=[el('div',{class:'msg assistant','data-id':'@live:A'},[el('div',{class:'body'},[text('CURRENT bounded 日本語 👩‍💻 é')]),el('button',{'data-action':'copy'},[text('ACTION')]),el('input',{value:'DRAFT'})])];
 const record=m.captureFocused({main:el('main'),timeline,title:el('span',{},[text('title')]),scope,epoch:'actual'}),encoded=JSON.stringify(record);assert.equal(record.partial,true);assert.equal(record.rows,1);assert(m.validFocused(record,scope));assert(encoded.includes('CURRENT bounded 日本語 👩‍💻 é'));for(const s of ['@live:A','data-id','ACTION','DRAFT','cmds','session'])assert(!encoded.includes(s),s);
});
test('bounded live projection: foreign synthetic owner and synthetic user row cannot enter selected cache',async()=>{
 const {m,timeline}=await picture();timeline.children.push(el('div',{class:'msg assistant','data-id':'@live:B'},[text('FOREIGN STREAM')]),el('div',{class:'msg user','data-id':'@live:A'},[text('USER SYNTHETIC')]));
 const record=m.captureFocused({main:el('main'),timeline,title:el('span',{},[text('title')]),scope,epoch:'actual'}),encoded=JSON.stringify(record);assert.equal(record.partial,false);assert.equal(record.rows,1);assert(!encoded.includes('FOREIGN STREAM'));assert(!encoded.includes('USER SYNTHETIC'));
});
test('bounded live projection: selected synthetic stream-only DOM retains bounded UTF8 current tail',async()=>{
 const {m,timeline}=await picture();timeline.children=[el('div',{class:'msg assistant','data-id':'@live:A'},[el('div',{class:'body'},[text('OLD'.repeat(800000)+' current 日本語 👩‍💻 é')])])];
 const record=m.captureFocused({main:el('main'),timeline,title:el('span',{},[text('title')]),scope,epoch:'actual'}),encoded=JSON.stringify(record);assert.equal(record.partial,true);assert(record.clipped);assert(m.validFocused(record,scope));assert(Buffer.byteLength(encoded)<=m.VIEW_LIMIT);assert(encoded.includes('current 日本語 👩‍💻 é'));assert(!encoded.includes('�'));
});
