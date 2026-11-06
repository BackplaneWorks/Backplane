// Parity harness core: a recording client that keeps every frame as raw bytes AND decoded JSON.
import {decode,encode} from '../wire.mjs';
import {applyViewDelta} from '../../../src/web/view-delta.js';
export const sleep=ms=>new Promise(r=>setTimeout(r,ms));
export const b64=u8=>Buffer.from(u8).toString('base64');

export class Rec{
  constructor(){this.frames=[];this.t0=performance.now();this.n=0}
  push(c,dir,kind,raw,json,extra={}){if(this.vol)extra={...extra,vol:true};const f={i:this.n++,c,dir,kind,ms:Math.round(performance.now()-this.t0),raw:raw==null?null:(typeof raw==='string'?raw:b64(raw)),json,...extra};this.frames.push(f);return f}
}

// Profiles: what a renderer asks for. query = the websocket query a real client uses.
export const PROFILES={
  raw:{query:'',view:false},
  mobile:{query:'view=1&mobile=1',view:true,extra:{}},
  gpui:{query:'view=1&mobile=1',view:true,extra:{nativeSidebar:1}},
  // a native client that offers the keyed view delta (viewDelta {version, token}) and applies the ops it is sent
  gpuid:{query:'view=1&mobile=1',view:true,extra:{nativeSidebar:1},delta:true},
};

export async function httpProbe(hub,path,rec,name,{auth=false}={}){
  rec.push(name,'out','http-get',null,{path,auth});
  const r=await fetch(hub.url+path,auth?{headers:{Authorization:'Bearer '+hub.token}}:{}).catch(e=>({status:0,headers:new Headers(),arrayBuffer:async()=>new ArrayBuffer(0),err:String(e)}));
  const body=new Uint8Array(await r.arrayBuffer());
  let json=null;try{json=decode(body)}catch{}
  rec.push(name,'in','http',body,json,{status:r.status,ctype:r.headers.get('content-type')||''});
  return r.status;
}

// a websocket client; frames recorded as they arrive
export async function connect({hub,rec,name,profile='raw',query,token,expectFail=false}){
  const P=PROFILES[profile];const q=(query??P.query);
  const url=`ws://${hub.host}:${hub.port}/ws?${q?q+'&':''}token=${encodeURIComponent(token??hub.token)}`;
  rec.push(name,'out','connect',null,{mode:'good',profile,query:q});
  const ws=new WebSocket(url);ws.binaryType='arraybuffer';
  const c={name,ws,profile,P,rec,inbox:[],greet:null,closed:false,closeCode:null,view:{viewId:'',rev:0,target:{kind:'recent',id:''},session:{sel:'',rid:1,drafts:{},open:{},adding:false,panel:false,info:{}},next:2**47+100,last:null,bases:new Map(),token:'',noOffer:false},id:0};
  ws.onmessage=e=>{const bin=typeof e.data!=='string';let json=null;try{json=bin?decode(new Uint8Array(e.data)):JSON.parse(e.data)}catch(err){json={undecodable:String(err)}}
    rec.push(name,'in',bin?'bin':'text',bin?new Uint8Array(e.data):e.data,JSON.parse(JSON.stringify(json)));c.inbox.push(json);if(json?.t==='view')c.greet=json;};
  ws.onclose=e=>{c.closed=true;c.closeCode=e.code;rec.push(name,'in','close',null,{code:e.code});};
  const ok=await new Promise(r=>{ws.onopen=()=>r(true);ws.onerror=()=>r(false)});
  if(!ok){rec.push(name,'in','connect-failed',null,{});c.failed=true;if(!expectFail)throw Error('websocket connect failed for '+name+' at '+url.replace(/token=[^&]*/,'token=…'));return c}
  c.send=(obj)=>{const bytes=encode(obj);rec.push(name,'out','bin',bytes,JSON.parse(JSON.stringify(obj)));ws.send(bytes)};
  c.sendRaw=(bytes)=>{rec.push(name,'out','bin',bytes,{raw:true});ws.send(bytes)};
  c.until=async(pred,ms=15000)=>{const end=Date.now()+ms;for(;;){const f=c.inbox.find(pred);if(f)return f;if(Date.now()>end)return null;await sleep(10)}};
  c.rpc=async(m,p,ms=15000)=>{const id=++c.id;c.send({id,m,p});return c.until(x=>x.t==='reply'&&x.id===id,ms)};
  // a poll that leaves no trace in the client's counters, and is marked volatile (compared by nobody)
  c.poll=async(m='view.update',extra={})=>{const v=c.view,was=rec.vol;rec.vol=true;try{return await c.vcall(m,extra)}finally{rec.vol=was}};
  // after a variable number of polls both counters jump to the next thousand, so what follows is the same in every run
  c.resync=()=>{const v=c.view;v.rev=(Math.floor(v.rev/1000)+1)*1000;v.next=(Math.floor((v.next-2**47)/1000)+1)*1000+2**47};
  c.end=async(quiet=1200)=>{rec.push(name,'out','end',null,{quiet});await c.settle(quiet,10000);c.ws.close();const e=Date.now()+3000;while(!c.closed&&Date.now()<e)await sleep(20)};
  c.settle=async(quiet=400,max=8000)=>{rec.push(name,'out','settle',null,{quiet});let last=c.inbox.length,t=Date.now();for(;;){await sleep(50);if(c.inbox.length!==last){last=c.inbox.length;t=Date.now()}else if(Date.now()-t>=quiet)return;if(Date.now()-t>max)return}};
  // ---- view protocol (the same request shape the clients send) ----
  c.vreq=(m,extra={})=>{const v=c.view;const id=++v.next;const p={...(P.extra||{}),...extra,version:1,hub:hub.url,generation:c.greet.generation,epoch:c.greet.epoch,requestRevision:++v.rev,target:v.target,viewId:v.viewId,session:v.session,cid:'parity-'+name};
    if(P.delta&&!v.noOffer&&v.token)p.viewDelta={version:1,token:v.token};
    c.send({id,m,p});return id};
  c.vcall=async(m,extra={},ms=20000)=>{const id=c.vreq(m,extra);const r=await c.until(x=>x.t==='reply'&&x.id===id,ms);if(r?.ok&&r.result){const res=r.result;
      const vd=res.viewDelta;if(vd){if(vd.ops){const base=c.view.bases.get(vd.base);if(!base)throw Error('delta against a base the client does not hold: '+vd.base);Object.assign(res,applyViewDelta(base,vd.ops))}
        c.view.bases.set(vd.token,{shown:res.shown,session:res.session,window:res.window,overlay:res.overlay});c.view.token=vd.token;if(c.view.bases.size>8)c.view.bases.delete(c.view.bases.keys().next().value)}if(res.session)Object.assign(c.view.session,res.session);if(res.viewId)c.view.viewId=res.viewId;c.view.last=res;
      for(const cm of res.cmds||[])if(cm.type==='send'){try{const q=JSON.parse(cm.text);c.send(q)}catch{}}
      if(res.nextTarget&&JSON.stringify(res.nextTarget)!==JSON.stringify(c.view.target)){c.view.target=res.nextTarget;c.view.viewId='';return c.vcall('view.open',{},ms)}}return r};
  if(P.view){await c.until(x=>x.t==='view',10000)}
  return c;
}

// an upgrade that should be refused (or opened): recorded as a connect step with its token mode
export async function attempt(hub,rec,name,mode){
  rec.push(name,'out','connect',null,{mode});
  const tok={none:'',wrong:'x'.repeat(32),badchars:'../../etc/passwd',empty:''}[mode];
  const url=`ws://${hub.host}:${hub.port}/ws?${mode==='none'?'':'token='+encodeURIComponent(tok)}`;
  const r=await new Promise(res=>{const ws=new WebSocket(url);let done=false;ws.onopen=()=>{done=true;ws.close();res('opened')};ws.onerror=()=>{if(!done)res('refused')}});
  rec.push(name,'in','upgrade',null,{result:r});return r;
}

// Lines no client may ever be sent (scenario `private`): the sentinels the fixture puts in device tokens, riding mail and turn inputs.
// Every inbound frame is searched as raw bytes (CBOR text is stored as UTF-8), so a field the decoder or a normalisation drops cannot hide one.
const PRIV=/PRIV(TOKEN|DROPPED|RIDE|LAUNCH|FALLBACK|LIVE)/;
export function privateLeaks(frames){
  const out=[];
  for(const f of frames){if(f.dir!=='in'||f.raw==null)continue;
    const s=f.kind==='bin'?Buffer.from(f.raw,'base64').toString('latin1'):String(f.raw);
    const m=PRIV.exec(s);if(m)out.push({c:f.c,i:f.i,kind:f.kind,marker:m[0]});}
  return out;
}
