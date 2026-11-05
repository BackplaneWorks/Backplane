'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
async function load(){
 const wire=await import('../../src/web/wire.js');
 wire.learnDom({$:'El',tag:'',key:'',attrs:{$:'Con',head:{$:'At',name:'',value:''},tail:{$:'Con',head:{$:'On',event:'',action:''},tail:{$:'Nil'}}},kids:{$:'Con',head:{$:'Txt',text:''},tail:{$:'Nil'}}});
 const m=await import('../../src/web/press-feedback.js');return {...m,T:wire.T};
}
const L=xs=>xs.reduceRight((tail,head)=>({$:'Con',head,tail}),{$:'Nil'});
const kids=n=>{const o=[];for(let x=n.kids;x.$==='Con';x=x.tail)o.push(x.head);return o;};
const at=(n,name)=>{for(let x=n.attrs;x.$==='Con';x=x.tail)if(x.head.name===name)return x.head.value;};
function fixture(T){
 const el=(tag,key,attrs,ks=[])=>({$:T.El,tag,key,attrs:L(Object.entries(attrs).map(([name,value])=>({$:T.At,name,value}))),kids:L(ks)});
 const picker=el('div','acct',{class:'account-picker'},[el('button','a1',{class:'mi'})]);
 const viewer=k=>el('aside','viewer',{class:'viewer '+k},[el('div','',{class:'viewer-bar'})]);
 const page=({acct=false,view=null,n=0}={})=>el('div','',{},[el('div','app',{class:'app'+(view?' panel':'')},[
  el('aside','sidebar',{class:'sidebar'}),
  el('main','main',{class:'thread-view'},[el('div','tl',{id:'timeline'},[el('div','r'+n,{class:'msg'})]),el('div','composer',{class:'composer'},[el('textarea','composer',{}),...(acct?[picker]:[]),el('div','foot',{class:'composer-foot'})])]),
  ...(view?[viewer(view)]:[])])]);
 return {page,picker};
}
test('a press the hub answered with a region draws that region at the next same press, until it settles',async()=>{
 const {PressFeedback,regions,T}=await load();let changed=0;const f=new PressFeedback(()=>changed++),{page}=fixture(T);
 const p0=page();f.seen(p0,'thread:A');
 let settle=f.choice('acct-open','','thread:A');assert.equal(f.showing,false,'nothing known yet');settle(true);
 const p1=page({acct:true});f.seen(p1,'thread:A');
 settle=f.choice('acct-open','','thread:A');settle(true);f.seen(page({n:1}),'thread:A');   // shut again
 settle=f.choice('acct-open','','thread:A');assert.equal(f.showing,true);assert.equal(f.active.kind,'account');
 const shown=f.project(page({n:2}));const comp=kids(kids(kids(kids(shown)[0])[1])[1]).map(k=>k.key);
 assert.deepEqual(comp,['composer','acct','foot'],'in its own place among the composer’s pieces');
 assert.equal(regions(shown).get('account').node,regions(p1).get('account').node);
 settle(true);assert.equal(f.showing,false);const p3=page({n:3});assert.equal(f.project(p3),p3);
 // the shut press from the open shape is known too: the picker goes at once
 f.seen(page({acct:true,n:4}),'thread:A');settle=f.choice('acct-open','','thread:A');assert.equal(f.active?.node,null);
 const gone=f.project(page({acct:true,n:5}));assert.equal(regions(gone).has('account'),false);settle(true);
});
test('a viewer pane comes with its parent’s class; other destinations, navigation and ambiguous answers are never replayed',async()=>{
 const {PressFeedback,regions,T}=await load();const f=new PressFeedback(()=>{}),{page}=fixture(T);
 f.seen(page(),'thread:A');let s=f.choice('view-kind','board','thread:A');s(true);f.seen(page({view:'board'}),'thread:A');
 f.seen(page({n:1}),'thread:A');
 s=f.choice('view-kind','board','thread:A');assert.equal(f.active.kind,'viewer');
 const shown=f.project(page({n:2})),app=kids(shown)[0];assert.equal(at(app,'class'),'app panel');assert.equal(kids(app).at(-1).key,'viewer');
 s(true);
 assert.equal(f.choice('view-kind','board','thread:B'),undefined,'kept for another destination only');
 assert.equal(f.choice('select','t2','thread:A'),undefined,'navigation is the hub’s');
 f.seen(page({n:3}),'thread:A');s=f.choice('weird','','thread:A');s(true);f.seen(page({acct:true,view:'board',n:4}),'thread:A');
 f.seen(page({n:5}),'thread:A');f.choice('weird','','thread:A');assert.equal(f.showing,false,'two regions moved: not learned');
 f.drop();assert.equal(f.showing,false);
});
test('a first viewer open shows an empty pane where the hub puts it, only while none is open',async()=>{
 const {PressFeedback,regions,T}=await load();const f=new PressFeedback(()=>{}),{page}=fixture(T);
 f.seen(page(),'thread:A');let s=f.choice('view-kind','board','thread:A');
 assert.equal(f.active?.kind,'viewer','the very first: a pane made from the page’s pieces');
 let shown=f.project(page()),app=kids(shown)[0];assert.equal(at(app,'class'),'app panel');assert.equal(kids(app).at(-1).tag,'aside');
 s(true);f.seen(page({view:'board'}),'thread:A');f.seen(page({n:1}),'thread:B');
 s=f.choice('fm','','thread:B');assert.equal(f.active?.kind,'viewer');
 shown=f.project(page({n:2}));const pane=regions(shown).get('viewer').node;assert.equal(at(pane,'class'),'viewer pending');assert.equal(kids(pane).at(-1).key,'viewer-pending');
 s(true);f.seen(page({view:'files',n:3}),'thread:B');
 f.choice('diff','','thread:B');assert.equal(f.showing,true,'another tab: the pane empties at once');f.drop();f.choice('fm','','thread:B');assert.equal(f.showing,false,'the tab that opened it shuts it: the hub\u2019s');
});
test('with a pane open, another tab empties it at the press; the tab that opened it is the hub’s',async()=>{
 const {PressFeedback,regions,T}=await load();const f=new PressFeedback(()=>{}),{page}=fixture(T);
 f.seen(page(),'thread:A');let s=f.choice('view-kind','board','thread:A');s(true);f.seen(page({view:'board'}),'thread:A');
 f.choice('view-kind','board','thread:A');assert.equal(f.showing,false,'Board again shuts it: not guessed');f.drop();
 s=f.choice('fm','','thread:A');assert.equal(f.active?.kind,'viewer');
 const pane=regions(f.project(page({view:'board',n:1}))).get('viewer').node;assert.equal(at(pane,'class'),'viewer pending');
 s(true);
});
test('Settings takes the thread’s place at the press; its closing is the hub’s',async()=>{
 const {PressFeedback,regions,T}=await load();const f=new PressFeedback(()=>{}),{page}=fixture(T);
 const el=(tag,key,attrs,ks=[])=>({$:T.El,tag,key,attrs:L(Object.entries(attrs).map(([name,value])=>({$:T.At,name,value}))),kids:L(ks)});
 const settings=el('main','settings',{class:'settings-view'},[el('div','sc',{id:'settings-content'})]);
 const top=t=>el('header','',{class:'topbar'},[el('span','',{class:t})]);
 const withTop=(p,t)=>{const app=kids(p)[0];return {...p,kids:L([{...app,kids:L([top(t),...kids(app)])}])};};
 const withSettings=()=>{const p=page();const app=kids(p)[0];return withTop({...p,kids:L([{...app,kids:L([kids(app)[0],settings])}])},'settings');};
 f.seen(withTop(page(),'thread'),'thread:A');let s=f.choice('flag','settings','thread:A');s(true);f.seen(withSettings(),'thread:A');
 s=f.choice('flag','settings','thread:A');s(true);f.seen(withTop(page({n:1}),'thread'),'thread:A');
 assert.equal(f.learned.size,1,'the open is learned, the close is not');
 s=f.choice('flag','settings','thread:A');assert.equal(f.active?.kind,'settings');
 const app=kids(f.project(page({n:2})))[0];assert.deepEqual(kids(app).map(k=>k.key),['sidebar','settings'],'the thread’s main is put away');
 s(true);
});
test('a region goes back after the sibling it came after',async()=>{
 const {PressFeedback,regions,T}=await load();const f=new PressFeedback(()=>{}),{page}=fixture(T);
 const el=(tag,key,attrs,ks=[])=>({$:T.El,tag,key,attrs:L(Object.entries(attrs).map(([name,value])=>({$:T.At,name,value}))),kids:L(ks)});
 f.seen(page(),'thread:A');let s=f.choice('acct-open','','thread:A');s(true);f.seen(page({acct:true}),'thread:A');
 s=f.choice('acct-open','','thread:A');s(true);f.seen(page({n:1}),'thread:A');
 // the composer gained a piece in front since: the picker still follows the field
 const p=page({n:2}),app=kids(p)[0],main=kids(app)[1],tc=kids(main),comp=tc[1];
 const grown={...p,kids:L([{...app,kids:L([kids(app)[0],{...main,kids:L([tc[0],{...comp,kids:L([el('div','note',{class:'note'}),...kids(comp)])}])}])}])};
 s=f.choice('acct-open','','thread:A');
 const out=kids(kids(kids(kids(f.project(grown))[0])[1])[1]).map(k=>k.key);assert.deepEqual(out,['note','composer','acct','foot']);s(true);
});
test('Settings shut at the press puts back the thread from the picture the host has of it',async()=>{
 const {PressFeedback,T}=await load();const f=new PressFeedback(()=>{}),{page}=fixture(T);
 const el=(tag,key,attrs,ks=[])=>({$:T.El,tag,key,attrs:L(Object.entries(attrs).map(([name,value])=>({$:T.At,name,value}))),kids:L(ks)});
 const top=t=>el('header','',{class:'topbar'},[el('span','',{class:t})]);
 const settings=el('main','settings',{class:'settings-view'},[]);
 const open=(()=>{const p=page();const app=kids(p)[0];return {...p,kids:L([{...app,kids:L([top('settings'),kids(app)[0],settings])}])};})();
 const picture=(()=>{const p=page({n:7});const app=kids(p)[0];return {...p,kids:L([{...app,kids:L([top('thread'),...kids(app)])}])};})();
 f.seen(open,'thread:A');
 assert.equal(f.choice('flag','settings','thread:A',null)&&f.showing,false,'no picture: the hub’s');
 const s=f.choice('flag','settings','thread:A',picture);assert.equal(f.showing,true);
 const app=kids(f.project(open))[0];
 assert.deepEqual(kids(app).map(k=>k.tag),['header','aside','main']);assert.equal(at(kids(app)[2],'class'),'thread-view');
 assert.equal(at(kids(kids(app)[0])[0],'class'),'thread','the thread’s top bar');s(true);assert.equal(f.showing,false);
});
test('what presses opened is kept across a reload for the same hub and pairing, fields blank, others dropped',async()=>{
 const {PressFeedback,T}=await load();
 const mem=new Map(),storage={getItem:k=>mem.has(k)?mem.get(k):null,setItem:(k,v)=>mem.set(k,v),removeItem:k=>mem.delete(k),key:i=>[...mem.keys()][i],get length(){return mem.size;}};
 const el=(tag,key,attrs,ks=[])=>({$:T.El,tag,key,attrs:L(Object.entries(attrs).map(([name,value])=>({$:T.At,name,value}))),kids:L(ks)});
 const {page}=fixture(T);
 const settings=el('main','settings',{class:'settings-view'},[el('input','q',{id:'q',value:'secret path'})]);
 const withSettings=()=>{const p=page();const app=kids(p)[0];return {...p,kids:L([{...app,kids:L([kids(app)[0],settings])}])};};
 let scope='hubA';mem.set('bp-press-v1:old','[]');
 const f=new PressFeedback(()=>{},64,{storage,scope:()=>scope});
 f.seen(page(),'thread:A');const s=f.choice('flag','settings','thread:A');s(true);f.seen(withSettings(),'thread:A');
 await new Promise(r=>setTimeout(r,2100));
 assert.ok(mem.has('bp-press-v1:hubA'));assert.ok(!mem.has('bp-press-v1:old'),'another pairing’s entries go');
 assert.ok(!mem.get('bp-press-v1:hubA').includes('secret path'),'a field’s value is never stored');
 const g=new PressFeedback(()=>{},64,{storage,scope:()=>scope});g.seen(page({n:1}),'thread:A');
 g.choice('flag','settings','thread:A');assert.equal(g.active?.kind,'settings','after a reload: drawn at the first press');
 scope='hubB';const h=new PressFeedback(()=>{},64,{storage,scope:()=>scope});h.seen(page({n:2}),'thread:A');h.choice('flag','settings','thread:A');assert.equal(h.showing,false,'another hub: nothing');
 mem.set('bp-press-v1:hubC','{not json');scope='hubC';const k=new PressFeedback(()=>{},64,{storage,scope:()=>scope});k.seen(page({n:3}),'thread:A');assert.equal(k.learned.size,0,'a broken store is ignored');
});
test('a popover drawn with its shade before it is put in and taken away with the shade, in the hub’s order',async()=>{
 const {PressFeedback,regions,T}=await load();const f=new PressFeedback(()=>{});
 const el=(tag,key,attrs,ks=[])=>({$:T.El,tag,key,attrs:L(Object.entries(attrs).map(([name,value])=>({$:T.At,name,value}))),kids:L(ks)});
 const pop=el('div','',{class:'tok-pop'},[el('span','',{class:'tok-title'})]),shade=el('div','',{class:'shade','data-value':'tokstats'});
 const page=(open,n=0)=>el('div','',{},[el('div','app',{class:'app'},[el('main','main',{class:'thread-view'},[el('div','tl',{id:'timeline'},[el('div','r'+n,{class:'msg'})]),
  el('div','composer',{class:'composer'},[el('textarea','composer',{}),el('button','tok',{class:'tok-chip'}),...(open?[shade,pop]:[]),el('div','foot',{class:'composer-foot'})])])])]);
 const comp=pg=>kids(kids(kids(kids(pg)[0])[0])[1]);
 f.seen(page(false),'thread:A');let s=f.choice('flag','tokstats','thread:A');s(true);f.seen(page(true),'thread:A');
 f.seen(page(false,1),'thread:A');s=f.choice('flag','tokstats','thread:A');assert.equal(f.active?.kind,'tokens');
 assert.deepEqual(comp(f.project(page(false,2))).map(k=>at(k,'class')),[undefined,'tok-chip','shade','tok-pop','composer-foot'],'the shade, then the popover, where the hub draws them');
 s(true);f.seen(page(true,3),'thread:A');s=f.choice('flag','tokstats','thread:A');s(true);f.seen(page(false,5),'thread:A');
 f.seen(page(true,6),'thread:A');s=f.choice('flag','tokstats','thread:A');assert.equal(f.active?.node,null,'the shut press is known too');
 assert.deepEqual(comp(f.project(page(true,4))).map(k=>at(k,'class')),[undefined,'tok-chip','composer-foot'],'the shade goes with it (it would catch every click)');
 assert.equal(regions(page(true)).get('tokens').pre[0],shade);
});
test('a viewer press draws the rail with the pressed tab on show and Close the viewer at its foot, as the hub draws them',async()=>{
 const {PressFeedback,T}=await load();const f=new PressFeedback(()=>{});
 const el=(tag,key,attrs,ks=[],ons=[])=>({$:T.El,tag,key,attrs:L([...Object.entries(attrs).map(([name,value])=>({$:T.At,name,value})),...ons.map(([event,action])=>({$:'On',event,action}))]),kids:L(ks)});
 const tab=(k,cur,act,val)=>el('button',k,{type:'button',class:cur===k?'tab on':'tab','aria-current':cur===k?'page':'false','aria-pressed':cur===k?'true':'false','data-value':cur===k?'':val},[],[['click',cur===k?'panel':act]]);
 const closer=el('button','',{class:'viewer-close'});
 const rail=cur=>el('nav','workspace',{class:'workspace-bar'},[el('div','',{class:'workspace-rail'},[el('div','',{class:'workspace-group'},[tab('board',cur,'view-kind','board'),tab('files',cur,'fm','')])]),
  el('div','',{class:'workspace-close'},cur?[closer]:[]),el('button','rail-toggle',{class:'rail-toggle'})]);
 const page=(cur,n=0)=>el('div','',{},[el('div','app',{class:'app'+(cur?' panel':'')},[el('main','main',{class:'thread-view'},[el('div','tl',{id:'timeline'},[el('div','r'+n,{class:'msg'})])]),
  el('div','ws',{class:'workspace-thread'},[rail(cur)]),...(cur?[el('aside','viewer',{class:'viewer '+cur},[el('div','',{class:'viewer-bar'})])]:[])])]);
 const railOf=pg=>kids(kids(kids(pg)[0]).find(k=>k.key==='ws'))[0];
 const on=r=>kids(kids(kids(r)[0])[0]).filter(b=>at(b,'class')==='tab on').map(b=>b.key), shut=r=>kids(kids(r)[1]).length;
 // nothing learned: the first open draws no rail change without its Close button
 f.seen(page(''),'thread:A');let s=f.choice('view-kind','board','thread:A');assert.equal(f.active.kind,'viewer');
 assert.deepEqual(on(railOf(f.project(page('',1)))),[],'Close never seen: the rail is the hub’s');s(true);
 f.seen(page('board',2),'thread:A');f.seen(page('',3),'thread:A');
 s=f.choice('view-kind','board','thread:A');let r=railOf(f.project(page('',4)));
 assert.deepEqual(on(r),['board']);assert.equal(shut(r),1,'Close the viewer is there at the press');
 s(true);f.seen(page('board',5),'thread:A');
 // a tab never seen on show: drawn on as the hub draws one, its click shutting the viewer
 s=f.choice('fm','','thread:A');r=railOf(f.project(page('board',6)));const files=kids(kids(kids(r)[0])[0])[1];
 assert.deepEqual(on(r),['files']);assert.equal(at(files,'data-value'),'');
 assert.deepEqual(JSON.stringify(files),JSON.stringify(tab('files','files','fm','')),'the same node the hub draws for it');
 s(true);f.seen(page('files',7),'thread:A');
 // the tab on show pressed again shuts the viewer: no tab on, no Close
 s=f.choice('panel','','thread:A');s(true);f.seen(page('',8),'thread:A');f.seen(page('files',9),'thread:A');
 s=f.choice('panel','','thread:A');assert.equal(f.active?.node,null);r=railOf(f.project(page('files',10)));assert.deepEqual(on(r),[]);assert.equal(shut(r),0);
});
test('a shelf press draws the one sidebar section it changed as the hub drew it, only from the same shape before',async()=>{
 const {PressFeedback,T}=await load();const f=new PressFeedback(()=>{});
 const el=(tag,key,attrs,ks=[])=>({$:T.El,tag,key,attrs:L(Object.entries(attrs).map(([name,value])=>({$:T.At,name,value}))),kids:L(ks)});
 const row=id=>el('bp-memo',id,{fn:'row'});
 const proj=(id,open,extra=[])=>el('section',id,{class:'project'},[el('div','',{class:'project-threads'},[row(id+'a'),el('button','',{class:open?'shelf open':'shelf','data-value':'settled:'+id}),...(open?[row(id+'s1'),row(id+'s2')]:[]),...extra])]);
 const page=(openP,n=0,extra=[])=>el('div','',{},[el('div','app',{class:'app'},[el('aside','sidebar',{class:'sidebar'},[el('div','',{class:'list'},[proj('p1',openP==='p1',extra),proj('p2',openP==='p2')])]),
  el('main','main',{class:'thread-view'},[el('div','tl',{id:'timeline'},[el('div','r'+n,{class:'msg'})])])])]);
 const secs=pg=>kids(kids(kids(kids(pg)[0])[0])[0]);
 f.seen(page(''),'thread:A');let s=f.choice('toggle-settled','settled:p1','thread:A');assert.equal(f.showing,false,'nothing known yet');s(true);
 const opened=page('p1',1);f.seen(opened,'thread:A');
 s=f.choice('toggle-settled','settled:p1','thread:A');assert.equal(f.showing,false,'the open shelf’s press (shut) is not known yet');s(true);f.seen(page('',2),'thread:A');
 s=f.choice('toggle-settled','settled:p1','thread:A');assert.equal(f.active?.kind,'section');
 const shown=f.project(page('',3));assert.equal(secs(shown)[0],secs(opened)[0],'the section as the hub drew it open');assert.equal(secs(shown)[1].key,'p2','the others as they are');
 s(true);f.seen(page('p1',4),'thread:A');
 s=f.choice('toggle-settled','settled:p1','thread:A');assert.equal(f.active?.kind,'section','the shut press, learned from the open shape');
 assert.deepEqual(kids(kids(secs(f.project(page('p1',5)))[0])[0]).map(k=>k.key),['p1a','']);s(true);f.seen(page('',6),'thread:A');
 // the section changed shape since (a new row): not drawn from what was learned
 f.seen(page('',7,[row('new')]),'thread:A');s=f.choice('toggle-settled','settled:p1','thread:A');assert.equal(f.showing,false);s(true);
 assert.equal(f.choice('toggle-settled','settled:p1','thread:B'),undefined,'another destination: nothing seen there');
});
test('a learned shelf is kept across a reload with the rest',async()=>{
 const {PressFeedback,T}=await load();
 const mem=new Map(),storage={getItem:k=>mem.has(k)?mem.get(k):null,setItem:(k,v)=>mem.set(k,v),removeItem:k=>mem.delete(k),key:i=>[...mem.keys()][i],get length(){return mem.size;}};
 const el=(tag,key,attrs,ks=[])=>({$:T.El,tag,key,attrs:L(Object.entries(attrs).map(([name,value])=>({$:T.At,name,value}))),kids:L(ks)});
 const proj=open=>el('section','p1',{class:'project'},[el('button','',{class:open?'shelf open':'shelf'}),...(open?[el('bp-memo','s1',{fn:'row'})]:[])]);
 const page=(open,n=0)=>el('div','',{},[el('div','app',{class:'app'},[el('aside','sidebar',{class:'sidebar'},[proj(open)]),el('main','main',{class:'thread-view'},[el('div','tl',{id:'timeline'},[el('div','r'+n,{})])])])]);
 const f=new PressFeedback(()=>{},64,{storage,scope:()=>'hubA'});
 f.seen(page(false),'thread:A');f.choice('toggle-settled','settled:p1','thread:A')(true);f.seen(page(true,1),'thread:A');
 await new Promise(r=>setTimeout(r,2100));
 const g=new PressFeedback(()=>{},64,{storage,scope:()=>'hubA'});g.seen(page(false,2),'thread:A');
 g.choice('toggle-settled','settled:p1','thread:A');assert.equal(g.active?.kind,'section','after a reload: drawn at the first press');
});
