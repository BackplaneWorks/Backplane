// Production prefetch transport against a throwaway hub. This never reads a
// live home, runs agents, or writes to user projects. Run with Bun.
import fs from 'node:fs';import os from 'node:os';import path from 'node:path';import assert from 'node:assert/strict';import {randomBytes} from 'node:crypto';
import {ViewPrefetch} from '../../src/web/view-prefetch.js';import {ThinViews} from '../../src/web/thin.js';import {cbor,plain} from '../../src/web/wire.js';
const root=fs.mkdtempSync(path.join(os.tmpdir(),'bp-prefetch-')),home=path.join(root,'home'),project=path.join(root,'project'),agents=path.join(root,'agents');
for(const d of [home,project,agents])fs.mkdirSync(d);
const token=randomBytes(24).toString('hex');fs.writeFileSync(path.join(home,'token'),token,{mode:0o600});
const at=Math.floor(Date.now()/1000)-1000,rows=[{ $:'ProjectCreated',id:'p',title:'Private prefetch fixture',root:project,at},...['restart.continue','telemetry.share','fallback.on'].map(key=>({$:'SettingSet',key,value:'off',at})),...['onboard.done','onboard.uber'].map(key=>({$:'SettingSet',key,value:'1',at}))];
for(const id of ['A','B','C']){rows.push({$:'ThreadCreated',id,project:'p',title:'Fixture '+id,env:'local',branch:'',worktree:project,provider:'claude',model:'fixture',at});for(let i=0;i<40;i++)rows.push({$:'MessagePosted',thread:id,msg:id+i,role:i%2?'assistant':'user',text:'Fixture '+id+' message '+i,at:at+i});}
const log=path.join(home,'events.jsonl');fs.writeFileSync(log,rows.map(x=>JSON.stringify(x)).join('\n')+'\n');
const source=fs.readFileSync(new URL('../../src/core/cbor.bend',import.meta.url),'utf8'),dict=name=>{const a=source.indexOf('def Cbor.'+name+'()');return JSON.parse(source.slice(source.indexOf('[',a),source.indexOf(']',a)+1));},keys=dict('keys'),words=dict('words');
function encode(v){const out=[],head=(m,n)=>{if(n<24)out.push(m<<5|n);else if(n<256)out.push(m<<5|24,n);else if(n<65536)out.push(m<<5|25,n>>8,n&255);else out.push(m<<5|26,n>>>24&255,n>>>16&255,n>>>8&255,n&255)};const put=x=>{if(x===null)out.push(246);else if(typeof x==='boolean')out.push(x?245:244);else if(typeof x==='number'){out.push(199);put(String(x))}else if(typeof x==='string'){const b=new TextEncoder().encode(x);head(3,b.length);out.push(...b)}else if(Array.isArray(x)){head(4,x.length);x.forEach(put)}else{const xs=Object.entries(x);head(5,xs.length);xs.forEach(([k,v])=>{put(k);put(v)})}};put(v);return new Uint8Array(out)}
const decode=data=>plain(cbor(new Uint8Array(data),keys,words));
const listener=Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}}),port=listener.port;listener.stop();assert.notEqual(port,3787);const origin='http://127.0.0.1:'+port;
let child,socket,prefetch;
const pause=ms=>new Promise(r=>setTimeout(r,ms));
async function until(fn,ms=15000){const end=Date.now()+ms;while(Date.now()<end){const v=await fn();if(v)return v;await pause(20)}throw Error('Private prefetch fixture deadline');}
try{
 const bin=process.env.INSTANT_HUB;if(!bin)throw Error('INSTANT_HUB required; explicit binary and private home only');
 child=Bun.spawn([bin,'--home',home,'--port',String(port),'--host','127.0.0.1','--headless','--no-tailscale'],{env:{...process.env,BACKPLANE_COPY:'1',BACKPLANE_AGENTS:agents,BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_NO_UPDATE:'1',BACKPLANE_TELEMETRY:'off'},stdout:Bun.file(path.join(root,'stdout')),stderr:Bun.file(path.join(root,'stderr'))});
 await until(async()=>{try{return(await fetch(origin+'/hello')).ok}catch{return false}});
 socket=new WebSocket(origin.replace('http:','ws:')+'/ws?view=1&frontend=1&token='+token);socket.binaryType='arraybuffer';let greeting,opened;
 const views=new ThinViews({now:()=>performance.now(),send:(_hub,r)=>{socket.send(encode(r));return r},command:()=>null}),session={sel:'A',rid:1,drafts:{},open:{},info:{},adding:false,panel:false};
 socket.onmessage=e=>{const f=decode(e.data);if(f.t==='view'){greeting=f;views.connect(origin,f);views.select(origin,{kind:'thread',id:'A'},session,'foreground')}else if(f.t==='reply')opened=views.reply(origin,f)};
 await until(()=>opened?.remoteReady);
 const before=fs.readFileSync(log,'utf8'),pictures=[],requests=[],scope={origin,identity:'fixture-owner',epoch:greeting.epoch,language:'en'};
 prefetch=new ViewPrefetch({hub:origin,scope:()=>scope,has:()=>false,idle:()=>true,
  socket:()=>new WebSocket(origin.replace('http:','ws:')+'/ws?view=1&frontend=1&token='+token),
  encode:r=>{requests.push(r);return encode(r)},decode,picture:(shown,s,target)=>pictures.push({shown,selected:s.sel,target})});
 prefetch.enqueue({kind:'thread',id:'B'});prefetch.enqueue({kind:'thread',id:'C'});
 await until(()=>pictures.length===2);prefetch.stop();
 assert.deepEqual(pictures.map(p=>p.selected),['B','C']);assert(requests.every(r=>r.m==='view.open'));assert(requests.every(r=>Object.keys(r.p.session.drafts).join(',')==='@lang'));
 assert.equal(fs.readFileSync(log,'utf8'),before,'prefetch must not write read markers or any canonical event');
 opened=null;views.request(origin,'view.update',{session});await until(()=>opened?.remoteReady);assert.deepEqual(opened.result.target,{kind:'thread',id:'A'},'prefetch must not replace foreground actor');
 assert(opened.window.ids.length<=32);assert.equal(fs.readFileSync(log,'utf8'),before,'foreground read update leaves canonical event log unchanged');
 console.log(JSON.stringify({pass:true,checks:7,prefetched:pictures.map(p=>p.selected),foreground:opened.result.target.id,canonicalLogUnchanged:true,maxWindow:opened.window.ids.length}));
}finally{prefetch?.stop();socket?.close();child?.kill();if(child)await child.exited;fs.rmSync(root,{recursive:true,force:true});}
