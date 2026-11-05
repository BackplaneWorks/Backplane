'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
async function load(){
 const wire=await import('../../src/web/wire.js');
 wire.learnDom({$:'El',tag:'',key:'',attrs:{$:'Con',head:{$:'At',name:'',value:''},tail:{$:'Con',head:{$:'On',event:'',action:''},tail:{$:'Nil'}}},kids:{$:'Con',head:{$:'Txt',text:''},tail:{$:'Nil'}}});
 const m=await import('../../src/web/fold-feedback.js');return {...m,T:wire.T};
}
const L=xs=>xs.reduceRight((tail,head)=>({$:'Con',head,tail}),{$:'Nil'});
const kids=n=>{const o=[];for(let x=n.kids;x.$==='Con';x=x.tail)o.push(x.head);return o;};
function fx(T){
 const el=(tag,key,attrs,ks=[],on)=>({$:T.El,tag,key,attrs:L([...Object.entries(attrs).map(([name,value])=>({$:T.At,name,value})),...(on?[{$:'On',event:'click',action:on}]:[])]),kids:L(ks)});
 const head=open=>el('button','',{class:open?'fold open':'fold','data-value':(open?'0':'1')+'e1'},[],'fold');
 const row=k=>el('div',k,{class:'act','data-run':'e1'});
 const page=(open,extra=[])=>el('div','',{},[el('main','main',{},[el('div','tl',{id:'timeline'},[el('div','m0',{class:'msg'}),head(open),...(open?[row('e1'),row('e2')]:[]),el('div','m3',{class:'msg'}),...extra])])]);
 const keys=p=>kids(kids(kids(p)[0])[0]).map(k=>k.key||((x=>{for(let a=x.attrs;a.$==='Con';a=a.tail)if(a.head.name==='class')return a.head.value;})(k)));
 return {page,keys};
}
test('a run seen shut shuts at the click; seen open, it opens with the rows the hub drew',async()=>{
 const {FoldFeedback,T}=await load();let changed=0;const f=new FoldFeedback(()=>changed++),{page,keys}=fx(T);
 f.seen(page(false));
 assert.equal(f.choice('fold','1e1'),undefined,'never seen open: the hub draws its rows');
 f.seen(page(true));
 let settle=f.choice('fold','0e1');assert.ok(settle);
 assert.deepEqual(keys(f.project(page(true))).slice(0,3),['m0','fold','m3'],'rows gone, the shut head in place');
 settle(true);assert.equal(f.showing,false);
 f.seen(page(false,[]));settle=f.choice('fold','1e1');
 const shown=f.project(page(false));assert.deepEqual(keys(shown).slice(0,5),['m0','fold open','e1','e2','m3']);
 settle(true);assert.equal(changed,4);
 assert.equal(f.choice('fold','1sub:x'),undefined,'subagent rows are subs-feedback’s');
 assert.equal(f.choice('fold','0subs:t1'),undefined);
});
