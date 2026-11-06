const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
function fixture(){
 const source=fs.readFileSync(require.resolve('../../src/web/host.js'),'utf8');
 const session={sel:'t',drafts:{}},controller={ready:true,epoch:'epoch',client:{links:new Map([['hub',{viewId:'view'}]])},hub:'hub'};
 let uploads=0,pending=false,stalled=false,clock=1000;const notes=[];
 const context={ui:{},document:{cookie:'old'},cacheIdentity:c=>c,attachmentIdentity:'',attachmentIdentityPending:true,attachmentIdentitySince:clock,
  SEND_WAIT:5000,Date:{now:()=>clock},trace(){},
  thin:controller,attachmentController:null,attachmentView:null,attachmentActor:null,attachmentSelection:'',
  currentSession:()=>session,uploadScope:()=>({controller,epoch:'epoch',viewId:'view',thread:'t'}),upload(){},
  draftAttachmentPaths:()=>[],viewMode:'view-v1',socket:{readyState:1},showViewStatus(){},viewNote:'',viewNoteTimer:0,holdViewNote(m){notes.push(m);context.viewNote=m},clearTimeout(){},App:{host_message:(_ui,kind)=>kind},
  draftAttachments:{started:new Map(),pending:()=>pending&&!stalled,stalled:()=>pending&&stalled,abandon(){pending=false},restores:()=>pending,
   sync(thread,paths,up,current,actor){uploads++;pending=true;this.started.set(thread,actor);return Promise.resolve()}},};
 const h=source.indexOf('function attachmentsHoldSend('),i=source.indexOf('\nfunction draftAttachmentPaths',h);
 const a=source.indexOf('function restoreDraftAttachments('),z=source.indexOf('\nfunction uploadScope',a);
 const d=source.indexOf('function dispatch(action, value, timer)'),e=source.indexOf('  if (["send", "send-alt"].includes(action)) {',d);
 assert(h>0&&i>h&&a>0&&z>a&&d>0&&e>d);
 vm.runInNewContext(source.slice(h,i)+'\n'+source.slice(a,z)+'\n'+source.slice(d,e)+'  return false;\n}',context);
 return {context,session,notes,uploads:()=>uploads,settle(){pending=false},stall(){stalled=true},tick(ms){clock+=ms}};
}
test('a public cache tag cannot restore bytes before the authenticated identity is verified',()=>{
 const f=fixture();f.context.restoreDraftAttachments(f.session);assert.equal(f.uploads(),0);
 assert.equal(f.context.dispatch('send',''),true);
 f.context.attachmentIdentity='old';f.context.attachmentIdentityPending=false;
 f.context.restoreDraftAttachments(f.session);assert.equal(f.uploads(),1);
 assert.equal(f.context.dispatch('send',''),true);
 f.settle();assert.equal(f.context.dispatch('send',''),false);
});
test('a changed cookie cannot borrow an earlier verified identity',()=>{
 const f=fixture();f.context.attachmentIdentity='old';f.context.document.cookie='new';
 f.context.restoreDraftAttachments(f.session);assert.equal(f.uploads(),0);
 f.context.attachmentIdentity='new';f.context.restoreDraftAttachments(f.session);assert.equal(f.uploads(),1);
});

test('a temporary identity failure keeps Send gated until verification can retry',async()=>{
 const f=fixture(),source=fs.readFileSync(require.resolve('../../src/web/host.js'),'utf8');
 let result={state:'offline',identity:'old'},retry;
 Object.assign(f.context,{attachmentIdentityGeneration:0,attachmentIdentityRetry:0,trace(){},
  window:{fetch(){}},location:{protocol:'http:'},refreshCacheIdentity:async()=>result,
  enterImprovements(){},pairingUI:{accepted(){}},clearTimeout(){},setTimeout(fn){retry=fn;return 1}});
 const a=source.indexOf('function refreshFocusedIdentity()'),z=source.indexOf('\nlet cacheReady',a);
 vm.runInNewContext(source.slice(a,z),f.context);
 await f.context.refreshFocusedIdentity();
 assert.equal(f.context.attachmentIdentityPending,true);assert.equal(f.context.dispatch('send',''),true);
 assert.equal(typeof retry,'function');
 result={state:'verified',identity:'old'};f.context.draftAttachments.prune=async()=>{};
 await f.context.refreshFocusedIdentity();
 assert.equal(f.context.attachmentIdentityPending,false);assert.equal(f.uploads(),1);
});

test('an unanswered pairing check holds Send for SEND_WAIT, then Send goes and restores nothing',()=>{
 const f=fixture();
 assert.equal(f.context.dispatch('send',''),true);assert.deepEqual(f.notes,['attachment-restoring']);
 f.tick(4999);assert.equal(f.context.dispatch('send',''),true);
 f.tick(1);assert.equal(f.context.dispatch('send',''),false);
 // the wait's note goes with it, and no bytes went under the unverified pairing
 assert.equal(f.context.viewNote,'');
 assert.equal(f.uploads(),0);
});
test('a stalled restore is abandoned at Send and said once',()=>{
 const f=fixture();f.context.attachmentIdentity='old';f.context.attachmentIdentityPending=false;
 f.context.restoreDraftAttachments(f.session);assert.equal(f.uploads(),1);
 assert.equal(f.context.dispatch('send',''),true);
 f.stall();assert.equal(f.context.dispatch('send',''),false);
 assert.equal(f.context.dispatch('send',''),false);
 assert.deepEqual(f.notes,['attachment-restoring','attachment-unrestored']);assert.equal(f.uploads(),1);
});
test('the pairing check keeps its start across retries, so Send is not held again by each retry',async()=>{
 const f=fixture(),source=fs.readFileSync(require.resolve('../../src/web/host.js'),'utf8');
 Object.assign(f.context,{attachmentIdentityGeneration:0,attachmentIdentityRetry:0,window:{fetch(){}},location:{protocol:'http:'},
  refreshCacheIdentity:async()=>({state:'offline',identity:'old'}),enterImprovements(){},clearTimeout(){},setTimeout(){return 1}});
 const a=source.indexOf('function refreshFocusedIdentity()'),z=source.indexOf('\nlet cacheReady',a);
 vm.runInNewContext(source.slice(a,z),f.context);
 f.context.attachmentIdentityPending=false;f.tick(10);
 await f.context.refreshFocusedIdentity();const since=f.context.attachmentIdentitySince;assert.equal(since,1010);
 f.tick(3000);await f.context.refreshFocusedIdentity();assert.equal(f.context.attachmentIdentitySince,since);
 f.tick(2000);assert.equal(f.context.dispatch('send',''),false);
});
