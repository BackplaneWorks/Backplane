// View-protocol client (the GPUI/web wire): view.open / view.update with optional keyed delta offer.
import {decode,encode} from '../wire.mjs';
import {sleep} from './lib.mjs';
export async function viewClient(h,{mobile=false,delta=true,host,tag='c'}={}){
 const q=mobile?'view=1&mobile=1':'view=1&frontend=1';
 const ws=new WebSocket(`ws://${host||h.host}:${h.port}/ws?${q}&token=${encodeURIComponent(h.token)}`);ws.binaryType='arraybuffer';
 const v={ws,greet:null,session:{sel:'',rid:Date.now(),drafts:{},open:{},adding:false,panel:false,info:{}},target:{kind:'recent',id:''},viewId:'',rev:0,id:2**47+Math.floor(Math.random()*1e6),pending:new Map(),bytesIn:0,msgsIn:0,push:0,bases:new Map(),newest:'',last:null,stats:[],onPush:null,closed:false};
 ws.onmessage=({data})=>{const n=typeof data==='string'?data.length:data.byteLength;v.bytesIn+=n;v.msgsIn++;let x;try{x=typeof data==='string'?JSON.parse(data):decode(data)}catch{return}
  if(x.t==='view'&&!v.greet){v.greet=x;v.ready?.();return}
  if(x.t==='reply'&&v.pending.has(x.id)){const p=v.pending.get(x.id);v.pending.delete(x.id);p({x,n,at:performance.now()});return}
  v.push++;v.onPush?.(x,n)};
 ws.onclose=()=>{v.closed=true};
 await new Promise((r,j)=>{ws.onopen=()=>{};ws.onerror=j;v.ready=r;setTimeout(()=>j(Error('no greeting')),10000)});
 v.req=async(m,extra={},ms=20000)=>{const id=++v.id;const rev=++v.rev;
  const p={...extra,version:1,hub:h.url,generation:v.greet.generation,epoch:v.greet.epoch,requestRevision:rev,target:v.target,viewId:v.viewId,session:v.session,cid:'perf-'+tag};
  if(!mobile){p.viewControl=1;p.windowProjection=1;p.accountProfiles=1;p.accountBinding=''}
  if(delta)p.viewDelta={version:1,token:v.newest};
  const t0=performance.now();const pr=new Promise(r=>v.pending.set(id,r));
  const body=encode({id,m,p});ws.send(body);
  const r=await Promise.race([pr,sleep(ms).then(()=>null)]);if(!r){v.pending.delete(id);return null}
  const res=r.x.result;if(r.x.ok&&res){if(res.session)Object.assign(v.session,res.session);if(res.viewId)v.viewId=res.viewId;
   const d=res.viewDelta;if(d?.token){v.newest=d.token}}
  const rec={m,ms:r.at-t0,bytes:r.n,reqBytes:body.length,delta:!!(res&&res.viewDelta&&Object.hasOwn(res.viewDelta,'ops')),ok:r.x.ok};v.stats.push(rec);v.last=r.x;return {...rec,x:r.x}};
 v.open=async()=>{let r=await v.req('view.open');for(let i=0;i<4&&r?.x?.result?.nextTarget;i++){v.target=r.x.result.nextTarget;v.viewId='';r=await v.req('view.open')}return r};
 v.close=()=>{try{ws.close()}catch{}};
 return v;
}
