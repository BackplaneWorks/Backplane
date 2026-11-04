'use strict';
// sw.js's notificationclick: a phone's notification (host.js's notify,
// shown through the worker) brings the page forward and opens its thread
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
function worker(windows){
 const source=fs.readFileSync(path.resolve(__dirname,'../../src/web/sw.js'),'utf8'),events=new Map(),opened=[];
 const ctx={self:{addEventListener:(name,fn)=>events.set(name,fn),skipWaiting(){},clients:{claim(){},matchAll:async()=>windows,openWindow:async u=>{opened.push(u);}}},location:{origin:'https://fixture'},URL,Response,fetch:async()=>{throw Error('offline');},caches:{open:async()=>({})}};
 vm.createContext(ctx);vm.runInContext(source,ctx);
 return{opened,click:async thread=>{let closed=false,wait;events.get('notificationclick')({notification:{data:{thread},close(){closed=true;}},waitUntil:p=>wait=p});await wait;return closed;}};
}
const win=(focused)=>{const w={focused,posted:[],focusedNow:false,postMessage(m){w.posted.push(m);},focus(){w.focusedNow=true;return Promise.resolve(w);}};return w;};
test('a tap on a notification opens its thread in the page it came from',async()=>{
 const a=win(false),b=win(true),w=worker([a,b]);
 assert.equal(await w.click('t1'),true);
 assert.equal(JSON.stringify(b.posted),JSON.stringify([{t:'bp-open',thread:'t1'}]));assert.equal(b.focusedNow,true);assert.equal(a.posted.length,0);
});
test('with no page open, a tap opens the app',async()=>{
 const w=worker([]);await w.click('t1');assert.equal(JSON.stringify(w.opened),'["/"]');
});
