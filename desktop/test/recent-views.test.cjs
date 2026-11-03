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
