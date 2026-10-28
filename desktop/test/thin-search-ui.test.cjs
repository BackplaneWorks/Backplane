'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=fs.readFileSync(path.resolve(__dirname,'../../src/web/host.js'),'utf8');
function trigger(mode,ready){
 const a=source.indexOf('function dispatch('),b=source.indexOf('  if (viewMode !== "compatibility")',a),panels=[],status=[];
 const ctx={viewMode:mode,thin:{ready},showThinSearch:x=>panels.push(x),showViewStatus:x=>status.push(x)};vm.createContext(ctx);vm.runInContext(source.slice(a,b)+'return false;}',ctx);
 return{ctx,panels,status};
}
test('human search entry opens an empty dialog only on a current thin connection, without any query',()=>{
 const f=trigger('view-v1',true);assert.equal(f.ctx.dispatch('thin-search-open',''),true);assert.equal(f.panels.length,1);assert.equal(f.panels[0].query,'');assert.equal(f.panels[0].rows.length,0);
});
test('cached, unready and compatibility search entry cannot query or acquire authority',()=>{
 for(const [mode,ready]of [['read-only',false],['view-v1',false],['compatibility',true]]){const f=trigger(mode,ready);f.ctx.dispatch('thin-search-open','');assert.equal(f.panels.length,0);assert.equal(f.status.length,1);}
});
// the search dialog (host.js's showThinSearch) over a small DOM
function panelFixture(){
 let connected=true;
 class Element{
  constructor(tag){this.tag=tag;this.children=[];this.handlers={};this.style={};this.attrs={};this.textContent='';const cls=new Set();this.classList={add:c=>cls.add(c),remove:c=>cls.delete(c),contains:c=>cls.has(c)};}
  get isConnected(){return connected;}
  append(...e){for(const c of e)c.parent=this;this.children.push(...e);}replaceChildren(...e){for(const c of e)c.parent=this;this.children=e;}
  remove(){const p=this.parent;if(p)p.children=p.children.filter(c=>c!==this);this.parent=null;}setAttribute(k,v){this.attrs[k]=v;}removeAttribute(k){delete this.attrs[k];}
  addEventListener(k,fn){this.handlers[k]=fn;}focus(){}scrollIntoView(){}
 }
 const panel=new Element('section'),calls=[],select=[];let closes=0;
 const ctx={resultPanel:()=>({panel,close:()=>{closes++;connected=false;}}),
  document:{createElement:tag=>new Element(tag),createTextNode:text=>({tag:'#text',textContent:text}),getElementById:id=>id==='find-rows'?panel.children[2]:null},
  thin:{search:(...args)=>{calls.push(args);return true;}},dispatch:(...args)=>select.push(args),later:()=>{},setTimeout,clearTimeout,Date};
 const a=source.indexOf('let thinFind = null;'),b=source.indexOf('function showThinMessage(',a);vm.createContext(ctx);vm.runInContext(source.slice(a,b),ctx);
 const parts=()=>({query:panel.children[0],note:panel.children[1],rows:panel.children[2]});
 const key=(k,more={})=>parts().query.handlers.keydown({key:k,isComposing:false,preventDefault(){},...more});
 return{ctx,panel,calls,select,parts,key,closes:()=>closes};
}
const narwhal={query:'narwhal',rows:[
 {thread:'A',id:'m1',title:'check the narwhal',project:'proj',here:true,snip:['narwhal16\n','narwhal',' 17'],when:'03:26',text:'narwhal16'},
 {thread:'B',id:'m2',title:'New thread',project:'multi',here:false,snip:['a ','narwhal',''],when:'Mon 26 Oct 14:05',text:'a narwhal'}],hasMore:true,cursor:'exact-next'};
const hits=rows=>rows.children.filter(e=>e.className==='find-hit');
test('actual query field submits exact Unicode on Enter only after composition commit',()=>{
 const f=panelFixture();f.ctx.showThinSearch({query:'',rows:[],hasMore:false});const {query}=f.parts();query.value='日本語 é';query.handlers.keydown({key:'Enter',isComposing:true});assert.equal(f.calls.length,0);
 f.key('Enter');assert.deepEqual(f.calls,[['日本語 é','']]);
});
test('typing searches after a pause, one search at a time, and a stale answer is not shown',async()=>{
 const f=panelFixture();f.ctx.showThinSearch({query:'',rows:[],hasMore:false});const {query,rows}=f.parts();
 query.value='nar';query.handlers.input();assert.equal(f.calls.length,0);
 await new Promise(r=>setTimeout(r,250));assert.deepEqual(f.calls,[['nar','']]);
 query.value='narwhal';query.handlers.input();await new Promise(r=>setTimeout(r,250));assert.equal(f.calls.length,1);
 f.ctx.thinSearchAnswer({query:'nar',rows:narwhal.rows,hasMore:false});assert.deepEqual(f.calls[1],['narwhal','']);assert.equal(hits(rows).length,0);
 f.ctx.thinSearchAnswer(narwhal);assert.equal(hits(rows).length,2);
});
test('results come under their project and thread, the words lit, the first chosen; a click opens the message in its thread',()=>{
 const f=panelFixture();f.ctx.showThinSearch({query:'',rows:[],hasMore:false});const {query,rows}=f.parts();query.value='narwhal';f.key('Enter');f.ctx.thinSearchAnswer(narwhal);
 assert.deepEqual(rows.children.map(e=>e.className),['find-group','find-hit','find-group','find-hit','find-more']);
 const head=rows.children[0];assert.deepEqual(head.children.map(e=>e.textContent),['proj','/','check the narwhal','this thread']);
 const [first,second]=hits(rows),snip=first.children[0];
 assert.deepEqual(snip.children.map(e=>[e.tag,e.textContent]),[['#text','narwhal16\n'],['mark','narwhal'],['#text',' 17']]);
 assert.equal(first.children[1].textContent,'03:26');assert.ok(first.classList.contains('on'));assert.ok(!second.classList.contains('on'));
 second.handlers.click();assert.deepEqual(f.select,[['select','B']]);assert.equal(f.closes(),1);assert.deepEqual(JSON.parse(JSON.stringify(f.ctx.searchJump)),{id:'m2',thread:'B',pages:0,asked:false,word:'narwhal'});
});
test('arrow keys choose a result and Enter opens it; More sends only the opaque cursor and adds the next page',()=>{
 const f=panelFixture();f.ctx.showThinSearch({query:'',rows:[],hasMore:false});const {query,rows}=f.parts();query.value='narwhal';f.key('Enter');f.ctx.thinSearchAnswer(narwhal);
 f.key('ArrowDown');assert.ok(hits(rows)[1].classList.contains('on'));f.key('ArrowDown');assert.ok(hits(rows)[0].classList.contains('on'));f.key('ArrowUp');assert.ok(hits(rows)[1].classList.contains('on'));
 rows.children.at(-1).handlers.click();assert.deepEqual(f.calls.at(-1),['narwhal','exact-next']);
 f.ctx.thinSearchAnswer({query:'narwhal',rows:[{thread:'B',id:'m3',title:'New thread',project:'multi',snip:['','narwhal',' again'],when:'Sun 25 Oct 09:00'}],hasMore:false,cursor:''});
 assert.equal(hits(rows).length,3);assert.deepEqual(rows.children.map(e=>e.className),['find-group','find-hit','find-group','find-hit','find-hit']);
 f.key('Enter');assert.deepEqual(f.select,[['select','B']]);
});
test('a search the hub refuses says what it takes',()=>{
 const f=panelFixture();f.ctx.showThinSearch({query:'',rows:[],hasMore:false});const {query,note}=f.parts();
 query.value='???';f.key('Enter');f.ctx.thinSearchFailed('LIMIT');assert.equal(note.textContent,'Search takes one to eight words of letters or digits.');
 query.value='ok';f.key('Enter');assert.deepEqual(f.calls.at(-1),['ok','']);
});
test('a search that finds nothing says so',()=>{
 const f=panelFixture();f.ctx.showThinSearch({query:'',rows:[],hasMore:false});const {query,note,rows}=f.parts();assert.match(note.textContent,/Type a word/);
 query.value='zebra';f.key('Enter');assert.equal(note.textContent,'Searching…');f.ctx.thinSearchAnswer({query:'zebra',rows:[],hasMore:false,cursor:''});
 assert.equal(hits(rows).length,0);assert.equal(note.textContent,'Nothing matches “zebra”.');
});
test('owned Bend sidebar source exposes the named accessible human search control',()=>{
 const view=fs.readFileSync(path.resolve(__dirname,'../../src/web/view.bend'),'utf8');assert(view.includes('WI.WIcon.button("icon", "thin-search-open", "", "Search conversations", "search", 16, "")'));
});
