// S09 Voice on the real engine: a recording dictated into a thread the way a window sends its microphone (bots.voice start with src=client, the audio POSTed to /voice/in),
// heard by backplane-voice's local model on this machine, then the same recording again after Settings > Voice > Dictionary gets a line that rewrites a word it heard.
//   bun test/e2e/voice-effect.mjs HUB_BIN OUT_DIR WAV [PORT]   WAV: 16 kHz mono 16-bit speech (Windows: test/windows/rig/say.ps1 makes one)
// backplane-voice must sit beside the hub (where the hub looks for it). The local model is downloaded into the temp home (bots.voice install, the page's Download button).
// A check passes only from what the hub pushed to this client (the edits a window applies to its draft) and the hub's own settings; one line per check, OUT_DIR/voice.json.
import {mkdirSync,rmSync,writeFileSync,readFileSync,existsSync,copyFileSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep,client} from '../perf/lib.mjs';
import {sock,idof,waitFor} from './lib/hubrows.mjs';
const [bin,out,wav,port0]=process.argv.slice(2);const port=Number(port0||3875);process.env.NO_SCOPE??='1';
rmSync(out,{recursive:true,force:true});mkdirSync(out+'/proj',{recursive:true});makeStandins(out+'/bin');
const res=[];const rec=(id,control,ok,evidence)=>{const o={id,control,ok,evidence};res.push(o);console.log(JSON.stringify(o))};
const hub=await startHub({bin,home:out+'/home',port,standins:out+'/bin',log:out+'/hub.log'});
for(const ev of ['uncaughtException','unhandledRejection'])process.on(ev,e=>{console.error(e);stopHub(hub);process.exit(1)});
const frames=[];const k=await client(hub,{onMsg:o=>{if(o.t==='voice')frames.push(o)}});
const voice=async(p,ms=20000)=>{const o=(await k.rpc('bots.voice',p,ms))?.o||{ok:false,error:'no reply'};return {...o,result:o.result??o}}; // bots.voice answers at the top level of the reply
const c=await sock(hub,port);const P=idof((await c.rpc('project.add',{path:out+'/proj'})).result?.project);
const T=idof((await c.rpc('thread.create',{project:P,title:'dictation'})).result?.thread);
// the page's chips: On this machine, then Download for the model it names
// VAD_SEED=<silero_vad.onnx>: that file in place first, so its download is skipped (when GitHub's release files answer 504 for hours); the helper still fetches the runtime and the model
if(process.env.VAD_SEED){mkdirSync(out+'/home/voice/local',{recursive:true});copyFileSync(process.env.VAD_SEED,out+'/home/voice/local/silero_vad.onnx')}
const eng=await voice({op:'page-act',action:'setting',value:'voice.engine=local'});
const t0=Date.now();const inst=await voice({op:'install',model:'parakeet-v2'});
const failed=()=>frames.filter(f=>f.ev==='page').map(f=>(f.rows||[]).find(r=>r.label==='Local model')?.note||'').find(n=>/Download failed/.test(n));
const ready=await waitFor(()=>{try{return readFileSync(out+'/home/voice/local/ready-parakeet-v2','utf8').trim()==='ok'}catch{return !!failed()}},20*60000,2000)&&!failed();
rec('S09','engine On this machine, model Download',eng.ok&&inst.ok&&!!ready,`voice.engine=local ok=${eng.ok}; install ok=${inst.ok} ${inst.error||''}; the model ready after ${((Date.now()-t0)/1000).toFixed(0)} s: ${!!ready}${failed()?' — the page says: '+failed():''}${process.env.VAD_SEED?' (VAD seeded)':''}`);
if(!ready){writeFileSync(out+'/voice.json',JSON.stringify(res,null,1));k.ws.close();c.ws.close();stopHub(hub);process.exit(1)}
const pcm=(()=>{const b=readFileSync(wav);let i=12;while(i+8<=b.length){const id=b.toString('ascii',i,i+4),n=b.readUInt32LE(i+4);if(id==='fmt '){const ch=b.readUInt16LE(i+10),rate=b.readUInt32LE(i+12),bits=b.readUInt16LE(i+22);if(ch!==1||rate!==16000||bits!==16)throw Error(`the WAV is ${ch} ch ${rate} Hz ${bits} bit, not 16 kHz mono 16-bit`)}if(id==='data')return b.subarray(i+8,i+8+n);i+=8+n+(n&1)}throw Error('no data chunk in the WAV')})();
/** one dictation: start, upload the recording, end; the draft is what the hub's edits make of an empty draft (screens/voice.rs edit_range) */
async function dictate(){
  const n0=frames.length;const st=await voice({op:'start',thread:T,draft:'',caret:0,src:'client'});const path=st.result?.path;
  if(!st.ok||!path)return {ok:false,text:'',why:`start: ${st.error||JSON.stringify(st.result)}`};
  const up=await fetch(`${hub.url}${path}&end=1`,{method:'POST',headers:{Authorization:`Bearer ${hub.token}`,'Content-Type':'application/octet-stream'},body:pcm});
  const done=await waitFor(()=>frames.slice(n0).some(f=>f.ev==='view'&&f.thread===T&&f.view?.on===false&&frames.slice(n0).some(g=>g.ev==='edit')),120000,300);
  let text=[],mark=0;for(const f of frames.slice(n0).filter(f=>f.thread===T&&f.ev==='edit')){const m=Math.min(mark,text.length),end=Math.min(m+(f.del||0),text.length);text.splice(m,end-m,...Array.from(f.ins||''));mark=m+(f.adv||0)}
  const last=frames.slice(n0).filter(f=>f.ev==='view').at(-1)?.view||{};
  return {ok:up.status===200&&!!done,up:up.status,text:text.join('').trim(),engine:last.engine,err:last.err||last.error||''};
}
const a=await dictate();
rec('S09','dictation heard on this machine',a.ok&&a.text.length>0,`upload ${a.up}; heard "${a.text}"${a.why?' '+a.why:''}${a.err?' err '+a.err:''}`);
const words=(a.text.match(/[A-Za-z]{4,}/g)||[]).sort((x,y)=>y.length-x.length);const w=words[0]||'schematic';const line=`Zebrafile = ${w.toLowerCase()}`;
const add=await voice({op:'page-act',action:'voice-term',value:line});await sleep(300);
const page=await voice({op:'page'});const dict=(page.result?.rows||[]).find(r=>r.label==='Dictionary');const shown=JSON.stringify(dict?.buttons||[]).includes('Zebrafile');
const b=await dictate();
rec('S09','Dictionary line rewrites what is heard',add.ok&&shown&&b.ok&&/Zebrafile/.test(b.text)&&!new RegExp(`\\b${w}\\b`,'i').test(b.text),`added "${line}" from the page (ok=${add.ok}, listed under Dictionary ${shown}); the same recording again: "${b.text}"`);
const del=await voice({op:'page-act',action:'setting',value:(dict?.buttons||[]).find(x=>/Zebrafile/.test(x.label))?.value||'voice.e.none='}); // the line's × chip: voice.e.<key>=
const c2=await dictate();
rec('S09','the line removed, the word is heard as spoken',del.ok&&c2.ok&&!/Zebrafile/.test(c2.text),`× on the line (ok=${del.ok}); the recording a third time: "${c2.text}"`);
writeFileSync(out+'/voice.json',JSON.stringify(res,null,1));k.ws.close();c.ws.close();stopHub(hub);process.exit(res.every(r=>r.ok)?0:1);
