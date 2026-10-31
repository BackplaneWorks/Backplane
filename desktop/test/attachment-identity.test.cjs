const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
function fixture(){
 const source=fs.readFileSync(require.resolve('../../src/web/host.js'),'utf8');
 const session={sel:'t',drafts:{}},controller={ready:true,epoch:'epoch',client:{links:new Map([['hub',{viewId:'view'}]])},hub:'hub'};
 let uploads=0,pending=false;
 const context={ui:{},document:{cookie:'old'},cacheIdentity:c=>c,attachmentIdentity:'',attachmentIdentityPending:true,
  thin:controller,attachmentController:null,attachmentView:null,attachmentActor:null,attachmentSelection:'',
  currentSession:()=>session,uploadScope:()=>({controller,epoch:'epoch',viewId:'view',thread:'t'}),upload(){},
  draftAttachmentPaths:()=>[],viewMode:'view-v1',socket:{readyState:1},showViewStatus(){},App:{host_message:()=>''},
  draftAttachments:{started:new Map(),pending:()=>pending,sync(thread,paths,up,current,actor){uploads++;pending=true;this.started.set(thread,actor);return Promise.resolve()}},};
 const a=source.indexOf('function restoreDraftAttachments('),z=source.indexOf('\nfunction uploadScope',a);
 const d=source.indexOf('function dispatch(action, value, timer)'),e=source.indexOf('  if (["send", "send-alt"].includes(action)) historyClear();',d);
 vm.runInNewContext(source.slice(a,z)+'\n'+source.slice(d,e)+'  return false;\n}',context);
 return {context,session,uploads:()=>uploads,settle(){pending=false}};
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
  clearTimeout(){},setTimeout(fn){retry=fn;return 1}});
 const a=source.indexOf('function refreshFocusedIdentity()'),z=source.indexOf('\nlet cacheReady',a);
 vm.runInNewContext(source.slice(a,z),f.context);
 await f.context.refreshFocusedIdentity();
 assert.equal(f.context.attachmentIdentityPending,true);assert.equal(f.context.dispatch('send',''),true);
 assert.equal(typeof retry,'function');
 result={state:'verified',identity:'old'};f.context.draftAttachments.prune=async()=>{};
 await f.context.refreshFocusedIdentity();
 assert.equal(f.context.attachmentIdentityPending,false);assert.equal(f.uploads(),1);
});
