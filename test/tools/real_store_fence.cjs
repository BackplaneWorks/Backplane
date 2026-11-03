// Owned copied-store child only; never loaded into the live hub.
const net=require('node:net');
const ok=h=>h==='127.0.0.1'||h==='::1'||h==='localhost';
const deny=()=>{throw Error('owned-copy network fence')};
function host(v){if(v instanceof URL)return v.hostname;if(typeof v?.url==='string')return new URL(v.url).hostname;if(typeof v==='string'){try{return new URL(v).hostname}catch{return v}}return v?.hostname||v?.host||'localhost';}
const f=globalThis.fetch;globalThis.fetch=function(v,...a){if(!ok(host(v)))return Promise.reject(Error('owned-copy network fence'));return f.call(this,v,{...a[0],redirect:"error"})};
if(typeof Bun.connect==='function'){const b=Bun.connect;Bun.connect=function(v){if(v.unix||!ok(host(v)))return deny();return b.call(this,v)}}
for(const name of ['connect','createConnection']){const f=net[name];net[name]=function(...a){const x=typeof a[0]==='number'?a[1]:a[0];if(typeof x==='string'&&!ok(x)||typeof x==='object'&&(x.path||!ok(host(x))))return deny();return f.apply(this,a)}}
for(const n of ['node:http','node:https']){const m=require(n);for(const name of ['request','get']){const f=m[name];m[name]=function(v,...a){if(!ok(host(v)))return deny();return f.call(this,v,{...a[0],redirect:"error"})}}}
const WS=globalThis.WebSocket;if(WS)globalThis.WebSocket=class extends WS{constructor(v,...a){if(!ok(host(v)))deny();super(v,...a)}};
