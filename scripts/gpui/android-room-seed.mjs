// A room with bots and a post from the person on a stand-in hub: bun android-room-seed.mjs HOME HOST:PORT
import {readFileSync} from 'node:fs';
import {encode} from '/home/h/Documents/i2cjak/backplane-gpui/test/gpui/wire.mjs';
const [home,hp]=process.argv.slice(2);
const token=readFileSync(home+'/token','utf8').trim();
const ws=new WebSocket(`ws://${hp}/ws?token=${token}`);ws.binaryType='arraybuffer';
let id=700;const send=(m,p)=>ws.send(encode({id:++id,m,p}));
const events=()=>readFileSync(home+'/events.jsonl','utf8').split('\n').filter(l=>l.includes('"$":"RoomSet"')).map(l=>JSON.parse(l));
ws.onopen=async()=>{
  send('bots.room',{name:'den',members:'Bub,Mochi,Pixel,Ember'});await new Promise(r=>setTimeout(r,1500));
  const room=events().filter(e=>e.name==='den').pop()?.id;console.log('room',room);
  send('bots.post',{room,text:'hello den, who is here?'});
  setTimeout(()=>process.exit(0),4000);};
ws.onerror=()=>process.exit(1);
