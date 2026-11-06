// bun scratch_thread.mjs PORT TOKEN_FILE FOLDER -> makes a new project (folder given) and thread on the hub, then opens it as the GPUI client does. Prints ids' lengths and codes only.
import {viewClient} from '../perf/view.mjs';
import {readFileSync,mkdirSync} from 'node:fs';
import {encode} from '../wire.mjs';
const [port,tf,folder]=process.argv.slice(2); const token=readFileSync(tf,'utf8').trim(); mkdirSync(folder,{recursive:true});
const h={host:'127.0.0.1',port:Number(port),url:`http://127.0.0.1:${port}`,token};
let v;try{v=await viewClient(h,{mobile:true,delta:false})}catch(e){console.log('connect failed');process.exit(1)}
const rpc=(m,p)=>new Promise(res=>{const id=++v.id;v.pending.set(id,x=>res(x.x));v.ws.send(encode({id,m,p}));setTimeout(()=>res(null),20000)});
let r=await rpc('project.add',{path:folder}); console.log('project.add',JSON.stringify({ok:r?.ok,err:r?.error&&r.error.code||r?.error&&String(r.error).slice(0,80)}));
await new Promise(r=>setTimeout(r,500));
const ev=tf.replace(/\/token$/,'/events.jsonl');
const lastOf=(kind,filter)=>{const t=readFileSync(ev,'utf8');const ls=t.slice(-4000000).split('\n').reverse();for(const l of ls){if(l.startsWith('{"$":"'+kind+'"')&&(!filter||l.includes(filter)))return JSON.parse(l)}return null};
const pc=lastOf('ProjectCreated',folder); console.log('project id found:',!!pc);
r=await rpc('thread.create',{project:pc.id,title:'integration scratch',env:'local',provider:'claude'}); console.log('thread.create',JSON.stringify({ok:r?.ok,err:r?.error&&r.error.code}));
await new Promise(r=>setTimeout(r,500));
const tc=lastOf('ThreadCreated','integration scratch'); console.log('thread id found:',!!tc); console.log('IDS',pc.id,tc.id);
v.target={kind:'thread',id:tc.id};v.viewId='';const t=await v.req('view.open',{nativeSidebar:1},20000);
console.log('open',JSON.stringify({ok:t?.x?.ok,ms:t&&+t.ms.toFixed(1),bytes:t?.bytes,code:t?.x?.error?.code}));
v.close();process.exit(0);
