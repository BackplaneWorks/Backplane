import {readFileSync,writeFileSync} from 'node:fs';
import {decode,encode} from './wire.mjs';
const home=process.env.GPUI_HOME||'/home/h/.backplane-gpui';
if(home.includes('.backplane-bend'))throw Error('live home refused');
const token=readFileSync(home+'/token','utf8').trim();
const ws=new WebSocket('ws://100.64.84.41:3799/ws?view=1&mobile=1&token='+encodeURIComponent(token));ws.binaryType='arraybuffer';
const session={sel:'',rid:Date.now(),drafts:{},open:{},adding:false,panel:false,info:{}};
let greet;let revision=0;let target={kind:'recent',id:''};let viewId='';let next=2**47+100;let made=false;let stage=0;
const request=(m,extra={})=>{const p={...extra,version:1,hub:'http://100.64.84.41:3799',generation:greet.generation,epoch:greet.epoch,requestRevision:++revision,target,viewId,session,cid:'gpui-probe'};ws.send(encode({id:++next,m,p}));};
const timer=setTimeout(()=>{console.error('probe timeout');ws.close();process.exit(1)},12000);
ws.onmessage=({data})=>{const x=typeof data==='string'?JSON.parse(data):decode(data);console.log(JSON.stringify(x).slice(0,1600));
 if(x.t==='view'){greet=x;request('view.open');}
 if(x.t==='reply'&&x.ok&&x.result?.session){Object.assign(session,x.result.session);if(x.result.nextTarget){target=x.result.nextTarget;viewId='';request('view.open');return;}}
 if(x.t==='reply'&&x.ok&&x.result?.shown){const r=x.result;viewId=r.viewId;writeFileSync('build/gpui/probe.json',JSON.stringify(r,null,2));
 if(!made){made=true;ws.send(encode({id:1,m:'project.add',p:{path:home+'/project'}}));}
 else if(stage===1){stage=2;request('view.action',{action:'select',value:r.shown.projects?.[0]?.threads?.[0]?.id||''});}
 else {clearTimeout(timer);ws.close();}}
 if(x.t==='reply'&&x.id===1){stage=1;request('view.update');}
};ws.onerror=()=>{console.error('socket error');process.exit(1)};
