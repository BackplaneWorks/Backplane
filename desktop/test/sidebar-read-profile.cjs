// Headless exact packaged-runtime comparison; synthetic metadata only.
const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const {performance}=require('node:perf_hooks');
const {sidebarPicture}=require('../../src/web/sidebar-region.js');
const Wire=require('../../src/web/wire.js');
const cborSource=fs.readFileSync(require('node:path').join(__dirname,'../../src/core/cbor.bend'),'utf8');
const dict=name=>{const m=cborSource.match(new RegExp('def Cbor\\.'+name+'\\(\\) -> List<&2, String>:\\s*(\\[[\\s\\S]*?\\])'));assert(m,'Canonical dictionary missing');return JSON.parse(m[1]);};
const KEYS=dict('keys'),WORDS=dict('words');
const [out,beforeRaw,beforeNative,beforeCommit,afterRaw,afterNative,afterCommit]=process.argv.slice(2);assert(out&&afterCommit);
const sha=x=>crypto.createHash('sha256').update(x).digest('hex');
function engine(file,stamped){
const source=fs.readFileSync(file,'utf8'),defs=new Map();
for(const m of source.matchAll(/^function ([\w$]+)\([^\n]*\) \{[\s\S]*?^\}$/gm))defs.set(m[1],m[0]);
const root=tail=>{const a=[...defs.keys()].filter(n=>n.endsWith(tail));assert.equal(a.length,1,tail);return a[0]};
const dependent=[...defs.keys()].some(n=>n.endsWith('thin$045actor$058Actor$render$revision$'));
const names={empty:root('core$047model$058State$empty$'),apply:root('core$047model$058State$apply$'),index:root('core$047thin$058Index$empty$'),stream:root('core$047thin$045live$058Stream$blank$'),refresh:root('thin$045actor$058Actor$refresh$'),render:root(dependent?'thin$045actor$058Actor$render$revision$':'thin$045actor$058Actor$render$'),show:root('core$047json$058Json$show$'),encode:root('core$047cbor$058Cbor$encode$'),encText:root('core$047cbor$058Enc$text$'),encHead:root('core$047cbor$058Enc$head$'),output:root('thin$045sidebar$045wire$058Region$output$'),sidebar:root('web$047sidebar$045slot$058Slot$sidebar$')};
Object.assign(names,{plain:root('core$047client$058Side$ix$'),mine:root('core$047client$058SIx$mine$'),seq:root('core$047model$058State$seq$'),indexOf:root('core$047thin$058Index$of$'),page:root('core$047thin$058Index$page$')});
if(dependent)Object.assign(names,{commit:root('server$058Cfg$thin$committed$'),signalSeq:root('server$058Cfg$thin$signal_seq$'),bump:root('server$058Cfg$thin$bump$'),live:root('server$058Cfg$thin$live$'),warm:root('server$058Cfg$thin$warm$'),withNotes:root('server$058Cfg$thin$with_notes$'),mailEmpty:root('thin$045mail$058Cache$empty$'),notesEmpty:root('core$047thin$045pace$058Notes$empty$'),mapNew:root('Map$new$')});
const needed=new Set(),todo=Object.values(names);while(todo.length){const n=todo.pop();if(needed.has(n))continue;needed.add(n);for(const m of defs.get(n).matchAll(/[A-Za-z_$][\w$]*/g))if(defs.has(m[0])&&!needed.has(m[0]))todo.push(m[0]);}

const native=fs.readFileSync(stamped,'utf8'),acorn=require('/home/h/.bun/install/cache/acorn@8.16.0@@@1/dist/acorn.js');
const ast=acorn.parse(native,{ecmaVersion:'latest',sourceType:'module'}),nativeDefs=new Map(),nativeNodes=new Map();
// Bun packages the compiled module inside a module factory. Locate its
// lexical block without evaluating that factory or any IO initializer.
let moduleScope=null;function locate(n,parent){if(!n||typeof n!=='object')return;if(n.type==='FunctionDeclaration'&&n.id?.name===names.render)moduleScope=parent;for(const [k,v] of Object.entries(n)){if(k==='start'||k==='end')continue;if(Array.isArray(v)){for(const c of v)locate(c,n)}else if(v&&typeof v==='object')locate(v,n)}}locate(ast,null);assert(moduleScope?.type==='BlockStatement','Compiled module lexical scope missing');
const declarations=[...ast.body,...moduleScope.body];
for(const node of declarations)if(node.type==='FunctionDeclaration'){nativeDefs.set(node.id.name,native.slice(node.start,node.end));nativeNodes.set(node.id.name,node);}
// Inspection-only helpers were tree-shaken from the package; they run outside the measured renderer.
const inspectionOnly=[root('web$047sidebar$045slot$058Slot$sidebar$'),root('web$047sidebar$045slot$058Slot$one$')];for(const n of inspectionOnly)if(!nativeDefs.has(n)){nativeDefs.set(n,defs.get(n));nativeNodes.set(n,acorn.parse(defs.get(n),{ecmaVersion:'latest'}).body[0]);}
// Follow actual identifier references, never names occurring in strings or
// property keys. Textual matching can accidentally pull in an IO factory.
function references(node){const found=new Set();function walk(n,parent,key){if(!n||typeof n!=='object')return;if(n.type==='Literal')return;if(n.type==='Identifier'){
 if(!((parent?.type==='MemberExpression'&&key==='property'&&!parent.computed)||(parent?.type==='Property'&&key==='key'&&!parent.computed)))found.add(n.name);return;
}for(const [k,v]of Object.entries(n)){if(k==='start'||k==='end')continue;if(Array.isArray(v)){for(const c of v)walk(c,n,k)}else if(v&&typeof v==='object')walk(v,n,k)}}walk(node,null,'');return found}
const nativeNeeded=new Set(),nativeTodo=Object.values(names);while(nativeTodo.length){const n=nativeTodo.pop();if(nativeNeeded.has(n))continue;assert(nativeDefs.has(n),'Packaged function missing: '+n);nativeNeeded.add(n);for(const name of references(nativeNodes.get(n)))if(nativeDefs.has(name)&&!nativeNeeded.has(name))nativeTodo.push(name);}
// Only referenced pure module constants are admitted. IO initializers never run.
const code=[...nativeNeeded].map(n=>nativeDefs.get(n)).join('\n'),tokens=new Set([...nativeNeeded].flatMap(n=>[...references(nativeNodes.get(n))]));let constants='';
for(const node of declarations)if(node.type==='VariableDeclaration')for(const d of node.declarations)if(d.id.type==='Identifier'&&tokens.has(d.id.name)){
 assert(d.id.name==='__bpParseMemo'||(d.id.name==='f32_round'&&d.init?.type==='FunctionExpression')||(/^TAB_[0-9]+$/.test(d.id.name)&&d.init?.type==='ArrayExpression'&&d.init.elements.every(x=>x?.type==='Literal'&&typeof x.value==='number'))||(d.init&&['Literal','UnaryExpression','BinaryExpression'].includes(d.init.type)),'Unexpected non-pure referenced constant: '+d.id.name);
 constants+='const '+native.slice(d.start,d.end)+';\n';
}
assert((nativeNeeded.has('__bpBit')||(nativeNeeded.has('__bpMapBit')&&nativeNeeded.has('__bpMapRead')))&&nativeNeeded.has('__bpOrd')&&nativeNeeded.has('__bpParsed'),'Real packaged native helpers required');
const ctx=vm.createContext({});vm.runInContext(constants+code+'\nglobalThis.f={'+Object.entries(names).map(([k,n])=>k+':'+n).join(',')+'};',ctx,{timeout:10000});

vm.runInContext('globalThis.activity={index:'+root('core$047signal$058Signal$index$')+',at:'+root('core$047signal$058Signal$at$')+'};',ctx);
let count=0,workCount=0,atCount=0,plainCount=0,frameCount=0,frameMs=0;
const frame=root('web$047view$058View$sidebar$frame$');ctx.frameStart=()=>{frameCount++;return performance.now()};ctx.frameEnd=t=>{frameMs+=performance.now()-t};vm.runInContext(frame+'=((old)=>(...args)=>{const t=frameStart();try{return old(...args)}finally{frameEnd(t)}})('+frame+');',ctx);
const plain=root('core$047client$058Side$ix$');ctx.recordPlain=()=>{plainCount++};vm.runInContext(plain+'=((old)=>(...args)=>{recordPlain();return old(...args)})('+plain+');',ctx);
const n=root('core$047signal$058Signal$index$');ctx.recordSignal=()=>{count++};
vm.runInContext(n+'=((old)=>(...args)=>{recordSignal();return old(...args)})('+n+');',ctx);
const w=root('core$047signal$058Signal$cache$work$');ctx.recordWork=()=>{workCount++};vm.runInContext(w+'=((old)=>(...args)=>{recordWork();return old(...args)})('+w+');',ctx);
const at=root('core$047signal$058Signal$at$');ctx.recordAt=()=>{atCount++};vm.runInContext(at+'=((old)=>(...args)=>{recordAt();return old(...args)})('+at+');',ctx);
return {metadataKey:[...defs.keys()].some(n=>n.endsWith('thin$045sidebar$058Sidebar$key$revision$')),frameCount:()=>frameCount,frameMs:()=>frameMs,dependent,ctx,names,root,nativeNeeded,rawSha256:sha(source),nativeSha256:sha(native),plainCount:()=>plainCount,count:()=>count,atCount:()=>atCount,workCount:()=>workCount,reset:()=>{count=0;workCount=0;atCount=0;plainCount=0;frameCount=0;frameMs=0}};
}
function json(v){if(v===null)return {$:'../core/json.Null'};if(typeof v==='string')return {$:'../core/json.Str',text:v};if(typeof v==='boolean')return {$:'../core/json.Flag',value:v};if(typeof v==='number')return {$:'../core/json.Num',raw:String(v)};if(Array.isArray(v)){let items={$:'../core/json.End'};for(const x of [...v].reverse())items={$:'../core/json.Item',head:json(x),tail:items};return {$:'../core/json.Arr',items};}let fields={$:'../core/json.End'};for(const [key,value]of Object.entries(v).reverse())fields={$:'../core/json.Field',key,value:json(value),tail:fields};return {$:'../core/json.Obj',fields};}

const nil={$:'Nil'},now=1792800000,rows=[],stage={step:'setup'},integration=[];
const list=xs=>xs.reduceRight((tail,head)=>({$:'Con',head,tail}),nil);
const E=(name,fields)=>({$:'../core/model.'+name,...fields});
function bytes(xs){const a=[];for(let x=xs;x.$!=='Nil';x=x.tail)a.push(x.head);return Buffer.from(a)}
function measure(fn){const t=performance.now(),value=fn();return {value,ms:performance.now()-t}}
function makeCfg(e,st){const f=e.ctx.f;return {$:'server.Cfg',home:'/tmp/synthetic-signal-dependencies',web:'',host:'127.0.0.1',port:0,prompt:'',dirs:nil,version:'synthetic',net:null,telemetry:nil,views:list([{$:'server.ThinCache',index:{$:'../core/thin.Built',index:f.indexOf(st)},epoch:'synthetic',revision:1,streams:f.mapNew(),mail:{$:'thin-mail.Built',cache:f.mailEmpty()},waiting:nil,notes:f.notesEmpty(),store:'',signal_seq:f.seq(st)}])}}
function render(e,actor,st,index,p,epoch,clock,ids,revision){return e.ctx.f.render(actor,true,st,index,json(p),epoch,clock,ids,e.ctx.f.stream(),...(e.dependent?[revision]:[]))}
function seed(e){
 const f=e.ctx.f;let st=f.empty();for(let p=0;p<45;p++)st=f.apply(st,E('ProjectCreated',{id:'p'+p,title:'Synthetic project '+p,root:'/tmp/signal-facts-'+p,at:now-3600}));
 for(let i=0;i<2786;i++){st=f.apply(st,E('ThreadCreated',{id:'t'+i,project:'p'+i%45,title:'Synthetic thread '+i,env:{$:'../core/model.Local'},branch:'',worktree:'',provider:'codex',model:'',at:now-3600}));if(i>=32)st=f.apply(st,E('ThreadSettled',{id:'t'+i,manual:false,at:now-3599}));}
 for(const [key,value]of [['onboard.done','1'],['sidebar.view','active']])st=f.apply(st,E('SettingSet',{key,value,at:now-3590}));
 const info=json({models:JSON.stringify({codex:['gpt-6.1-sol'],claude:['opus']})}),live=json({});
 const p={target:{kind:'thread',id:'t0'},session:{sel:'t0',rid:100,open:{},drafts:{t0:'Synthetic 日本語 IME draft'},info:{}},action:'flag',value:'settings',requestRevision:1,sidebarRegion:{version:1,token:''},signalRevision:0};
 const actor=f.refresh(nil,true,st,info,live,now,json(p),'synthetic'),cfg=e.dependent?makeCfg(e,st):null,index=f.indexOf(st),ids=f.page(index,'t0',0,32),input=sha(JSON.stringify([st,cfg,actor]));e.reset();
 const cold=measure(()=>render(e,actor,st,index,p,'synthetic',now,ids,f.seq(st)));const coldCalls=e.count(),coldPlainCalls=e.plainCount();assert.equal(coldPlainCalls,1,'Cold render derives one plain metadata index');
 assert.equal(sha(JSON.stringify([st,cfg,actor])),input,'Cold render mutated input');assert.equal(coldCalls,1,'Cold render must derive one index');
 const issued=f.output(true,list([cold.value.actor]),cold.value.result,json(p));const coldPlain=Wire.plain(Wire.cbor(bytes(f.encode(issued.result)),KEYS,WORDS));assert.deepEqual(coldPlain,JSON.parse(f.show(issued.result)),'Cold binary decode mismatch');const receiver=sidebarPicture(coldPlain,p,null);assert.equal(receiver.result.session.open.settings,true,'Seed must enter Settings by its normal Open action');const next={...p,session:receiver.result.session,action:'settings-page',value:'appearance'};return {st,info,live,p:next,cfg,issued,cold,coldCalls,coldPlainCalls,receiverCache:receiver.cache};
}
function run(e,s,scenario){
 const f=e.ctx.f,events=scenario.events||[];let st=s.st;for(const c of events)st=f.apply(st,c);
 const beforeCfg=e.dependent?sha(JSON.stringify(s.cfg)):'';
 const cfg=e.dependent?f.commit(s.cfg,list(events),st):null,revision=e.dependent?f.signalSeq(cfg):f.seq(st);
 if(e.dependent){assert.equal(sha(JSON.stringify(s.cfg)),beforeCfg,'Commit mutated previous config');assert.equal(revision,scenario.keep?f.seq(s.st):f.seq(st),'Actual committed dependency revision wrong');}
 const p={...s.p,session:{...s.p.session,drafts:{t0:scenario.draft||'Synthetic 日本語 IME draft'}},...(scenario.history?{action:'flag',value:'settings'}:{}),requestRevision:2,sidebarRegion:scenario.noAck?{version:1,token:''}:scenario.badAck?{version:1,token:'99'}:{version:1,token:'1'},signalRevision:scenario.forged||0};
 if(scenario.legacy)delete p.sidebarRegion;
 const clock=scenario.sameClock?now:now+1,epoch=scenario.epoch||'synthetic',prior=scenario.reconnect?nil:s.issued.actors;
 const actor=f.refresh(prior,true,st,scenario.info?json({models:JSON.stringify({codex:['gpt-6.1-sol'],claude:['opus']}),status:'fresh'}):s.info,s.live,clock,json(p),scenario.view||'synthetic'),index=f.indexOf(st),ids=f.page(index,'t0',0,32);
 if(scenario.mailHidden){const mine=f.mine(f.plain(st),'p1');let present=false;for(let xs=mine;xs.$!=='Nil';xs=xs.tail)if(xs.head.id==='t1')present=true;assert.equal(present,false,'Actual RunSet must hide the mail-run thread');}
 const rawSignals=e.ctx.activity.index(st);if(scenario.bits!==undefined)assert.equal(e.ctx.activity.at(rawSignals,'t0'),scenario.bits,'Fixture must contain actual delegated running work');const raw=JSON.stringify(rawSignals);const old=sha(JSON.stringify([st,prior,actor,cfg]));e.reset();
 const measured=measure(()=>render(e,actor,st,index,p,epoch,clock,ids,revision)),calls=e.count();assert.equal(sha(JSON.stringify([st,prior,actor,cfg])),old,'Render mutated input aliases');
 const frames=e.frameCount(),frameTime=e.frameMs();
 const wire=f.output(true,list([measured.value.actor]),measured.value.result,json(p)),text=f.show(measured.value.result),logicalEncoded=bytes(f.encode(measured.value.result));
 const enc=measure(()=>bytes(f.encode(wire.result))),encoded=enc.value,plain=Wire.plain(Wire.cbor(encoded,KEYS,WORDS));assert.deepEqual(plain,JSON.parse(f.show(wire.result)),'Exact native CBOR decoder mismatch');
 const expectRef=!!(e.metadataKey&&scenario.ref&&!scenario.noAck&&!scenario.badAck&&!scenario.legacy);
 if((plain.sidebarRegion?.base||'')!==(expectRef?'1':'')){
  const saved=a=>{for(let xs=a.memo;xs.$!=='Nil';xs=xs.tail)if(xs.head.$.endsWith('.Sidebar'))return xs.head.key;return ''};const k0=saved(s.issued.actors.head),k1=saved(measured.value.actor),a=k0?JSON.parse(k0):{},b=k1?JSON.parse(k1):{};
  stage.finding={candidate:e.metadataKey,frames,priorKeyLength:k0.length,currentKeyLength:k1.length,changedFields:[...new Set([...Object.keys(a),...Object.keys(b)])].filter(k=>JSON.stringify(a[k])!==JSON.stringify(b[k])),receivedBase:plain.sidebarRegion?.base||'',expectedReference:expectRef};
 }
 assert.equal(plain.sidebarRegion?.base||'',expectRef?'1':'','Only acknowledged same-clock history may send a reference');
 const reconstructed=sidebarPicture(plain,p,s.receiverCache).result;
 const reconstructedLogical={...reconstructed};delete reconstructedLogical.sidebarRegion;
 assert.deepEqual(reconstructedLogical,JSON.parse(text),'Actual receiver reconstructed different logical reply');
 assert.equal(frames,scenario.ref&&e.metadataKey?0:1,'Sidebar render reuse count mismatch');
 if(scenario.body)assert(text.includes(scenario.body),'New actual timeline body missing: '+scenario.name);
 let retained=0;for(let m=measured.value.actor.memo;m.$!=='Nil';m=m.tail)if(m.head.$.endsWith('.Signals'))retained++;
 assert.equal(retained,1,'Exactly one bounded actor-private signal index');
 let metadata=0;for(let m=measured.value.actor.memo;m.$!=='Nil';m=m.tail)if(m.head.$.endsWith('.SideIndex'))metadata++;const expectedMetadata=1;assert.equal(metadata,expectedMetadata,'Bounded metadata remains private and singular');
 return {text,encoded,logicalEncoded,signals:raw,sidebarFrames:frames,sidebarFrameMs:frameTime,receiverExact:true,receiverDecodedFromNativeCbor:true,sentReference:expectRef,wireEncodeMs:enc.ms,renderMs:measured.ms,plainIndexCalls:e.plainCount(),signalIndexCalls:calls,signalLookupCalls:e.atCount(),dependencySequence:revision,canonicalSequence:f.seq(st),timelineBodyPresent:scenario.body?true:undefined,replySha256:sha(text),cborSha256:sha(encoded),cborBytes:encoded.length,fullSidebarSent:!expectRef};
}
let state='failed';try{
 const before=engine(beforeRaw,beforeNative),after=engine(afterRaw,afterNative);before.commitTag=beforeCommit;after.commitTag=afterCommit;assert(before.dependent&&after.dependent&&!before.metadataKey&&after.metadataKey,'PR411 and changed metadata-key entries required');stage.step='seed';
 const seeds=[seed(before),seed(after)];assert.equal(before.ctx.f.show(seeds[0].cold.value.result),after.ctx.f.show(seeds[1].cold.value.result),'Cold complete reply mismatch');
 rows.push({scenario:'cold',identicalReply:true,beforeRenderMs:seeds[0].cold.ms,afterRenderMs:seeds[1].cold.ms,beforeSignalIndexCalls:seeds[0].coldCalls,afterSignalIndexCalls:seeds[1].coldCalls,beforePlainIndexCalls:seeds[0].coldPlainCalls,afterPlainIndexCalls:seeds[1].coldPlainCalls});
 const f=after.ctx.f,cfg=seeds[1].cfg,original=f.signalSeq(cfg),originalCfg=sha(JSON.stringify(cfg));
 for(const [name,changed]of [['view invalidation',f.bump(cfg)],['stream update',f.live(cfg,f.mapNew(),'t0')],['warm store',f.warm(cfg,f.index(),f.mailEmpty())],['pacing notes',f.withNotes(cfg,f.notesEmpty())]]){assert.equal(f.signalSeq(changed),original,name+' changed dependency revision');integration.push({name,passed:true});}
 assert.equal(sha(JSON.stringify(cfg)),originalCfg,'Config transforms mutated original');
 const scenarios=[
 {name:'clock-only',keep:true,before:0,after:0},
 {name:'warning-history',sameClock:true,ref:true,history:true,keep:true,body:'Fresh synthetic warning',events:[E('ActivityLogged',{thread:'t0',act:'warn1',tone:'warn',kind:'notice',summary:'Fresh synthetic warning',at:now+1})],before:1,after:0},
 {name:'assistant-history',sameClock:true,ref:true,history:true,keep:true,body:'Fresh synthetic assistant',events:[E('MessagePosted',{thread:'t0',msg:'assistant1',role:{$:'../core/model.Assistant'},text:'Fresh synthetic assistant',at:now+1})],before:1,after:0},
 {name:'system-history',sameClock:true,ref:true,history:true,keep:true,body:'Fresh synthetic system',events:[E('MessagePosted',{thread:'t0',msg:'system1',role:{$:'../core/model.System'},text:'Fresh synthetic system',at:now+1})],before:1,after:0},
 {name:'user-touches-settled-shelf',events:[E('MessagePosted',{thread:'t40',msg:'user1',role:{$:'../core/model.User'},text:'New synthetic request',at:now+1})],before:1,after:1},
 {name:'approval',events:[E('AskOpened',{id:'a',thread:'t0',kind:'approval',tool:'tool',detail:'Allow',options:'',at:now+1})],before:1,after:1},
 {name:'question-closed',events:[E('AskOpened',{id:'a',thread:'t0',kind:'input',tool:'tool',detail:'Choose',options:'',at:now+1}),E('AskClosed',{id:'a',answer:'yes',at:now+2})],before:1,after:1},
 {name:'pending',events:[E('PendingChanged',{thread:'t0',count:1,at:now+1})],before:1,after:1},
 {name:'owned-watch',events:[E('WorkSet',{thread:'t0',kind:'watch',rows:'w\twatching\t\tbuild\twatch',at:now+1})],before:1,after:1},
 {name:'delegated-task',bits:1,events:[E('TaskDelegated',{task:'child',parent:'t0',child:'t1',depth:1,provider:'codex',model:'',at:now+1}),E('TurnChanged',{thread:'t1',state:{$:'../core/model.Running'},at:now+1})],before:1,after:1},
 {name:'settle',events:[E('ThreadSettled',{id:'t0',manual:false,at:now+1})],before:1,after:1},
 {name:'wake',events:[E('ThreadWoke',{id:'t40',at:now+1})],before:1,after:1},
 {name:'failure',events:[E('TurnChanged',{thread:'t0',state:{$:'../core/model.Failed'},at:now+1})],before:1,after:1},
 {name:'auth-hold',events:[E('SettingSet',{key:'auth.wait.t0',value:'codex',at:now+1})],before:1,after:1},
 {name:'fallback-queue',events:[E('TurnQueued',{thread:'t0',msg:'fallback:held:1',text:'held',at:now+1})],before:1,after:1},
 {name:'unclassified-rename-fail-closed',events:[E('ThreadRenamed',{id:'t1',title:'Changed synthetic title',at:now+1})],before:1,after:1},
 {name:'mixed-history-and-approval',forged:0,events:[E('MessagePosted',{thread:'t0',msg:'assistant2',role:{$:'../core/model.Assistant'},text:'More fresh history',at:now+1}),E('AskOpened',{id:'a',thread:'t0',kind:'approval',tool:'tool',detail:'Allow',options:'',at:now+2})],before:1,after:1},
 {name:'mail-run-hidden',mailHidden:true,events:[E('SettingSet',{key:'mail.on',value:'on',at:now+1}),E('RunSet',{thread:'t1',bot:'b',conv:'conv',mail:'mail',at:now+1})],before:1,after:1},
 {name:'bot-owned-child',events:[E('TaskDelegated',{task:'botchild',parent:'t0',child:'t1',depth:1,provider:'codex',model:'',at:now+1}),E('BotSet',{id:'bot',name:'Bot',thread:'t1',look:1,persona:'',at:now+1})],before:1,after:1},
 {name:'reconnect',keep:true,reconnect:true,before:1,after:1},
 {name:'epoch-change',sameClock:true,keep:true,epoch:'new-epoch',before:1,after:1},
 {name:'view-change',sameClock:true,keep:true,view:'new-view'},
 {name:'history-clock-miss',keep:true,history:true,body:'Clock-fresh warning',events:[E('ActivityLogged',{thread:'t0',act:'warnclock',tone:'warn',kind:'notice',summary:'Clock-fresh warning',at:now+1})]},
 {name:'history-without-ack',sameClock:true,keep:true,ref:true,noAck:true,events:[E('ActivityLogged',{thread:'t0',act:'noack',tone:'warn',kind:'notice',summary:'No ack body',at:now})]},
 {name:'history-bad-ack',sameClock:true,keep:true,ref:true,badAck:true,events:[E('ActivityLogged',{thread:'t0',act:'badack',tone:'warn',kind:'notice',summary:'Bad ack body',at:now})]},
 {name:'history-legacy-client',sameClock:true,keep:true,ref:true,legacy:true,events:[E('ActivityLogged',{thread:'t0',act:'legacy',tone:'warn',kind:'notice',summary:'Legacy body',at:now})]},
 {name:'history-draft-edit',sameClock:true,keep:true,draft:'Changed 日本語 IME',events:[E('ActivityLogged',{thread:'t0',act:'draft',tone:'warn',kind:'notice',summary:'Fresh draft body',at:now})]},
 {name:'history-info-update',sameClock:true,keep:true,info:true,events:[E('ActivityLogged',{thread:'t0',act:'info',tone:'warn',kind:'notice',summary:'Fresh info body',at:now})]}
 ];
 for(const scenario of scenarios){if(process.env.SIDEBAR_READ_ONLY&&!process.env.SIDEBAR_READ_ONLY.split(',').includes(scenario.name))continue;if(process.env.SIDEBAR_READ_FROM&&scenarios.findIndex(x=>x.name===scenario.name)<scenarios.findIndex(x=>x.name===process.env.SIDEBAR_READ_FROM))continue;if(!scenario.keep)scenario.sameClock=true;if(scenario.sameClock&&scenario.events)scenario.events=scenario.events.map(c=>({...c,at:now}));stage.step=scenario.name;const results=[run(before,seeds[0],scenario),run(after,seeds[1],scenario)];
  assert.equal(results[0].signals,results[1].signals,'Complete raw signal map changed');assert.equal(results[0].text,results[1].text,'Complete reply mismatch: '+scenario.name);assert.deepEqual(results[0].logicalEncoded,results[1].logicalEncoded,'Exact logical native CBOR mismatch: '+scenario.name);if(!results[1].sentReference)assert.deepEqual(results[0].encoded,results[1].encoded,'Exact full native CBOR mismatch: '+scenario.name);
  const expectedSignal=scenario.keep&&!scenario.reconnect&&!scenario.epoch&&!scenario.view?0:1;assert.equal(results[0].signalIndexCalls,expectedSignal,'PR411 revision behavior wrong');assert.equal(results[1].signalIndexCalls,expectedSignal,'Candidate revision behavior wrong');if(!scenario.ref)assert.equal(results[0].signalLookupCalls,results[1].signalLookupCalls,'Signal decoration fresh on misses');
  assert.equal(results[0].plainIndexCalls,expectedSignal,'PR411 metadata behavior wrong');assert.equal(results[1].plainIndexCalls,expectedSignal,'Reuse only unchanged trusted metadata, never owner or epoch replacement');
  rows.push({scenario:scenario.name,threads:2786,projects:45,identicalReply:true,identicalLogicalCbor:true,identicalWireCbor:!results[1].sentReference,receiverExact:true,identicalRawSignalMap:true,before:{...results[0],text:undefined,encoded:undefined,logicalEncoded:undefined,signals:undefined},after:{...results[1],text:undefined,encoded:undefined,logicalEncoded:undefined,signals:undefined}});
  fs.writeFileSync(out+'.partial.json',JSON.stringify({state:'pending',integration,rows},null,2)+'\n');
 }
 state='passed';
}catch(e){rows.push({step:stage.step,finding:stage.finding,error:e.stack})}
const result={state,only:process.env.SIGNAL_DEPENDENCY_ONLY||null,engine:{bun:Bun.version,runtime:'Bun executing extracted exact packaged pure functions including native map/string/parse/CBOR; candidate uses production Actor.render.revision and Cfg.thin.committed'},before:{commit:beforeCommit,rawSha256:sha(fs.readFileSync(beforeRaw)),nativeSha256:sha(fs.readFileSync(beforeNative))},after:{commit:afterCommit,rawSha256:sha(fs.readFileSync(afterRaw)),nativeSha256:sha(fs.readFileSync(afterNative))},integration,rows,scope:'One bounded paired synthetic2786-thread/45-project/32-active exact packaged-runtime comparison against PR411 for exact-clock history-only rendered-sidebar reuse. Direct receiver resolves acknowledged references and matches full logical replies/native CBOR. Missing/bad ack and legacy-client full transport remain exact. Actual server commit revision paired with state, complete raw signal maps/replies/native CBOR, actual indexed timeline bodies, config preservation and input immutability checked. Four preserved cfg paths are bump/live/warm/notes; store switch is unreachable in this package and receives no runtime acceptance claim. No IO module factory, hub, user profile/history, UI/provider or live speedup/INP claim. Timing includes wrapper overhead and is not a stable live latency ratio.'};
fs.writeFileSync(out,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result));if(state!=='passed')process.exitCode=1;
