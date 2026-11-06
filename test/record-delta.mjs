// Records view-delta frames from a hub of our own: usage: bun record-delta.mjs HOME HOST:PORT OUT.json
// Each step: the reply to a request offering the newest token ("delta"), then the whole body of the same state
// (offering nothing), so a test can check applying the delta equals the whole.
import {readFileSync,writeFileSync} from 'node:fs';
import {decode,encode} from './wire.mjs';
const [home,hp,out]=process.argv.slice(2);
const token=readFileSync(home+'/token','utf8').trim(),H='http://'+hp;
const ws=new WebSocket(`ws://${hp}/ws?view=1&mobile=1&token=${token}`);ws.binaryType='arraybuffer';
let g,rev=0,id=2**47+10,target={kind:'recent',id:''},viewId='',session={sel:'',rid:Date.now(),drafts:{},open:{},adding:false,panel:false,info:{}},newest='';
const frames=[];let waiting=null;
const req=(m,extra,offer)=>new Promise(res=>{const my=++id,r=++rev;waiting={my,res};
  ws.send(encode({id:my,m,p:{version:1,hub:H,generation:g.generation,epoch:g.epoch,requestRevision:r,target,viewId,session,cid:'rec',nativeSidebar:1,viewDelta:{version:1,token:offer},...extra}}));});
ws.onmessage=({data})=>{const x=decode(data);
 if(x.t==='view'){g=x;go();return}
 if(x.t==='reply'&&waiting&&x.id===waiting.my){const w=waiting;waiting=null;w.res(x)}};
async function step(name,m,extra){
  const offered=newest;
  const d=await req(m,extra,newest);
  if(d.result?.nextTarget){target=d.result.nextTarget;viewId='';return step(name,'view.open',{})}
  if(d.result?.session)session=d.result.session;if(d.result?.viewId)viewId=d.result.viewId;
  const w=await req('view.update',{},'');
  if(w.result?.session)session=w.result.session;
  newest=w.result.viewDelta.token;
  frames.push({name,method:m,offered,delta:d.result,whole:w.result});
}
async function go(){
  await step('open','view.open',{});
  const t=frames[0].delta.shown?.projects?.[0]?.threads?.[0]?.id||frames[0].whole.shown.projects[0].threads[0].id;
  await step('select','view.action',{action:'select',value:t});
  await step('update-same','view.update',{});
  await step('draft-slash','view.action',{action:'draft',value:'/'});
  await step('draft-slash-b','view.action',{action:'draft',value:'/b'});
  writeFileSync(out,JSON.stringify({hub:H,frames}));console.log('frames',frames.length,frames.map(f=>[f.name,f.delta.viewDelta?.ops?f.delta.viewDelta.ops.length+' ops':'whole',JSON.stringify(f.delta).length,JSON.stringify(f.whole).length]));process.exit(0);
}
setTimeout(()=>{console.error('timeout');process.exit(2)},20000);
