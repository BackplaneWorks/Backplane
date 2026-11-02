'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=()=>fs.readFileSync(process.env.READER_HOST_SOURCE||path.join(__dirname,'../../src/web/host.js'),'utf8');
test('an underfilled entry loads real older pages before a wheel can generate scroll',()=>{
 const src=source(),a=src.indexOf('function earlierCheck(tl)'),b=src.indexOf('function laterCheck(tl)',a),requests=[];
 const tl={id:'timeline',scrollHeight:700,clientHeight:700,scrollTop:0};
 const context=vm.createContext({historyDirection:'',historyFillScope:'scope',historyFillOwner:()=> 'scope',historyFillSeeking:true,historySeekMessage:()=>false,historyFillPages:0,earlierAsked:false,historyFillMore(){},historyHasMessage:t=>t.scrollHeight>t.clientHeight+10,thin:{ready:true,pageError:null,paging:null,hasAhead:()=>true,ahead:(direction,options)=>{requests.push({direction,options});return true;}},historyGap:()=>0,historyRunway:()=>1400});
 const check=vm.runInContext('('+src.slice(a,b)+')',context);check(tl);
 assert.equal(requests.length,1);assert.equal(requests[0].direction,'older');assert.equal(requests[0].options.two,true);
 context.earlierAsked=false;tl.scrollHeight=1200;check(tl);assert.equal(requests.length,1,'stop automatic filling once history scrolls');
 tl.scrollHeight=700;context.historyFillSeeking=true;context.historyFillPages=32;check(tl);assert.equal(requests.length,1,'automatic work remains bounded');
 context.historyDirection='older';check(tl);assert.equal(requests.length,2,'explicit user input can continue past the automatic budget');
});
test('usable history means an actually visible committed message rather than a tiny scrollbar',()=>{
 const src=source(),a=src.indexOf('function historyHasMessage(tl)'),b=src.indexOf('function historyMessageKey(',a);
 let rows=[];const tl={scrollHeight:705,clientHeight:700,getBoundingClientRect:()=>({top:0,bottom:700}),querySelectorAll:()=>rows};
 const has=vm.runInNewContext('('+src.slice(a,b)+')');assert.equal(has(tl),false);
 const row=(id,text,top=20)=>({dataset:{id},textContent:text,getBoundingClientRect:()=>({height:30,top,bottom:top+30})});
 rows=[row('@live:t','Working'),row('empty','  '),row('offscreen','committed',900)];assert.equal(has(tl),false);
 rows.push(row('committed-message','Visible message'));assert.equal(has(tl),true);
});
test('a tool-only prefix beyond the fill budget keeps an accessible bounded continuation',()=>{
 const src=source(),a=src.indexOf('function historyFillMore('),b=src.indexOf('function laterCheck(tl)',a),nodes=new Map(),requests=[];
 let scheduled=0;const tl={id:'timeline',scrollHeight:700,clientHeight:700,scrollTop:0,getBoundingClientRect:()=>({top:30,left:200})};
 const document={getElementById:id=>nodes.get(id),createElement:()=>({style:{},attrs:{},setAttribute(k,v){this.attrs[k]=v;},remove(){nodes.delete(this.id);}}),body:{appendChild:n=>nodes.set(n.id,n)}};
 const context=vm.createContext({document,historyDirection:'',historyFillScope:'scope',historyFillOwner:()=> 'scope',historyFillSeeking:true,historySeekMessage:()=>false,historyFillPages:32,earlierAsked:false,historyHasMessage:t=>t.scrollHeight>t.clientHeight+10,thin:{ready:true,pageError:null,paging:null,hasAhead:()=>true,ahead:(direction,options)=>{requests.push({direction,options});return true;}},historyGap:()=>0,historyRunway:()=>1400,historyCheckSoon(){scheduled++;}});
 vm.runInContext(src.slice(a,b),context);context.earlierCheck(tl);
 const button=nodes.get('history-fill-more');assert(button);assert.equal(button.textContent,'Earlier messages');assert.equal(button.attrs['aria-controls'],'timeline');assert.equal(requests.length,0);
 button.onclick();assert.equal(scheduled,1);assert.equal(context.historyFillPages,0);assert.equal(context.historyDirection,'');
 for(let n=0;n<20;n++){context.earlierAsked=false;context.earlierCheck(tl);}
 assert.equal(requests.length,16,'one explicit continuation permits at most32 raw pages');assert(nodes.has('history-fill-more'),'another continuation remains after another tool-only batch');
 tl.scrollHeight=1100;context.earlierCheck(tl);assert(!nodes.has('history-fill-more'),'remove the continuation when ordinary scrolling becomes usable');
});
test('initial context seeking uses a loaded committed key and respects restored reading intent',()=>{
 const src=source(),a=src.indexOf('function earlierCheck(tl)'),b=src.indexOf('function laterCheck(tl)',a);let seeks=0,pages=0,visible=false;
 const tl={id:'timeline',scrollTop:5000,scrollHeight:6000,clientHeight:700};
 const ctx={historyDirection:'',historyFillScope:'scope',historyFillOwner:()=> 'scope',historyFillSeeking:true,historyFillPages:4,earlierAsked:false,historyFillMore(){},historyHasMessage:()=>visible,historySeekMessage(){seeks++;return true;},thin:{ready:true,pageError:null,paging:null,hasAhead:()=>true,ahead(){pages++;return true;}},historyGap:()=>0,historyRunway:()=>1400};
 vm.createContext(ctx);const check=vm.runInContext('('+src.slice(a,b)+')',ctx);check(tl);assert.equal(seeks,1);assert.equal(pages,0,'do not fetch past a message already loaded outside the mounted range');
 visible=true;check(tl);assert.equal(ctx.historyFillSeeking,false);assert.equal(pages,0);
 visible=false;check(tl);assert.equal(seeks,1,'a restored/established reading intent is not moved by background fill');assert.equal(pages,0);
});
test('message-key lookup reads opaque metadata without constructing hidden bodies',()=>{
 const src=source(),a=src.indexOf('function historyMessageKey(node)'),b=src.indexOf('function historySeekMessage(tl)',a);
 const key=vm.runInNewContext('('+src.slice(a,b)+')',{each:x=>x||[],historyRow:n=>n.key});
 const memo=(id,fn,text)=>({tag:'bp-memo',key:id,attrs:[{name:'fn',value:fn}],kids:[{text}]});
 assert.equal(key(memo('committed','msg','Visible text')),'committed');assert.equal(key(memo('tool','act','private body')),'');assert.equal(key(memo('@live:t','msg','Working')),'');assert.equal(key(memo('empty','msg','  ')),'');
});
test('context-seek ownership changes with account, selection, view generation and owning history epoch',()=>{
 const src=source(),a=src.indexOf('function historyFillOwner()'),b=src.indexOf('function historyHasMessage(tl)',a);
 let account='one';const link={viewId:'v',greeting:{generation:'1',epoch:'hub'}};
 const thin={ready:true,hub:'hub-url',client:{links:new Map([['hub-url',link]])},selectionRevision:1,target:{kind:'thread',id:'peer~t'},session:{sel:'peer~t'},historyEpoch:'owner-basis'};
 const owner=vm.runInNewContext('('+src.slice(a,b)+')',{thin,readingIdentity:()=>account});
 let prior=owner();for(const change of [()=>account='two',()=>thin.selectionRevision++,()=>link.greeting.generation='2',()=>thin.historyEpoch='replacement',()=>thin.session.sel='peer~other']){change();const next=owner();assert.notEqual(next,prior);prior=next;}
});
test('new-output read mark is a source identity even when the visible tail is a folded tool run',()=>{
 const src=source(),a=src.indexOf('function holdMark()'),b=src.indexOf('function holdBottom()',a);
 const mark=vm.runInNewContext('('+src.slice(a,b)+')',{timelineList:()=>({el:{getAttribute:()=> 'latest-source-call'},rows:[{id:'fold:oldest-call'}]}),historyRow:r=>r.id});
 assert.equal(mark(),'latest-source-call');
});
test('reaching the old viewport bottom cannot clear unread while the newest page is still travelling',()=>{
 const src=source(),a=src.indexOf('function timelineChanged(list)'),b=src.indexOf('function holdMark()',a),said=[];
 const list={atEnd:()=>true,el:{}},thin={atNewest:()=>true,paging:{direction:'newest'}};
 const changed=vm.runInNewContext('('+src.slice(a,b)+')',{transitionPosition:null,canonicalBoot:false,activeTimeline:list,focusedNode:null,frontendNode:{},thin,held:false,
  App:{sel:()=> 'chat'},ui:{},holdSay:s=>said.push(s),historyCheckSoon(){},bodyWindows:{observe(){}},remoteReady:true});
 changed(list);assert.deepEqual(said,['chat']);thin.paging=null;changed(list);assert.deepEqual(said,['chat','']);
});
test('a cached reconnect picture cannot report reading progress or consume unread content',()=>{
 const src=source(),a=src.indexOf('function timelineChanged(list)'),b=src.indexOf('function holdMark()',a),said=[];
 const list={atEnd:()=>true,el:{}};
 const changed=vm.runInNewContext('('+src.slice(a,b)+')',{transitionPosition:null,canonicalBoot:false,activeTimeline:list,focusedNode:null,frontendNode:null,thin:{atNewest:()=>true},held:false,
  App:{sel:()=> 'chat'},ui:{},holdSay:s=>said.push(s),historyCheckSoon(){},bodyWindows:{observe(){}},remoteReady:false});
 changed(list);assert.deepEqual(said,[]);
});
test('the temporary bottom drawn before restoring a bookmark cannot clear its unread boundary',()=>{
 const src=source(),a=src.indexOf('function timelineChanged(list)'),b=src.indexOf('function holdMark()',a),said=[];
 let bottom=true;const list={atEnd:()=>bottom,el:{}},context=vm.createContext({transitionPosition:{anchor:'saved',atEnd:false},canonicalBoot:false,
  activeTimeline:list,focusedNode:null,frontendNode:{},thin:{atNewest:()=>true},held:false,App:{sel:()=> 'chat'},ui:{},
  holdSay:s=>said.push(s),historyCheckSoon(){},bodyWindows:{observe(){}},remoteReady:true});
 const changed=vm.runInContext('('+src.slice(a,b)+')',context);changed(list);assert.deepEqual(said,[]);
 bottom=false;context.transitionPosition=null;changed(list);assert.deepEqual(said,['chat']);
});
test('actual scrollbar movement updates paging direction without requiring a wheel, key or touch',()=>{
 const src=source(),a=src.indexOf('function timelineScrolled(list, delta)'),b=src.indexOf('function timelineChanged(list)',a),list={};
 const context=vm.createContext({activeTimeline:list,historyDirection:'older',historyFillMore(){}});
 const scrolled=vm.runInContext('('+src.slice(a,b)+')',context);
 scrolled(list,25);assert.equal(context.historyDirection,'newer');scrolled(list,-10);assert.equal(context.historyDirection,'older');
 scrolled({},30);assert.equal(context.historyDirection,'older');
});
test('rejected current inline reader response produces visible feedback instead of a dead button',()=>{
 const src=source(),a=src.indexOf('function showThinMessage(message)'),b=src.indexOf('function stopThin(',a),warnings=[];let paints=0;
 const show=vm.runInNewContext('('+src.slice(a,b)+')',{viewMode:'view-v1',thinContinuations:{accept:()=>false},showViewStatus:s=>warnings.push(s),later(){paints++;}});
 show({id:'report',version:'wrong',text:'stale'});assert.equal(paints,0);assert.equal(warnings.length,1);assert.match(warnings[0],/continuation.*unavailable/i);
});
test('actual ThinHost warning hook forwards rejected chunk/transport feedback to the existing status UI',()=>{
 const src=source(),start=src.indexOf('thin = new ThinHost('),tail=src.slice(start),line=/^\s*warning:\s*(.+),$/m.exec(tail);assert(line,'Current ThinHost must wire its existing warning callback');
 const warnings=[],warning=vm.runInNewContext('('+line[1]+')',{showViewStatus:s=>warnings.push(s)});warning('Message continuation is unavailable; reopen it explicitly');assert.equal(warnings.length,1);
});
test('accepted inline reader clears its stale feedback and renders without a modal or extra request',()=>{
 const src=source(),a=src.indexOf('function showThinMessage(message)'),b=src.indexOf('function stopThin(',a),warnings=[];let paints=0;
 const show=vm.runInNewContext('('+src.slice(a,b)+')',{viewMode:'view-v1',thinContinuations:{accept:()=>true},showViewStatus:s=>warnings.push(s),later(){paints++;}});
 show({id:'report',version:'1',text:'accepted'});assert.equal(paints,1);assert.deepEqual(warnings,['']);
});

test('reconnect history is one-use and scoped to account, hub, epoch, target and source conversation',()=>{
 const src=source(),a=src.indexOf('function takeReadingHistory('),b=src.indexOf('// (the re-render harness',a);
 for(const mismatch of ['', 'identity','hub','epoch','target','conversation']){
  const saved={target:'thread:A',conversation:'A',node:{privateHistory:true}},record={identity:'account',state:{hub:'home',epoch:'1'},history:saved};
  const thin={hub:'home',epoch:'1'},session={sel:'A'},target={id:'A'};
  if(mismatch==='identity')record.identity='other';if(mismatch==='hub')thin.hub='other';if(mismatch==='epoch')thin.epoch='2';
  if(mismatch==='target')target.id='B';if(mismatch==='conversation')session.sel='B';
  const take=vm.runInNewContext('('+src.slice(a,b)+')',{thinReading:record,readingIdentity:()=> 'account',thin,targetKey:t=>'thread:'+t.id});
  assert.equal(take(target,session),mismatch?null:saved,mismatch||'same scope retained');
  assert.equal(take(target,session),null,'a later selection never borrows this reconnect history');
 }
});

test('a matching retained reconnect history keeps the current viewport instead of an old transition picture',()=>{
 const src=source(),a=src.indexOf('selection: (target, session) => {'),b=src.indexOf('\n      local:',a);
 const callback=src.slice(a+'selection: '.length,b).trim().replace(/,$/,'');
 for(const retained of [true,false]){
  const saved=retained?{node:{history:true},direction:'older',conversation:'A',gone:{older:['a'],newer:[]},held:{pages:[{ids:['a']}]}}:null;
  let transitions=0;const ctx={composerKeeper:{selecting(){}},takeReadingHistory:()=>saved,historyClear(){},historyNode:null,historyDirection:'',resumedConversation:null,historyGone:{older:new Set(),newer:new Set()},
   modelFeedback:{clear(){}},subsFeedback:{clear(){}},thinMailDraft:{clear(){}},thinContinuations:{clear(){}},bodyWindows:{disconnect(){}},App:{presentation_select:u=>u},ui:{},restoreFocused(){},restoreTransition(){transitions++;},transitionPosition:{atEnd:true}};
  vm.createContext(ctx);const select=vm.runInContext('('+callback+')',ctx);assert.equal(select({kind:'thread',id:'A'},{sel:'A'}),saved?.held);
  assert.equal(transitions,retained?0:1);if(retained){assert.equal(ctx.transitionPosition,null);assert.equal(ctx.historyNode,saved.node);assert.equal(ctx.resumedConversation,'A');}
 }
});

test('owning-history reset drops retained DOM rows before normal history projection',()=>{
 const src=source(),from=src.indexOf('view: (node, detail) => {',src.indexOf('thin = new ThinHost(')),to=src.indexOf('        const p0 = RR',from);
 assert(from>0&&to>from);const body=src.slice(from+'view: '.length,to)+'return {previous,projection};}';
 const clear=src.slice(src.indexOf('function historyClear()'),src.indexOf('// Rows let go past the cap'));
 for(const reset of [true,false]){
  let resets=0,removed=0;const old={oldHistory:true},fresh={freshHistory:true};
  const ctx={RR:null,current:()=>true,pnow:()=>0,scroll:false,historyNode:old,resumedConversation:'A',currentSession:()=>({sel:'A'}),transitionPosition:null,
   historyDirection:'older',document:{getElementById:id=>id==='history-fill-more'?{remove(){removed++;}}:null},historyPageState(){},historyGone:{older:new Set(['old']),newer:new Set()},thin:{heldReset(){resets++;}}};
  vm.createContext(ctx);vm.runInContext(clear,ctx);const view=vm.runInContext('('+body+')',ctx),got=view(fresh,{historyReset:reset,epoch:'local',target:{kind:'thread',id:'peer~A'}});
  assert.equal(got.projection,reset?'reset':'update');assert.equal(got.previous,reset?fresh:old);assert.equal(resets,reset?1:0);assert.equal(removed,reset?1:0);
  if(reset){assert.equal(ctx.historyGone.older.size,0);assert.equal(ctx.transitionPosition.atEnd,false);assert.equal(ctx.transitionPosition.top,0);}
 }
});
