// Put seeded bots in moods on a stand-in hub: bun android-bots-moods.mjs HOME HOST:PORT
// work = a turn the stand-in holds (mode:pair, BP_WORK_MS), error = the agent exits (mode:exit-now), idle = a finished turn.
import {readFileSync} from 'node:fs';
import {encode} from '/home/h/Documents/i2cjak/backplane-gpui/test/wire.mjs';
const [home,hp]=process.argv.slice(2);
const token=readFileSync(home+'/token','utf8').trim();
const ev=readFileSync(home+'/events.jsonl','utf8').split('\n').filter(l=>l.includes('"$":"BotSet"')).map(l=>JSON.parse(l));
const bot=n=>ev.filter(e=>e.name===n).pop()?.id;
const plan=[['Mochi','mode:pair keep working'],['Pixel','mode:exit-now'],['Ember','mode:plain hello'],['Nori','mode:plain hi there']];
const ws=new WebSocket(`ws://${hp}/ws?token=${token}`);ws.binaryType='arraybuffer';
let id=500;const send=(m,p)=>ws.send(encode({id:++id,m,p}));
ws.onopen=async()=>{for(const [n,t] of plan){send('bots.say',{bot:bot(n),text:t});await new Promise(r=>setTimeout(r,2500));}
  setTimeout(()=>process.exit(0),3000);};
ws.onerror=()=>process.exit(1);
