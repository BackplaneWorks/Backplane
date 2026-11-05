import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {BodyWindows} from '../../src/web/body-windows.js';
import {validateBodyPage} from '../../src/web/thin-results.js';
const list = xs => xs.reduceRight((tail,head)=>({$: 'Con',head,tail}),{$:'Nil'});
const elems = xs => {const out=[];for(;xs?.$==='Con';xs=xs.tail)out.push(xs.head);return out};
const node = (tag,key,attrs={},kids=[])=>({$:'El',tag,key,attrs:list(Object.entries(attrs).map(([name,value])=>({$:'At',name,value}))),kids:list(kids)});
const attrs = n => Object.fromEntries(elems(n.attrs).map(a=>[a.name,a.value]));
const text = value => ({$:'Txt',text:value});
const read = n => n?.$==='Txt'? n.text:elems(n?.kids).map(read).join('');
const part = (automatic=true)=>({id:'m',thread:'a',version:'7',cursor:'0',bodyPage:1,automatic});
const detail = (partial=[part()],generation='1',viewId='v')=>({epoch:'e',generation,viewId,target:{kind:'thread',id:'a'},window:{partial}});
const root = (activity=false)=>node('div','timeline',{id:'timeline'},[activity?node('div','m',{class:'act','data-id':'m'},[text('tool summary')]):node('bp-memo','m',{fn:'msg',who:'a'},[text('real prefix')])]);
function timeline(b,activity=false,direction='next',visible=true,inputRoot=root(activity)) {
 const projected=b.project(inputRoot), all=[];
 const walk=n=>{if(n?.$!=='El')return;const a=attrs(n);if(a['data-body-edge']||a['data-body-start']!==undefined)all.push(a);for(const k of elems(n.kids))walk(k)};walk(projected);
 return {scrollTop:100,scrollHeight:1600,clientHeight:600,getBoundingClientRect:()=>({top:0,bottom:600}),querySelectorAll:s=>all.filter(a=>s==='[data-body-start]'?a['data-body-start']!==undefined:a['data-body-edge']).map(a=>({dataset:{bodyId:a['data-body-id'],bodyStart:String(a['data-body-start']),bodyEdge:a['data-body-edge']},getBoundingClientRect:()=>({top:visible&&(!a['data-body-edge']||a['data-body-edge']===direction)?100:5000,bottom:visible&&(!a['data-body-edge']||a['data-body-edge']===direction)?140:5040,height:40})}))};
}
let checks=0;const check=(name,f)=>{f();checks++;console.log('ok '+name)};
const setup=(automatic=true)=>{const requests=[];const b=new BodyWindows({request:(p,cursor,tail)=>{requests.push({entryId:p.id,bodyPage:1,bodyVersion:p.version,bodyThread:p.thread,localGeneration:p.localGeneration,cursor,generation:b.generation,viewId:b.viewId,accountBinding:b.owner,tail});return true},changed:()=>{}});b.bind('h',detail([part(automatic)]),'owner',root(!automatic));return {b,requests}};
check('closing a tool run retires its body and an old reply cannot revive it after reopening',()=>{
 const {b,requests}=setup(false);
 const open=node('div','timeline',{id:'timeline'},[node('button','fold:r',{class:'fold open','data-value':'0r'},[text('1 tool call')]),node('div','m',{class:'act','data-id':'m','data-run':'r'},[text('tool summary')])]);
 const closed=node('div','timeline',{id:'timeline'},[node('button','fold:r',{class:'fold','data-value':'1r'},[text('1 tool call')])]);
 b.bind('h',detail([part(false)]),'owner',open);assert(b.open('m'));b.observe(timeline(b,true,'next',true,open),true);const old=requests.at(-1);assert(old);
 b.bind('h',detail([]),'owner',closed);assert.equal(b.pending,null);assert.equal(b.parts.has('m'),false);assert(!read(b.project(closed)).includes('private tool payload'));
 const message={id:'m',thread:'a',version:'7',bodyPage:1,start:0,end:20,total:20,text:'private tool payload',hasMore:false,complete:true,cursor:'',previous:''};
 assert.equal(b.accept(message,old),false,'collapse retires the outstanding body read');
 b.bind('h',detail([part(false)]),'owner',open);assert(b.open('m'));b.observe(timeline(b,true,'next',true,open),true);const current=requests.at(-1);
 assert.notEqual(current.localGeneration,old.localGeneration);assert.equal(b.accept(message,old),false,'same cursor/version cannot revive a previous expansion');
 assert(b.accept(message,current));assert(read(b.project(open)).includes('private tool payload'));
});
check('collapsed tool body has no node construction or automatic request',()=>{const {b,requests}=setup(false);assert.equal(read(b.project(root(true))),'tool summary');b.observe(timeline(b,true),true);assert.equal(requests.length,0)});
check('offscreen large message does not request its body',()=>{const {b,requests}=setup();b.observe(timeline(b,false,'next',false),true);assert.equal(requests.length,0);assert(read(b.project(root())).includes('real prefix'))});
check('explicit tool expansion requests actual scoped body only once',()=>{const {b,requests}=setup(false);assert(b.open('m'));b.observe(timeline(b,true),true);b.observe(timeline(b,true),true);assert.equal(requests.length,1);assert.equal(requests[0].bodyThread,'a')});
check('complete traversal of1MiB source retains bounded displayed pages/envelopes',()=>{const {b,requests}=setup();const want=createHash('sha256'),got=createHash('sha256');for(let i=0;i<128;i++){const source=`chunk${String(i).padStart(3,'0')}:`+'x'.repeat(8183);want.update(source);b.observe(timeline(b),true);const input=requests.at(-1),m={id:'m',thread:'a',version:'7',bodyPage:1,start:i*8192,end:(i+1)*8192,total:128*8192,text:source,hasMore:i<127,complete:i===127,cursor:i<127?String((i+1)*8192):'',previous:i?String((i-1)*8192):''};assert(validateBodyPage(m,input));assert(b.accept(m,input));got.update(m.text);assert(b.retainedBytes()<=1024*1024);const projected=JSON.stringify(b.project(root()));assert(projected.length<130000);if(i>6)assert(!read(b.project(root())).includes('chunk000:'));}assert.equal(got.digest('hex'),want.digest('hex'));assert.equal(requests.length,128)});
check('A to B to A rejects old body publication after generation/view change',()=>{const {b,requests}=setup();b.observe(timeline(b),true);const old=requests.at(-1);b.disconnect();b.bind('h',detail([part()],'2','v2'),'owner',root());const m={id:'m',thread:'a',version:'7',bodyPage:1,start:0,end:1,total:1,text:'stale owner content',hasMore:false,complete:true,cursor:'',previous:''};assert.equal(b.accept(m,old),false);assert(!read(b.project(root())).includes('stale owner content'))});
check('new account never restores another account body pages',()=>{const {b,requests}=setup();b.observe(timeline(b),true);const input=requests.at(-1);assert(b.accept({id:'m',thread:'a',version:'7',bodyPage:1,start:0,end:1,total:1,text:'private-owner-A',hasMore:false,complete:true,cursor:'',previous:''},input));b.bind('h',detail(),'owner-B',root());assert(!read(b.project(root())).includes('private-owner-A'))});
check('new short activity invalidates old expanded body cache',()=>{const {b}=setup(false);b.open('m');b.bind('h',detail([]),'owner',root(true));assert.equal(read(b.project(root(true))),'tool summary')});
check('body packet rejects cross-thread, extra hidden data and oversize text',()=>{const input={bodyPage:1,bodyVersion:'7',bodyThread:'a'},m={id:'m',thread:'a',version:'7',bodyPage:1,start:0,end:1,total:1,text:'x',hasMore:false,complete:true,cursor:'',previous:''};assert(validateBodyPage(m,input));assert(!validateBodyPage({...m,thread:'b'},input));assert(!validateBodyPage({...m,extra:'hidden'},input));assert(!validateBodyPage({...m,text:'x'.repeat(8197)},input))});
check('append-only live versions retain painted pages and refresh only their bounded tail',()=>{
 const requests=[],id='@live:a';
 const cursor=(version,start=0,until=0)=>JSON.stringify({version:1,epoch:'e',target:{kind:'thread',id:'a'},query:`body:${version}:${until}`,before:String(start)});
 const live=(version,blocks=24,generation='1',cut=false)=>({...part(),id,version:String(version),cursor:cursor(version),streamGeneration:generation,streamBlocks:blocks,streamCut:cut});
 const rr=node('div','timeline',{id:'timeline'},[node('bp-memo',id,{fn:'msg',who:'a'},[text('tail preview')])]);
 const b=new BodyWindows({request:(p,c,tail)=>{requests.push({entryId:id,bodyPage:1,bodyVersion:p.version,bodyThread:p.thread,localGeneration:p.localGeneration,cursor:c,generation:b.generation,viewId:b.viewId,accountBinding:b.owner,tail});return true},changed:()=>{}});
 b.bind('h',detail([live(7)]),'owner',rr);
 b.observe(timeline(b,false,'next',true,rr),true);
 assert(b.accept({id,thread:'a',version:'7',bodyPage:1,start:8,end:24,total:24,text:'READING TEXT',hasMore:false,complete:true,cursor:'',previous:cursor(7,0,8)},requests.at(-1)));
 const old=b.reader(id).pages[0];
 b.bind('h',detail([live(8,25)]),'owner',rr,true);
 assert.equal(b.reader(id).pages[0],old);assert(read(b.project(rr)).includes('READING TEXT'));
 b.observe(timeline(b,false,'next',true,rr),true);
 assert.equal(requests.at(-1).bodyVersion,'8');assert.equal(JSON.parse(requests.at(-1).cursor).before,'8');
 assert(b.accept({id,thread:'a',version:'8',bodyPage:1,start:8,end:24,total:25,text:'READING TEXT plus append',hasMore:true,complete:false,cursor:cursor(8,24),previous:cursor(8,0,8)},requests.at(-1)));
 assert(read(b.project(rr)).includes('READING TEXT plus append'));
 b.bind('h',detail([live(9,26,'2')]),'owner',rr,true);assert.equal(b.reader(id),null,'new stream incarnation cannot inherit old text');
 b.bind('h',detail([live(10,27)]),'owner',rr);b.reader(id,true);b.bind('h',detail([live(11,28,'1',true)]),'owner',rr,true);assert.equal(b.reader(id),null,'cut source cannot inherit block positions');
});
check('message Markdown construction is once per fetched page and never for collapsed tools',()=>{
 const {b,requests}=setup();let count=0;b.renderBody=n=>{if(read(n))count++;return node('div','made',{},[node('div','',{class:'body'},[node('strong','',{},[text(read(n))])])])};
 b.observe(timeline(b),true);assert.equal(count,1,'preview Markdown rendered synchronously');assert(b.accept({id:'m',thread:'a',version:'7',bodyPage:1,start:0,end:7,total:7,text:'**bold**',hasMore:false,complete:true,cursor:'',previous:''},requests.at(-1)));
 const page=b.project(root());assert.equal(count,2);b.project(root());assert.equal(count,2);assert(JSON.stringify(page).includes('strong'));
 const closed=setup(false);closed.b.renderBody=()=>{throw Error('collapsed renderer executed')};closed.b.project(root(true));
});
check('preview is Markdown on its first frame and cached by entry version',()=>{
 const {b}=setup();let renders=0;b.renderBody=n=>{if(read(n))renders++;return node('div','m',{},[node('div','',{class:'body'},[node('strong','',{},[text(read(n))])])])};
 const first=b.project(root()),again=b.project(root());assert(JSON.stringify(first).includes('strong'));assert(!JSON.stringify(first).includes('"tag":"pre"'));assert.equal(renders,1);
 assert.equal(elems(elems(first.kids)[0].kids)[0].kids.head,elems(elems(again.kids)[0].kids)[0].kids.head,'cached Markdown part identity survives projection');
 b.bind('h',detail([{...part(),version:'8'}]),'owner',root());b.project(root());assert.equal(renders,2,'new version renders before publication');
});
check('already rendered message preview keeps structured Markdown nodes',()=>{
 const {b}=setup();const made=node('div','timeline',{id:'timeline'},[node('div','m',{class:'msg assistant','data-id':'m'},[node('div','',{class:'body'},[node('strong','',{},[text('bold')]),node('a','',{href:'https://example.com'},[text('link')])])])]);
 const projected=b.project(made);assert(JSON.stringify(projected).includes('strong'));assert(JSON.stringify(projected).includes('https://example.com'));assert(!JSON.stringify(projected).includes('"tag":"pre"'));
});
check('five simultaneous visible body readers settle without eviction/refetch flashing',()=>{
 const requests=[],parts=Array.from({length:5},(_,i)=>({...part(),id:'m'+i})),rr=node('div','timeline',{id:'timeline'},parts.map(p=>node('bp-memo',p.id,{fn:'msg',who:'a'},[text('**preview**')])));
 const b=new BodyWindows({request:(p,cursor)=>{requests.push({entryId:p.id,bodyVersion:p.version,bodyThread:p.thread,localGeneration:p.localGeneration,cursor,generation:b.generation,viewId:b.viewId,accountBinding:b.owner});return true}});
 b.bind('h',detail(parts),'owner',rr);
 for(let i=0;i<20;i++){
  const n=requests.length;b.observe(timeline(b,false,'next',true,rr),true);
  if(requests.length>n){const input=requests.at(-1);assert(b.accept({id:input.entryId,thread:'a',version:'7',bodyPage:1,start:0,end:7,total:7,text:'**full**',hasMore:false,complete:true,cursor:'',previous:''},input));}
 }
 assert.equal(requests.length,5,'each complete visible body is fetched once');assert.equal(b.scopes.get(b.scope).size,5);assert(b.retainedBytes()<=b.maxBytes);
 const old=b.scope;b.bind('h',{...detail([]),target:{kind:'thread',id:'other'}},'owner',node('div','timeline',{id:'timeline'}));assert.equal(b.scopes.get(old).size,4,'parked scope retains only four readers');
});
check('an older retained timeline body is not invalidated by an adjacent hub page',()=>{
 const {b,requests}=setup();b.observe(timeline(b),true);assert(b.accept({id:'m',thread:'a',version:'7',bodyPage:1,start:0,end:4,total:4,text:'full',hasMore:false,complete:true,cursor:'',previous:''},requests.at(-1)));
 const old=b.reader('m').pages[0];b.bind('h',{...detail([]),window:{ids:['newer'],partial:[]}},'owner',root());assert.equal(b.reader('m').pages[0],old);assert(read(b.project(root())).includes('full'));
});
check('a windowed bot message keeps its header, Reply and a whole-message Copy',()=>{
 // as view.bend's View.msg.x.loc draws a bot chat's message: header, body, stamp, Copy, Reply
 const made=n=>{const a=attrs(n),id=n.key;return node('div',id,{class:'msg assistant','data-id':id},[
  ...(a.ctx?[node('div','',{class:'msg-ctx'},[node('a','',{class:'ref ctx',href:a.ctx,'data-on-click':'ref-go'},[text(a.ctxt)])])]:[]),
  node('div','',{class:'body'},[text(read(n))]),node('span','',{class:'stamp'},[text('')]),
  node('button','',{class:'copy',title:'Copy','data-on-click':'copy','data-value':read(n)},[text('Copy')]),
  ...(a.rpl==='1'?[node('button','',{class:'reply',title:'Reply','data-on-click':'reply-to','data-value':id},[text('Reply')])]:[])])};
 const rr=()=>node('div','timeline',{id:'timeline'},[node('bp-memo','m',{fn:'msg',who:'a',rpl:'1',ctx:'backplane://thread/t1',ctxt:'Boost'},[text('preview of a long report')])]);
 const {b,requests}=setup();b.renderBody=made;
 const shape=root=>{const msg=elems(b.project(root).kids)[0],ks=elems(msg.kids);
  return {classes:ks.map(k=>attrs(k).class),copy:ks.find(k=>attrs(k).class==='copy'),reply:ks.find(k=>attrs(k).class==='reply')}};
 const before=shape(rr());
 assert.deepEqual(before.classes,['msg-ctx','body body-window','stamp','copy','reply'],'preview: header, body window, stamp, Copy, Reply');
 b.observe(timeline(b,false,'next',true,rr()),true);
 assert(b.accept({id:'m',thread:'a',version:'7',bodyPage:1,start:0,end:9,total:99,text:'page one ',hasMore:true,complete:false,cursor:'9',previous:''},requests.at(-1)));
 const after=shape(rr());
 assert.deepEqual(after.classes,['msg-ctx','body body-window','stamp','copy','reply'],'pages: the same parts around the window');
 for(const x of [before,after]){
  assert.equal(attrs(x.copy)['data-on-click'],'copy-entry');assert.equal(attrs(x.copy)['data-value'],'m','Copy names the message, the hub copies all of it');
  assert.equal(read(x.copy),'Copy');assert.equal(attrs(x.reply)['data-on-click'],'reply-to');assert.equal(attrs(x.reply)['data-value'],'m');
 }
 const page=JSON.stringify(b.project(rr()));assert(!page.includes('displayed part'));
 assert.equal((page.match(/"reply-to"/g)||[]).length,1,'one Reply, outside the pages');
});
check('a picture of a thread read before draws the bodies read for it; never another thread’s, never a streaming row, never the one bound now',()=>{
 const {b,requests}=setup();
 const detailOf=(id,parts)=>({epoch:'e',generation:'1',viewId:'v',target:{kind:'thread',id},window:{partial:parts}});
 const pa={id:'m',thread:'a',version:'7',cursor:'0',bodyPage:1,automatic:true}, live={id:'@live:a',thread:'a',version:'1',cursor:'0',bodyPage:1,automatic:true};
 const rootA=node('div','timeline',{id:'timeline'},[node('bp-memo','m',{fn:'msg',who:'a'},[text('real prefix')]),node('bp-memo','@live:a',{fn:'msg',who:'a'},[text('streaming')])]);
 b.bind('h',detailOf('a',[pa,live]),'owner',rootA,false,'thread:a');
 b.observe(timeline(b,false,'next',true,rootA),true);
 assert(b.accept({id:'m',thread:'a',version:'7',bodyPage:1,start:0,end:12,total:12,text:'whole body A',hasMore:false,complete:true,cursor:'',previous:''},requests.find(r=>r.entryId==='m')));
 assert.equal(b.projectHeld(rootA,'thread:a'),rootA,'the destination bound now draws through project(), not held()');
 const rootB=node('div','timeline',{id:'timeline'},[node('bp-memo','n',{fn:'msg',who:'a'},[text('b prefix')])]);
 b.bind('h',detailOf('b',[{id:'n',thread:'b',version:'3',cursor:'0',bodyPage:1,automatic:true}]),'owner',rootB,false,'thread:b');
 const held=b.projectHeld(rootA,'thread:a'),again=b.project(rootA,{parts:new Map([['m',pa]]),reader:()=>null});
 assert(read(held).includes('whole body A'),'A’s page drawn at once');
 assert.equal(attrs(elems(held.kids)[0]).class,'msg assistant','as the hub’s view will draw it: a message with a body window');
 assert.equal(elems(held.kids)[1],elems(rootA.kids)[1],'the streaming row is left to the hub');
 assert(!read(again).includes('whole body A'));
 assert.equal(b.projectHeld(rootB,'thread:b'),rootB,'B is bound now');
 assert.equal(b.projectHeld(rootA,'thread:c'),rootA,'nothing held for C');
 assert(read(b.projectHeld(rootB,'thread:a')).includes('b prefix'),'held parts only touch their own ids');
 assert.equal(requests.filter(r=>r.entryId==='n').length,0,'held drawing asks nothing');
});
check('a body not read yet whose whole text a prefetched picture had draws it as its first page will, and still reads the page',()=>{
 const {b,requests}=setup();
 b.renderBody=n=>({...n,tag:'div',kids:list([node('p','',{},[text(read(n))])])});
 b.wholes(node('div','timeline',{id:'timeline'},[node('bp-memo','m',{fn:'msg',who:'a'},[text('the whole message')]),node('bp-memo','@live:x',{fn:'msg'},[text('streaming')])]));
 const before=b.project(root());
 assert(read(before).includes('the whole message'),'the whole text at once');assert(!read(before).includes('Loading message text'),'nothing says it is loading');
 b.observe(timeline(b,false,'next',true,root()),true);assert.equal(requests.length,1,'the page is still read');
 assert(b.accept({id:'m',thread:'a',version:'7',bodyPage:1,start:0,end:17,total:17,text:'the whole message',hasMore:false,complete:true,cursor:'',previous:''},requests[0]));
 const after=b.project(root()),part=k=>JSON.stringify(elems(elems(k.kids).find(x=>attrs(x).class==='body body-window').kids)[0]);
 assert.equal(part(elems(after.kids)[0]),part(elems(before.kids)[0]),'the page draws the very same part');
 assert(!b.whole.has('@live:x'),'never a streaming row');
});
console.log(JSON.stringify({pass:true,checks,scope:'Read-only body source, bounded retention and current owner/version fences; no service or DOM/anchor qualification'}));
