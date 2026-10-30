'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const list=xs=>xs.reduceRight((tail,head)=>({$: 'Con',head,tail}),{$:'Nil'});
const el=(tag,attrs,kids=[])=>({$:'El',tag,key:'',attrs:list(Object.entries(attrs).map(([name,value])=>({$:'At',name,value}))),kids:list(kids)});
const kids=n=>{const out=[];for(let xs=n?.kids;xs?.head;xs=xs.tail)out.push(xs.head);return out;};
const all=n=>[n,...kids(n).flatMap(all)];
const attr=(n,key)=>{for(let xs=n?.attrs;xs?.head;xs=xs.tail)if(xs.head.name===key)return xs.head.value;};
const button=id=>({...el('button',{'data-value':JSON.stringify({entryId:id,cursor:''})}),attrs:list([{$:'At',name:'data-value',value:JSON.stringify({entryId:id,cursor:''})},{$:'On',event:'click',action:'thin-message-more'}])});
const row=id=>el('div',{'class':'act','data-id':id},[el('div',{'class':'body'},[{$:'Txt',text:'preview'}])]);
const msg=id=>el('div',{'class':'msg assistant','data-id':id},[el('div',{'class':'body'},[{$:'Txt',text:'whole message text'}])]);
const timeline=xs=>el('div',{id:'timeline'},xs);
async function setup(ids){const {ThinContinuations}=await import(process.env.CONTINUATIONS_SOURCE||'../../src/web/thin-continuations.js');const c=new ThinContinuations();c.bind('h',{epoch:'e',target:{kind:'thread',id:'t'},window:{partial:ids.map(id=>({id,version:'1'}))}});return c;}
test('hidden partial entries produce no Additional-text disclosure or orphan footer; showing exact row restores its reader',async()=>{
 const c=await setup(['hidden']),hidden=c.project(timeline([button('hidden')]),n=>n);assert.equal(kids(hidden).length,0);assert.equal(c.partial.size,1);
 const shown=c.project(timeline([row('hidden'),button('hidden')]),n=>n);assert.equal(kids(shown).length,1);assert.equal(kids(kids(shown)[0]).at(-1).tag,'button');assert(!all(shown).some(n=>n.tag==='details'||n.tag==='summary'));
});
test('a complete explicit reader remains plaintext inline in its exact unique row without extra disclosure',async()=>{
 const c=await setup(['a']);assert(c.accept({id:'a',version:'1',text:'日本語 <script>text only</script>',hasMore:false}));
 const shown=c.project(timeline([row('a'),button('a')]),n=>n),pre=all(shown).find(n=>n.tag==='pre');assert(pre);assert.equal(kids(pre)[0].text,'日本語 <script>text only</script>');assert(!all(shown).some(n=>n.tag==='details'||n.tag==='summary'||n.tag==='button'));assert.equal(kids(shown).length,1);
});
test('ambiguous same-ID rows and duplicate controls create no guessed reader and retain issued metadata',async()=>{
 const c=await setup(['a']),shown=c.project(timeline([row('a'),row('a'),button('a'),button('a')]),n=>n);assert.equal(kids(shown).length,2);assert(!all(shown).some(n=>n.tag==='button'||n.tag==='details'));assert.equal(c.partial.size,1);assert.equal(c.readers.size,0);
});
test('a message row never gets a read-more control, even when the window names it partial',async()=>{
 const c=await setup(['m']),shown=c.project(timeline([msg('m'),button('m')]),n=>n);
 assert(!all(shown).some(n=>n.tag==='button'));assert(!all(shown).some(n=>n.tag==='pre'));
 assert.equal(all(shown).filter(n=>n.$==='Txt').map(n=>n.text).join(''),'whole message text');
 const src=require('node:fs').readFileSync(require('node:path').join(__dirname,'../../src/web/thin-continuations.js'),'utf8');assert(!src.includes('Read rest of message'));
});
test('saved opening pictures never show three More footer rows, even without matching partial metadata',async()=>{
 const c=await setup(['other-scope']);
 const transported=id=>el('button',{'class':'thin-message-more','data-on-click':'thin-message-more','data-value':JSON.stringify({entryId:id,cursor:''})},[{$:'Txt',text:'More'}]);
 for(const controls of [[button('a'),button('b'),button('c')],[transported('a'),transported('b'),transported('c')],[button('a'),button('a'),button('a')]]){
  const tree=timeline([msg('last'),...controls]),before=JSON.stringify(tree),shown=c.project(tree,n=>n,false);
  assert.equal(kids(shown).length,1,'no continuation rows after the final live-end entry');
  assert(!all(shown).some(n=>n.tag==='button'));assert.equal(JSON.stringify(tree),before,'saved picture is immutable');
 }
});
test('read-only pictures never restore another live scope reader into a matching row ID',async()=>{
 const c=await setup(['a']);assert(c.accept({id:'a',version:'1',text:'live scope private body',hasMore:true,cursor:'next'}));
 const shown=c.project(timeline([row('a'),button('a')]),n=>n,false);
 assert(!all(shown).some(n=>n.tag==='button'||n.tag==='pre'));assert(!all(shown).some(n=>n.text==='live scope private body'));
 assert.equal(c.readers.size,1,'picture projection cannot mutate the current reader');
 const live=c.project(timeline([row('a'),button('a')]),n=>n);assert(all(live).some(n=>n.tag==='pre'));assert(all(live).some(n=>n.tag==='button'),'current authoritative row retains its explicit reader');
});
test('unknown partial metadata cannot leave orphan controls in the authoritative timeline either',async()=>{
 const c=await setup([]),shown=c.project(timeline([msg('last'),button('stale')]),n=>n);
 assert.equal(kids(shown).length,1);assert.equal(c.partial.size,0);
});
