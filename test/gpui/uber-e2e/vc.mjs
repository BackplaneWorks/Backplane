// The GPUI client's view protocol, enough to open a target and read what the hub answers. Used by e2e.mjs.
import {decode,encode} from '../wire.mjs';
import {sleep} from '../perf/lib.mjs';
export async function viewClient(h){
 const ws=new WebSocket(`ws://127.0.0.1:${h.port}/ws?view=1&mobile=1&enc=cbor&token=${encodeURIComponent(h.token)}`);ws.binaryType='arraybuffer';
 let greet,rev=0,next=2**47+100;const replies=new Map();
 const c={target:{kind:'recent',id:''},viewId:'',session:{sel:'',rid:Date.now(),drafts:{},open:{},adding:false,panel:false,info:{}},frames:[]};
 ws.onmessage=({data})=>{let x;try{x=typeof data==='string'?JSON.parse(data):decode(data)}catch{return}
  c.frames.push(x);if(x.t==='view')greet=x;
  if(x.t==='reply'){replies.set(x.id,x);if(x.ok&&x.result?.session)Object.assign(c.session,x.result.session);if(x.ok&&x.result?.viewId)c.viewId=x.result.viewId}};
 await new Promise((r,j)=>{ws.onopen=r;ws.onerror=()=>j(new Error('ws error'))});
 for(let i=0;i<500&&!greet;i++)await sleep(10);
 if(!greet)throw Error('no greeting');
 c.ask=async(m,extra={},ms=30000)=>{const id=++next;const p={nativeSidebar:1,accountProfiles:1,...extra,version:1,hub:h.url,generation:greet.generation,epoch:greet.epoch,requestRevision:++rev,target:c.target,viewId:c.viewId,session:c.session,cid:'e2e'};
  ws.send(encode({id,m,p}));const end=Date.now()+ms;while(!replies.has(id)&&Date.now()<end)await sleep(5);return replies.get(id)};
 c.open=async t=>{c.target=t;c.viewId='';return c.ask('view.open')};
 c.close=()=>ws.close();
 return c;
}
