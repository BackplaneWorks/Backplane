const {test}=require('node:test'),assert=require('node:assert/strict');
const el=(tag,attrs=[],kids=[])=>({type:'element',tag,key:'',attrs,kids});
const view=thread=>el('main',[{name:'data-thread',value:thread}],[el('div',[{name:'class',value:'composer'}],[el('textarea',[{name:'value',value:'PRIVATE DRAFT'}],[{type:'text',text:'PRIVATE DRAFT'}])])]);
const scope={origin:'https://hub',identity:'a'.repeat(64),language:'en',epoch:'epoch',bundle:'web1',selected:'chat',target:{kind:'bot',id:'bot'},route:{kind:'bot-tab',bot:'bot',tab:'chat'}};
test('bot destination lookup works before backing conversation is known, with exact owner and hub fences',async()=>{
 const {ViewTransition}=await import('../../src/web/view-transition.js');const cache=new ViewTransition();assert(cache.put(scope,view('chat')));
 assert.equal(cache.destination({...scope,selected:'previous',route:null},scope.target).scope.selected,'chat');
 for(const key of ['origin','identity','language','epoch','bundle'])assert.equal(cache.destination({...scope,[key]:'other'},scope.target),null,key);
 assert.equal(cache.destination(scope,{kind:'thread',id:'bot'}),null);
});
test('warming hydrates bounded pictures without adopting drafts, cursor, readiness or an old request session',async()=>{
 const {ViewTransition}=await import('../../src/web/view-transition.js');const {pictureOnly}=await import('../../src/web/recent-views.js');
 const cache=new ViewTransition({store:{load:async()=>[{scope,shown:pictureOnly(view('chat')),session:{rid:999,drafts:{chat:'OLD'}}}],save(){throw Error('hydration must not persist again')}}});
 await cache.warm(scope);assert(cache.get(scope));assert(!JSON.stringify(cache.get(scope)).includes('PRIVATE DRAFT'));assert.equal(cache.session,undefined);assert.equal(cache.cursor,undefined);
});
test('late disk pictures lose to current memory and credential invalidation',async()=>{
 const {ViewTransition}=await import('../../src/web/view-transition.js');let release;
 const cache=new ViewTransition({store:{load:()=>new Promise(r=>release=r),save:async()=>true}}),warm=cache.warm(scope);
 const newest=view('chat');cache.put(scope,newest);release([{scope,shown:view('chat')}]);await warm;assert.equal(cache.get(scope),newest);
 const other={...scope,epoch:'new'},next=cache.warm(other);cache.clear();release([{scope:other,shown:view('chat')}]);await next;assert.equal(cache.views.size,0);
});
test('persisted picture validation rejects cross-owner, expired, oversized or malformed records',async()=>{
 const {RecentViews,recentPartition}=await import('../../src/web/recent-views.js');const cache=new RecentViews({now:()=>1000000,validate:n=>n?.type==='element'}),partition=recentPartition(scope);
 const good={partition,viewKey:'key',key:partition+'\nkey',scope,shown:view('chat'),savedAt:1000000};assert(cache.valid(good,partition));
 assert(!cache.valid({...good,scope:{...scope,identity:'b'.repeat(64)}},partition));assert(!cache.valid({...good,savedAt:-1e10},partition));assert(!cache.valid({...good,shown:{type:'text',text:'x'}},partition));
 assert.equal(await cache.load({...scope,identity:''}).then(x=>x.length),0);
});

test('project homes use the same bounded picture cache with exact artifact-owner fences',async()=>{
 const {ViewTransition}=await import('../../src/web/view-transition.js');
 const {pictureOnly}=await import('../../src/web/recent-views.js');
 const cache=new ViewTransition(), homeScope={...scope,selected:'project-home-project',target:{kind:'project',id:'project'},route:null};
 const home=el('main',[{name:'data-project-home',value:'project'}],[el('form',[{name:'class',value:'composer home-composer'}],[el('textarea',[{name:'value',value:'Private project draft'}])])]);
 assert(cache.put(homeScope,home));
 assert.equal(cache.get(homeScope),home);
 assert.equal(cache.destination(homeScope,{kind:'project',id:'project'}).shown,home);
 assert.equal(cache.destination(homeScope,{kind:'project',id:'other'}),null);
 assert(!cache.put({...homeScope,selected:'project-home-other'},home));
 assert(!cache.put({...homeScope,target:{kind:'project',id:'other'}},home));
 assert(!JSON.stringify(pictureOnly(home)).includes('Private project draft'));
});
test('a view is written at most once per gap, its newest picture kept for the next write',async()=>{
 const {RecentViews}=await import('../../src/web/recent-views.js');
 let clock=0;const saved=[];const r=new RecentViews({indexedDB:null,validate:()=>true,now:()=>clock});
 r.save=async(scope,key,shown,checked)=>{saved.push([key,shown,checked]);return true;};
 r.schedule({},'A','one');clearTimeout(r.scheduled);r.flush();
 clock=1000;r.schedule({},'A','two');clearTimeout(r.scheduled);r.flush();
 clock=2000;r.schedule({},'A','three');clearTimeout(r.scheduled);r.flush();
 assert.deepEqual(saved.map(x=>x[1]),['one'],'within the gap: not written again');
 clock=10001;clearTimeout(r.scheduled);r.flush();clearTimeout(r.scheduled);
 assert.deepEqual(saved.map(x=>x[1]),['one','three'],'after it: the newest');
 assert.equal(saved[0][2],true,'the host’s pictures are written without validating them again');
});
