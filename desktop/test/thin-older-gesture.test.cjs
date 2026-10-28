'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
function fixture(){
 const source=fs.readFileSync(path.resolve(__dirname,'../../src/web/host.js'),'utf8'),a=source.indexOf('function earlierCheck('),b=source.indexOf('document.addEventListener("click",',a),sent=[];
 class Element{constructor(){this.id='timeline';this.scrollTop=0;this.clientHeight=800;this.scrollHeight=2000;}closest(){return this;}querySelector(){return{getAttribute:()=> 'own-thread'};}getBoundingClientRect(){return{right:390};}}
 const frames=[],tl0={v:null},ctx={Element,viewMode:'view-v1',earlierAsked:false,historyBase:null,historyPage:null,performance:{now:()=>1},document:{addEventListener(){},body:{},getElementById:()=>tl0.v},dispatch:(a,v)=>{sent.push({a,v});return ctx.refuse?false:true;},requestAnimationFrame:f=>frames.push(f),leavingEndUntil:0,towardEndUntil:0,held:null,heldSize:null,focusedNode:null,frontendNode:null,anchorOf:()=>({id:'top',off:0}),holdSize:()=>({}),holdSay:v=>ctx.said.push(v),said:[],App:{sel:()=>'A'},ui:null,thin:null};vm.createContext(ctx);vm.runInContext(source.slice(a,b),ctx);const tl=new Element();tl0.v=tl;return{ctx,tl,sent,frames,run:()=>{while(frames.length)frames.shift()();},event:(type,extra)=>ctx.noteOlderGesture({type,target:tl,isTrusted:true,...extra})};
}
test('render/initial scroll alone cannot start automatic older paging',()=>{
 const f=fixture();f.ctx.earlierCheck(f.tl);assert.equal(f.sent.length,0);
});
test('one upward gesture continues across short pages, with a bounded stationary burst',()=>{
 const f=fixture();f.event('wheel',{deltaY:-40});for(let i=0;i<12;i++){f.ctx.earlierCheck(f.tl);f.ctx.earlierAsked=false;}assert.equal(f.sent.length,4);
 f.event('wheel',{deltaY:-40});f.ctx.earlierCheck(f.tl);assert.equal(f.sent.length,5,'continued input resumes paging');
});
test('downward scrolling and ordinary text clicks do not trigger older download',()=>{
 const f=fixture();f.event('wheel',{deltaY:40});f.ctx.earlierCheck(f.tl);f.event('pointerdown',{button:0,clientX:50});f.ctx.earlierCheck(f.tl);assert.equal(f.sent.length,0);
});
test('reversing into newer spare rows permits paging down, while further upward eviction stays blocked',()=>{
 const f=fixture();f.ctx.historyBase=[{id:'canonical'}];f.ctx.historySpare=[{id:'top'}];f.ctx.historySpare.direction='older';f.ctx.historyRow=n=>n.id;
 assert.equal(f.ctx.historyCanPage(f.tl,'newer'),true);assert.equal(f.ctx.historyCanPage(f.tl,'older'),false);
 f.ctx.historySpare.direction='newer';assert.equal(f.ctx.historyCanPage(f.tl,'older'),true);assert.equal(f.ctx.historyCanPage(f.tl,'newer'),false);
});
test('a finger movement toward older messages qualifies; a downward-reading gesture does not',()=>{
 const f=fixture();f.event('touchstart',{touches:[{clientY:100}]});f.event('touchmove',{touches:[{clientY:80}]});f.ctx.earlierCheck(f.tl);assert.equal(f.sent.length,0);f.event('touchmove',{touches:[{clientY:120}]});f.ctx.earlierCheck(f.tl);assert.equal(f.sent.length,1);
});
test('on an earlier page a downward gesture near its bottom asks for the newer page, once a render',()=>{
 const f=fixture(),asked=[];f.ctx.thin={atNewest:()=>false,newer:()=>asked.push(1)};f.tl.scrollTop=1200;
 f.ctx.laterCheck(f.tl);assert.equal(asked.length,0,'no gesture, no page');
 f.event('wheel',{deltaY:40});f.ctx.laterCheck(f.tl);f.ctx.laterCheck(f.tl);assert.equal(asked.length,1);
 f.ctx.laterAsked=false;f.event('wheel',{deltaY:-40});f.ctx.laterCheck(f.tl);assert.equal(asked.length,1,'an upward gesture does not page down');
});
test('at the newest there is nothing below to ask for',()=>{
 const f=fixture(),asked=[];f.ctx.thin={atNewest:()=>true,newer:()=>asked.push(1)};f.tl.scrollTop=1200;f.event('wheel',{deltaY:40});f.ctx.laterCheck(f.tl);assert.equal(asked.length,0);
});
test('the latest gesture wins: a swipe down right after one up asks for no older page',()=>{
 const f=fixture();f.event('wheel',{deltaY:-40});f.event('wheel',{deltaY:40});f.ctx.earlierCheck(f.tl);assert.equal(f.sent.length,0);
 const asked=[];f.ctx.thin={atNewest:()=>false,newer:()=>asked.push(1)};f.tl.scrollTop=1200;f.event('wheel',{deltaY:40});f.event('wheel',{deltaY:-40});f.ctx.laterCheck(f.tl);assert.equal(asked.length,0);
});
test('a gesture asks by itself where no scroll event comes: a timeline at its very top',()=>{
 const f=fixture();f.event('wheel',{deltaY:-40});assert.equal(f.sent.length,0,'nothing until the frame');f.run();assert.equal(f.sent.length,1,'the gesture asked for the older page');
 const asked=[];f.ctx.thin={atNewest:()=>false,newer:()=>{asked.push(1);return true;}};f.tl.scrollTop=1200;f.event('wheel',{deltaY:40});f.run();assert.equal(asked.length,1,'and at an earlier page\'s bottom, the newer one');
});
test('a page refused while another is on its way leaves the gesture standing: the next check asks again',()=>{
 const f=fixture();f.ctx.refuse=true;f.event('wheel',{deltaY:-40});f.ctx.earlierCheck(f.tl);assert.equal(f.ctx.earlierAsked,false);f.ctx.refuse=false;f.ctx.earlierCheck(f.tl);assert.equal(f.sent.length,2);
 const asked=[];let ok=false;f.ctx.thin={atNewest:()=>false,newer:()=>{asked.push(1);return ok;}};f.tl.scrollTop=1200;f.event('wheel',{deltaY:40});f.ctx.laterCheck(f.tl);ok=true;f.ctx.laterCheck(f.tl);assert.equal(asked.length,2,'refused, then asked again');f.ctx.laterCheck(f.tl);assert.equal(asked.length,2,'once accepted, once a render');
});
test('an upward notch at the end leaves the end at once (a smooth scroll\'s first frame is still within it)',()=>{
 const f=fixture();f.tl.scrollTop=1200;f.event('wheel',{deltaY:-40});assert.deepEqual(f.ctx.held,{id:'top',off:0});assert.deepEqual(f.ctx.said,['A']);assert(f.ctx.leavingEndUntil>1);
 f.event('wheel',{deltaY:40});assert.equal(f.ctx.leavingEndUntil,0,'a downward gesture lets the end catch the view again');
 const g=fixture();g.tl.scrollHeight=800;g.event('wheel',{deltaY:-40});assert.equal(g.ctx.held,null,'a timeline that does not scroll holds nothing');
});
test('a downward gesture lets the end catch the view from near it; an upward one does not',()=>{
 const f=fixture();f.tl.scrollTop=1200;f.event('wheel',{deltaY:40});assert(f.ctx.towardEndUntil>1,'down: near the end counts');
 f.event('wheel',{deltaY:-40});assert.equal(f.ctx.towardEndUntil,0,'up: only the end itself');
});
test('a page that came is drawn before the next is asked for (S07: two pages in one frame dropped the rows on screen)',()=>{
 const f=fixture(),asked=[];f.ctx.thin={atNewest:()=>false,newer:()=>{asked.push(1);return true;}};f.tl.scrollTop=1200;
 f.ctx.historyPage={direction:'newer'};f.event('wheel',{deltaY:40});f.ctx.laterCheck(f.tl);assert.equal(asked.length,0,'not while the last page waits for its render');
 f.event('wheel',{deltaY:-40});f.ctx.earlierCheck(f.tl);assert.equal(f.sent.length,0,'nor an older one');
 f.ctx.historyPage=null;f.event('wheel',{deltaY:40});f.ctx.laterCheck(f.tl);assert.equal(asked.length,1,'once drawn, the next page is asked for');
});
