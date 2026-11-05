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
