// Seeds bots with several looks into a hub of our own: bun seed-bots.mjs HOME HOST:PORT
import {readFileSync} from 'node:fs';
import {encode} from './wire.mjs';
const [home,hp]=process.argv.slice(2);
const token=readFileSync(home+'/token','utf8').trim();
const ws=new WebSocket(`ws://${hp}/ws?token=${token}`);ws.binaryType='arraybuffer';
const bots=[['Bub',null],['Mochi',5],['Pixel',30],['Ember',11],['Nori',47],['Zed',66]];
let id=100;const send=(m,p)=>ws.send(encode({id:++id,m,p}));
ws.onopen=async()=>{
  for(const [n] of bots){send('bots.create',{name:n,persona:`${n} the stand-in bot`,provider:'claude',model:''});await new Promise(r=>setTimeout(r,500));}
  await new Promise(r=>setTimeout(r,1500));
  for(const [n,l] of bots) if(l!==null){send('bots.edit',{bot:n,look:l});await new Promise(r=>setTimeout(r,300));}
  setTimeout(()=>process.exit(0),1000);};
ws.onerror=()=>process.exit(1);
