'use strict';
// Nothing is drawn in between two hub views: a thread picked, a hub
// restart's reconnect and a page load go from one whole picture to the next
// (host.js's holdPicture/restoreTransition, style.css's held/wait rules).
// Before, each showed a made-up "Loading conversation…" page for a few
// frames: no rail, no composer, an empty breadcrumb, State "idle".
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=fs.readFileSync(path.resolve(__dirname,'../../src/web/host.js'),'utf8');
const css=fs.readFileSync(path.resolve(__dirname,'../../src/web/style.css'),'utf8');
const el=(tag,key,attrs=[],kids=[])=>({type:'element',tag,key,attrs,kids});
const cls=v=>({name:'class',value:v});
// a thread's view as the hub draws it with the rail: the composer sits in .thread-content
const view=(thread,body)=>el('div','app',[cls('app picked top')],[el('header','topbar',[cls('topbar')]),el('aside','sidebar',[cls('sidebar')]),
 el('main','thread',[cls('thread-view workspace-thread'),{name:'data-thread',value:thread}],[el('nav','workspace',[cls('workspace-bar')]),
  el('div','',[cls('thread-content')],[el('div','timeline',[cls('timeline')],[{type:'text',text:body}]),el('div','',[cls('composer')],[el('textarea','',[{name:'id',value:'composer'}])])])])]);

test('a thread seen before comes back as it was, the rail layout included',async()=>{
 const {ViewTransition}=await import('../../src/web/view-transition.js');const cache=new ViewTransition();
 const scope=sel=>({origin:'o',identity:'i',language:'en',epoch:'e',target:{kind:'thread',id:sel},selected:sel,settings:false,route:null});
 assert(cache.put(scope('A'),view('A','A body')));assert.equal(cache.get(scope('A')).kids[2].kids[1].kids[0].kids[0].text,'A body');
 // never another thread's view, nor a page without a composer (Settings, forms)
 assert.equal(cache.put(scope('B'),view('A','A body')),false);
 assert.equal(cache.put(scope('C'),el('div','app',[],[el('main','',[{name:'data-thread',value:'C'}],[el('div','',[cls('settings')])])])),false);
 assert.equal(cache.put(scope('D'),el('div','app',[],[el('main','',[{name:'data-thread',value:'D'}],[el('div','',[cls('composer-foot')])])])),false);
});

// host.js's own holdPicture and restoreTransition, with what they read stubbed
function host(retained={}){
 const a=source.indexOf('const targetKey = '),b=source.indexOf('const focusedView = new FocusedView(',a);assert(a>0&&b>a);
 const timers=[];
 const ctx={frontendNode:null,transitionNode:null,transitionFor:'',transitionPosition:null,shownFor:'',heldAt:0,QUIET_MS:1200,epoch:'e1',
  performance:{now:()=>5000},setTimeout:(f,ms)=>timers.push(ms),later(){},themed:x=>x,presentationJson:x=>x,App:{presentation_node:n=>({node:n})},
  transitionScope:t=>({target:t,epoch:ctx.epoch}),
  transitionViews:{get:s=>retained[s.epoch+':'+s.target.id]||null,position:()=>null}};
 vm.createContext(ctx);vm.runInContext(source.slice(a,b).replace(/^let /gm,'var ').replace(/^const /gm,'var '),ctx);
 // vm globals: host.js's lets live on ctx
 return {ctx,timers,show:(id,node)=>{ctx.frontendNode=node;ctx.shownFor='thread:'+id;ctx.transitionNode=null;ctx.transitionFor='';ctx.heldAt=0;},
  pick:id=>{vm.runInContext('holdPicture()',ctx);return vm.runInContext(`restoreTransition({kind:'thread',id:${JSON.stringify(id)}})`,ctx);}};
}
test('picking a thread never seen keeps the view on screen until its own comes',()=>{
 const h=host();h.show('A','A view');
 assert.equal(h.pick('B'),false);
 assert.equal(h.ctx.transitionNode,'A view');assert.equal(h.ctx.frontendNode,null);assert.equal(h.ctx.transitionFor,'thread:A');
 // the kept view is another thread's: held from now, quieted past QUIET_MS
 assert.equal(h.ctx.heldAt,5000);assert.deepEqual(h.timers,[1220]);
 // and its scroll is left alone (the new view gets its own place when it comes)
 assert.equal(h.ctx.transitionPosition,null);
 // a third pick before the view comes keeps the same picture, held since the first
 h.ctx.performance.now=()=>5300;assert.equal(h.pick('C'),false);assert.equal(h.ctx.transitionNode,'A view');assert.equal(h.ctx.heldAt,5000);assert.equal(h.timers.length,1);
});
test('a thread seen before shows its own picture at once, not held',()=>{
 const h=host({'e1:B':'B retained'});h.show('A','A view');
 assert.equal(h.pick('B'),true);
 assert.deepEqual(h.ctx.transitionNode,{node:'B retained'});assert.equal(h.ctx.transitionFor,'thread:B');assert.equal(h.ctx.heldAt,0);
 assert.equal(JSON.stringify(h.ctx.transitionPosition),'{"atEnd":true}');assert.equal(h.timers.length,0);
});
test('a reconnect keeps the last view of the same thread, with no loading page',()=>{
 const h=host({'e1:A':'A retained'});h.show('A','A view');
 // the socket drops (state "disconnected"): the view stays
 vm.runInContext('holdPicture()',h.ctx);assert.equal(h.ctx.transitionNode,'A view');
 // the restarted hub has another epoch: nothing retained matches, the view stays
 h.ctx.epoch='e2';assert.equal(vm.runInContext("restoreTransition({kind:'thread',id:'A'})",h.ctx),false);
 assert.equal(h.ctx.transitionNode,'A view');assert.equal(h.ctx.transitionFor,'thread:A');assert.equal(h.ctx.heldAt,0);assert.equal(h.timers.length,0);
});
test('nothing in the page makes up a loading page; render marks held and waiting pictures for style.css',()=>{
 for(const f of fs.readdirSync(path.resolve(__dirname,'../../src/web')).filter(f=>f.endsWith('.js')))
  assert(!fs.readFileSync(path.resolve(__dirname,'../../src/web',f),'utf8').includes('Loading conversation'),f);
 assert.match(source,/dataset\.backplaneHeld = !heldFor \? "false" : heldAt && performance\.now\(\) - heldAt >= QUIET_MS \? "quiet" : "true";/);
 assert.match(source,/dataset\.backplaneWait = String\(!frontendNode && !transitionNode && !focusedNode && !canonicalBoot\);/);
 // a new view drops what was kept and says what it was drawn for
 assert.match(source,/frontendNode = node;transitionNode = null;transitionFor = "";heldAt = 0;\n\s*shownFor = targetKey\(detail\.target\);/);
 assert.match(css,/html\[data-backplane-held="true"\][^{]*\{ pointer-events: none; \}/);
 assert.match(css,/html\[data-backplane-held="quiet"\][^{]*\{ visibility: hidden; \}/);
 assert.match(css,/html\[data-backplane-wait="true"\][^{]*\.model-chip[^{]*\{ visibility: hidden; \}/);
});

test('the first connection says Connecting only when it takes a while',()=>{
 const a=source.indexOf('let connectingTimer = 0;'),b=source.indexOf('// Reconnecting',a);assert(a>0&&b>a);
 const status=[],timers=new Map();let next=0;
 const ctx={QUIET_MS:1200,remoteReady:false,downSince:0,showViewStatus:m=>{timers.clear();status.push(m);},
  setTimeout:(f,ms)=>{timers.set(++next,{f,ms});return next;},clearTimeout:id=>timers.delete(id)};
 vm.createContext(ctx);vm.runInContext(source.slice(a,b).replace('let connectingTimer','var connectingTimer'),ctx);
 const fire=()=>{for(const [id,t] of [...timers]){timers.delete(id);t.f();}};
 vm.runInContext('connectingStatus()',ctx);assert.deepEqual(status,[]);assert.equal([...timers.values()][0].ms,1200);
 fire();assert.deepEqual(status,['Connecting…']);
 // the view came first: nothing is said
 status.length=0;vm.runInContext('connectingStatus()',ctx);ctx.remoteReady=true;fire();assert.deepEqual(status,[]);
 // a reconnect's own words win
 ctx.remoteReady=false;vm.runInContext('connectingStatus()',ctx);ctx.downSince=1;fire();assert.deepEqual(status,[]);
 // startThin asks for it instead of the bar at once
 assert.match(source,/if \(downSince\) downStatus\(downNote\);\n\s*else connectingStatus\(\);/);
 assert(!/else showViewStatus\("Connecting…"\)/.test(source));
 // any status shown cancels a pending one
 assert.match(source,/function showViewStatus\(message\) \{\n\s*clearTimeout\(connectingTimer\);connectingTimer = 0;/);
});
