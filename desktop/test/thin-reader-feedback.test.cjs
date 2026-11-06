'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const idleEntry={active:()=>false,cancel(){}};
const source=()=>fs.readFileSync(process.env.READER_HOST_SOURCE||path.join(__dirname,'../../src/web/host.js'),'utf8');
test('entry fill waits for painted geometry and reconciles newest only after the merged return',async()=>{
 const {EntryHistory}=await import('../../src/web/entry-history.js'),entryHistory=new EntryHistory(),src=source(),
  check=src.slice(src.indexOf('function earlierCheck(tl)'),src.indexOf('function laterCheck(tl)')),
  changed=src.slice(src.indexOf('function timelineChanged(list)'),src.indexOf('function holdMark()'));
 const tl={id:'timeline',scrollHeight:700,clientHeight:700,scrollTop:0},sent=[],said=[];
 let newest=true;const list={el:tl,atEnd:()=>true},thin={ready:true,pageError:null,paging:null,pagingNext:null,
  pendingNewest:false,pageActionPending:()=>false,heldHere:()=>({pages:[{older:'issued'}]}),hasAhead:()=>true,
  atNewest:()=>newest,ahead:direction=>{sent.push(direction);newest=direction==='newer';thin.paging={direction};return true;}};
 const ctx=vm.createContext({entryHistory,historyFillOwner:()=> 'scope',thin,tl,activeTimeline:list,canonicalBoot:false,
  remoteReady:true,frontendNode:{},focusedNode:null,transitionPosition:null,held:false,historyDirection:'',historyOlderDemand:0,
  historyFillScope:'scope',earlierAsked:false,historyFillMore(){},historyHasMessage:()=>true,
  historyGap:()=>0,historyRunway:()=>1400,timelineList:()=>list,App:{sel:()=> 'A'},ui:{},
  holdSay:s=>said.push(s),historyCheckSoon(){},bodyWindows:{observe(){}}});
 vm.runInContext(check+changed,ctx);entryHistory.arm('scope');
 vm.runInContext('timelineChanged(activeTimeline)',ctx);assert.deepEqual(sent,[],'painting does not synchronously request pages');assert.deepEqual(said,[]);
 vm.runInContext('earlierCheck(tl)',ctx);assert.deepEqual(sent,['older']);
 thin.paging=null;tl.scrollHeight=1100;vm.runInContext('earlierCheck(tl)',ctx);assert.deepEqual(sent,['older','newer']);assert.deepEqual(said,[]);
 thin.paging=null;vm.runInContext('earlierCheck(tl)',ctx);assert.deepEqual(said,[''],'accepted newest reconciles bottom even without a DOM scroll event');
});
test('new-output click on the current source page retires upward paging before the next frame',()=>{
 const src=source(),bottom=src.slice(src.indexOf('function holdBottom()'),src.indexOf('// a <details>')),
  earlier=src.slice(src.indexOf('function earlierCheck(tl)'),src.indexOf('function laterCheck(tl)'));
 for(const route of ['local','older','travelling']){
  const requests=[],frames=[],tl={id:'timeline',scrollHeight:900,clientHeight:700,scrollTop:0};let scrolled=0;
  const thin={ready:true,pageError:null,paging:route==='travelling'?{direction:'older'}:null,atNewest:()=>route!=='older',
   newest:()=>requests.push('newest'),hasAhead:()=>true,ahead:()=>{requests.push('older');return true;}};
  const list={scrollToEnd(){scrolled++;tl.scrollTop=200;}},ctx=vm.createContext({entryHistory:idleEntry,thin,timelineList:()=>list,
   timelineChanged:()=>frames.push(()=>vm.runInContext('earlierCheck(tl)',ctx)),tl,
   historyDirection:'older',historyOlderDemand:2,historyFillScope:'scope',historyFillOwner:()=> 'scope',
   earlierAsked:false,historyFillMore(){},historyHasMessage:()=>true,historyGap:()=>0,historyRunway:()=>1400});
  vm.runInContext(earlier+bottom,ctx);vm.runInContext('holdBottom()',ctx);frames.forEach(f=>f());
  assert.deepEqual(requests,route==='local'?[]:['newest'],'returning to output must not revive an earlier scroll gesture');
  assert.equal(scrolled,route==='local'?1:0,'an older picture is never acknowledged as newest');
 }
});
test('renders and live tool-only pictures cannot rearm exhausted initial or gesture demand',()=>{
 const src=source(),a=src.indexOf('function earlierCheck(tl)'),b=src.indexOf('function laterCheck(tl)',a),requests=[];
 const tl={id:'timeline',scrollHeight:700,clientHeight:700,scrollTop:0};
 const context=vm.createContext({entryHistory:idleEntry,historyDirection:'',historyFillScope:'scope',historyFillOwner:()=> 'scope',historyOlderDemand:0,earlierAsked:false,historyFillMore(){},historyHasMessage:()=>false,thin:{ready:true,pageError:null,paging:null,hasAhead:()=>true,ahead:(direction,options)=>{requests.push({direction,options});return true;}},historyGap:()=>0,historyRunway:()=>1400});
 const check=vm.runInContext('('+src.slice(a,b)+')',context);for(let i=0;i<100;i++)check(tl);assert.equal(requests.length,0,'a long folded prefix cannot cause an automatic scan');
 context.historyDirection='older';context.historyOlderDemand=2;
 for(let i=0;i<100;i++){context.earlierAsked=false;check(tl);}assert.equal(requests.length,2,'one user gesture permits at most two pages, not an entire history');assert.equal(context.historyOlderDemand,0);
 context.historyOlderDemand=2;context.historyFillOwner=()=> 'new-account-or-history';check(tl);assert.equal(context.historyOlderDemand,0);assert.equal(requests.length,2,'retire demand at an ownership boundary');
});
test('a hidden timeline offers no Load earlier control over the screen above it',()=>{
 const src=source(),a=src.indexOf('function earlierCheck(tl)'),b=src.indexOf('function laterCheck(tl)',a),shown=[];
 const context=vm.createContext({entryHistory:idleEntry,historyDirection:'',historyFillScope:'scope',historyFillOwner:()=> 'scope',historyOlderDemand:0,earlierAsked:false,historyFillMore(_tl,on){shown.push(on);},historyHasMessage:()=>false,thin:{ready:true,pageError:null,paging:null,hasAhead:()=>true,ahead:()=>true},historyGap:()=>0,historyRunway:()=>1400});
 const check=vm.runInContext('('+src.slice(a,b)+')',context);
 check({id:'timeline',scrollHeight:0,clientHeight:0,scrollTop:0});check({id:'timeline',scrollHeight:300,clientHeight:300,scrollTop:0});
 assert.deepEqual(shown,[false,true]);
});
test('usable history means an actually visible committed message rather than a tiny scrollbar',()=>{
 const src=source(),a=src.indexOf('function historyHasMessage(tl)'),b=src.indexOf('// A short tool-only page',a);
 let rows=[];const tl={scrollHeight:705,clientHeight:700,getBoundingClientRect:()=>({top:0,bottom:700}),querySelectorAll:()=>rows};
 const has=vm.runInNewContext('('+src.slice(a,b)+')');assert.equal(has(tl),false);
 const row=(id,text,top=20)=>({dataset:{id},textContent:text,getBoundingClientRect:()=>({height:30,top,bottom:top+30})});
 rows=[row('@live:t','Working'),row('empty','  '),row('offscreen','committed',900)];assert.equal(has(tl),false);
 rows.push(row('committed-message','Visible message'));assert.equal(has(tl),true);
});
test('a tool-only recent window offers an explicit scoped earlier control',()=>{
 const src=source(),a=src.indexOf('function historyFillMore('),b=src.indexOf('function earlierCheck(tl)',a),nodes=new Map();let scheduled=0,owner='scope';
 const document={getElementById:id=>nodes.get(id),createElement:()=>({style:{},attrs:{},setAttribute(k,v){this.attrs[k]=v;},remove(){nodes.delete(this.id);}}),body:{appendChild:n=>nodes.set(n.id,n)}};
 const context=vm.createContext({document,historyFillScope:'scope',historyFillOwner:()=>owner,historyDirection:'',historyOlderDemand:0,earlierAsked:true,thin:{ready:true,paging:null,pageError:null,hasAhead:()=>true},historyCheckSoon(){scheduled++;}});
 const show=vm.runInContext('('+src.slice(a,b)+')',context);show({getBoundingClientRect:()=>({top:10,left:100})},true);const button=nodes.get('history-fill-more');assert.equal(button.textContent,'Load earlier messages');assert.equal(button.attrs['aria-controls'],'timeline');assert.equal(scheduled,0);button.onclick();assert.equal(context.historyOlderDemand,2);assert.equal(scheduled,1);owner='other';button.onclick();assert.equal(scheduled,1,'a stale control cannot page another conversation');show(null,false);assert.equal(nodes.size,0);
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
 const changed=vm.runInNewContext('('+src.slice(a,b)+')',{entryHistory:idleEntry,historyFillOwner:()=> "scope",transitionPosition:null,canonicalBoot:false,activeTimeline:list,focusedNode:null,frontendNode:{},thin,held:false,
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
 let bottom=true;const list={atEnd:()=>bottom,el:{}},context=vm.createContext({entryHistory:idleEntry,historyFillOwner:()=> "scope",transitionPosition:{anchor:'saved',atEnd:false},canonicalBoot:false,
  activeTimeline:list,focusedNode:null,frontendNode:{},thin:{atNewest:()=>true},held:false,App:{sel:()=> 'chat'},ui:{},
  holdSay:s=>said.push(s),historyCheckSoon(){},bodyWindows:{observe(){}},remoteReady:true});
 const changed=vm.runInContext('('+src.slice(a,b)+')',context);changed(list);assert.deepEqual(said,[]);
 bottom=false;context.transitionPosition=null;changed(list);assert.deepEqual(said,['chat']);
});
test('actual scrollbar movement updates paging direction without requiring a wheel, key or touch',()=>{
 const src=source(),a=src.indexOf('function timelineScrolled(list, delta)'),b=src.indexOf('function timelineChanged(list)',a),list={};
 const context=vm.createContext({entryHistory:idleEntry,activeTimeline:list,historyDirection:'older',historyFillMore(){}});
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
  let transitions=0;const ctx={transitionScope:()=>({}),thin:null,remoteReady:true,targetKey:()=>'',transitionViews:{warm:()=>Promise.resolve(false)},composerKeeper:{selecting(){}},takeReadingHistory:()=>saved,historyClear(){},historyNode:null,historyDirection:'',resumedConversation:null,historyGone:{older:new Set(),newer:new Set()},
   modelFeedback:{clear(){}},subsFeedback:{clear(){}},pressFeedback:{drop(){},seen(){},showing:false,project:p=>p,choice(){}},foldFeedback:{unseen:()=>[],drop(){},seen(){},showing:false,project:p=>p,choice(){}},thinMailDraft:{clear(){}},thinContinuations:{clear(){}},bodyWindows:{disconnect(){}},App:{presentation_select:u=>u},ui:{},restoreFocused(){},restoreTransition(){transitions++;},transitionPosition:{atEnd:true}};
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
  const ctx={entryHistory:idleEntry,RR:null,current:()=>true,pnow:()=>0,scroll:false,historyNode:old,resumedConversation:'A',currentSession:()=>({sel:'A'}),transitionPosition:null,
   historyDirection:'older',startJump:null,historyFillMore(){removed++;},historyPageState(){},historyGone:{older:new Set(['old']),newer:new Set()},thin:{heldReset(){resets++;}}};
  vm.createContext(ctx);vm.runInContext(clear,ctx);const view=vm.runInContext('('+body+')',ctx),got=view(fresh,{historyReset:reset,epoch:'local',target:{kind:'thread',id:'peer~A'}});
  assert.equal(got.projection,reset?'reset':'update');assert.equal(got.previous,reset?fresh:old);assert.equal(resets,reset?1:0);assert.equal(removed,reset?1:0);
  if(reset){assert.equal(ctx.historyGone.older.size,0);assert.equal(ctx.transitionPosition.atEnd,false);assert.equal(ctx.transitionPosition.top,0);}
 }
});

test('an oversized picture never closes the live socket or starts reconnect backoff',()=>{
 const src=source(),a=src.indexOf('  const viewRecover = reason => {'),b=src.indexOf('\n  try {',a);assert(a>0&&b>a);
 let shown=0;const recover=vm.runInNewContext('('+src.slice(a+'  const viewRecover = '.length,b).trim().replace(/;$/,'')+')',{previewViews:{enqueueFolds(){},stop(){}},current:()=>true,showViewStatus(message){assert.equal(message,'');},thin:{recover(){shown++;}},assert,downNote:'',Date:{now(){throw Error('must not enter reconnect/backoff');}},disconnect(){throw Error('socket must stay live');},s:{close(){throw Error('socket must stay live');}}});for(const reason of ['LIMIT','invalid view presentation','invalid view delta','View request timed out; actions were not resent'])recover(reason);assert.equal(shown,4);
});

test('bottom reconciles a paging-owned unread boundary even when host away is already empty',()=>{
 const src=source(),a=src.indexOf('function holdSay(now)'),b=src.indexOf('function timelineScrolled(',a),jobs=[],sent=[];
 let mark='before-six-mailroom-posts';
 const thin={ready:true,target:{kind:'bot',id:'bub'},awayOf:()=>mark};
 const ctx=vm.createContext({focusedNode:null,frontendNode:{},away:'',holdPending:null,thin,App:{sel:()=> 'chat'},ui:{},queueMicrotask:f=>jobs.push(f),
  dispatch:(action,thread)=>{sent.push([action,thread]);if(action==='hold-end'&&thread==='chat')mark='';}});
 const say=vm.runInContext('('+src.slice(a,b)+')',ctx);say('');jobs.splice(0).forEach(f=>f());
 assert.equal(mark,'','actual bottom must clear a controller boundary that was established by paging');
 assert.deepEqual(sent,[['hold-end','chat']]);
});

test('queued reading progress is retired by bot navigation and cannot consume another target',()=>{
 const src=source(),a=src.indexOf('function holdSay(now)'),b=src.indexOf('function timelineScrolled(',a),jobs=[],sent=[];
 const thin={ready:true,target:{kind:'bot',id:'bub'},awayOf:()=> 'old-mark'};
 const ctx=vm.createContext({focusedNode:null,frontendNode:{},away:'chat',holdPending:null,thin,App:{sel:()=> 'chat'},ui:{},queueMicrotask:f=>jobs.push(f),dispatch:(...args)=>sent.push(args)});
 const say=vm.runInContext('('+src.slice(a,b)+')',ctx);say('');thin.target={kind:'bot',id:'other'};jobs.splice(0).forEach(f=>f());
 assert.deepEqual(sent,[],'same source conversation alone cannot authorize a later bot target');
});

test('a pill-only bot/mailroom refresh reconciles reading even when virtual rows and geometry stay unchanged',()=>{
 const src=source(),a=src.indexOf('  const tl2 = document.getElementById("timeline");'),b=src.indexOf('  const main = viewMode',a);assert(a>0&&b>a);
 const seen=[],list={refresh(){},scrollToEnd(){}};
 vm.runInNewContext(src.slice(a,b),{document:{getElementById:()=>({})},timelineList:()=>list,activeTimeline:list,transitionPosition:null,scroll:false,timelineChanged:l=>seen.push(l)});
 assert.deepEqual(seen,[list],'accepted picture must reconcile bottom when only floating new-message navigation changed');
});

test('presenting newest output retires the earlier gesture and its blank history gaps',()=>{
 const src=source(),from=src.indexOf('view: (node, detail) => {',src.indexOf('thin = new ThinHost(')),to=src.indexOf('        terminalOutput.rebind();',from);
 assert(from>0&&to>from);const body=src.slice(from+'view: '.length,to)+'return projection;}';
 for(const direction of ['newest','older']){
  let resets=0,hidden=0;const ctx={entryHistory:idleEntry,current:()=>true,RR:null,resumedConversation:null,historyNode:{old:true},
   History:{project:(_old,n)=>n,project_block:(_old,n)=>n},historyDirection:'older',historyOlderDemand:2,
   historyGone:{older:new Set(['before']),newer:new Set(['after'])},thin:{heldReset(){resets++;}},
   historyBack(){},startJump:null,historyFillMore(_tl,shown){assert.equal(shown,false);hidden++;}};
  vm.createContext(ctx);const view=vm.runInContext('('+body+')',ctx),fresh={latest:true};
  assert.equal(view(fresh,{page:{direction},window:{ids:['reply']}}),direction);
  assert.equal(ctx.historyNode,fresh);
  assert.equal(ctx.historyOlderDemand,direction==='newest'?0:2,'only newest navigation consumes the old gesture');
  assert.equal(ctx.historyDirection,direction==='newest'?'':'older');
  assert.equal(ctx.historyGone.newer.size,direction==='newest'?0:1);
  assert.equal(resets,direction==='newest'?1:0);assert.equal(hidden,resets);
 }
});
