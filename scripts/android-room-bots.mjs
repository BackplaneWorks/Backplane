// Each bot posts into the room "den" (stand-in agents calling room_post): bun android-room-bots.mjs HOME HOST:PORT
import {readFileSync} from 'node:fs';
import {encode} from '../test/wire.mjs';
const [home,hp]=process.argv.slice(2);
const token=readFileSync(home+'/token','utf8').trim();
const ev=k=>readFileSync(home+'/events.jsonl','utf8').split('\n').filter(l=>l.includes(`"$":"${k}"`)).map(l=>JSON.parse(l));
const room=ev('RoomSet').filter(e=>e.name==='den').pop().id, bot=n=>ev('BotSet').filter(e=>e.name===n).pop().id;
const ws=new WebSocket(`ws://${hp}/ws?token=${token}`);ws.binaryType='arraybuffer';
let id=800;const send=(m,p)=>ws.send(encode({id:++id,m,p}));
ws.onopen=async()=>{for(const [n,t] of [['Bub','Bub here, ready.'],['Mochi','Mochi here, working on it.'],['Pixel','Pixel here, something failed.'],['Ember','Ember here too.']]){
  send('bots.say',{bot:bot(n),text:`mode:plain roompost:${room} ${t}`});await new Promise(r=>setTimeout(r,2500));}
  setTimeout(()=>process.exit(0),3000);};
ws.onerror=()=>process.exit(1);
