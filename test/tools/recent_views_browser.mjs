// Real IndexedDB eviction and reload tests; private browser profile, no live hub.
import {createServer} from 'node:http';
import {readFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import assert from 'node:assert/strict';
const require=createRequire(import.meta.url),{chromium}=require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const files=new Map(['recent-views.js','view-transition.js'].map(name=>['/'+name,readFileSync(new URL('../../src/web/'+name,import.meta.url))]));
const server=createServer((req,res)=>{res.setHeader('Content-Type',files.has(req.url)?'text/javascript':'text/html');res.end(files.get(req.url)||'<!doctype html><title>Private cache test</title>');});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const browser=await chromium.launch({headless:true,executablePath:process.env.CHROMIUM_PATH,args:['--no-sandbox']});
try{
 const page=await browser.newPage();await page.goto('http://127.0.0.1:'+server.address().port);
 const first=await page.evaluate(async()=>{
  const {RecentViews}=await import('/recent-views.js');
  const scope={origin:'https://hub',identity:'a'.repeat(64),language:'en',epoch:'e',bundle:'v',selected:'A',target:{kind:'thread',id:'A'}};
  const node=id=>({type:'element',tag:'main',key:'thread',attrs:[{name:'data-thread',value:id}],kids:[{type:'element',tag:'div',key:'',attrs:[{name:'class',value:'composer'}],kids:[{type:'element',tag:'textarea',key:'',attrs:[{name:'value',value:'UNSENT SECRET'}],kids:[{type:'text',text:'UNSENT SECRET'}]}]}]});
  const store=new RecentViews({indexedDB,validate:n=>n.type==='element',limit:3});
  for(const id of ['A','B','C','D'])await store.save({...scope,selected:id,target:{kind:'thread',id}},id,node(id));
  const rows=await store.load(scope);
  return {count:rows.length,keys:rows.map(r=>r.viewKey),privateDraft:JSON.stringify(rows).includes('UNSENT SECRET'),owner:(await store.load({...scope,identity:'b'.repeat(64)})).length};
 });
 assert.equal(first.count,3);assert(!first.keys.includes('A'));assert.equal(first.privateDraft,false);assert.equal(first.owner,0);
 await page.reload();
 const second=await page.evaluate(async()=>{
  const {RecentViews}=await import('/recent-views.js'),{ViewTransition}=await import('/view-transition.js');
  const scope={origin:'https://hub',identity:'a'.repeat(64),language:'en',epoch:'e',bundle:'v',selected:'D',target:{kind:'thread',id:'D'}};
  const cache=new ViewTransition({store:new RecentViews({indexedDB,validate:n=>n.type==='element',limit:3})});
  const start=performance.now();await cache.warm(scope);
  const result={ms:performance.now()-start,restored:!!cache.destination(scope,scope.target),count:cache.views.size};
  const db=await cache.store.open(),tx=db.transaction('pictures','readwrite'),store=tx.objectStore('pictures');
  const records=await new Promise(resolve=>{const r=store.getAll();r.onsuccess=()=>resolve(r.result)});
  for(const record of records)store.put({...record,shown:{type:'text',text:'malformed'}});
  await new Promise(r=>tx.oncomplete=r);
  result.corrupt=(await cache.store.load(scope)).length;
  return result;
 });
 assert(second.restored);assert.equal(second.count,3);assert.equal(second.corrupt,0);assert(second.ms<100,'disk cache restore exceeded100ms: '+second.ms);
 console.log(JSON.stringify({pass:true,checks:10,first,second}));
}finally{await browser.close();await new Promise(r=>server.close(r));}
