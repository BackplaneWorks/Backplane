// usage: bun set-setting.mjs HOME HOST:PORT KEY VALUE   (a plain RPC to a hub of our own)
import {readFileSync} from 'node:fs';
import {encode} from './wire.mjs';
const [home,hp,key,value]=process.argv.slice(2);
const token=readFileSync(home+'/token','utf8').trim();
const ws=new WebSocket(`ws://${hp}/ws?token=${token}`);ws.binaryType='arraybuffer';
ws.onopen=()=>{ws.send(encode({id:7,m:'setting.set',p:{key,value}}));setTimeout(()=>process.exit(0),800)};
ws.onerror=()=>process.exit(1);
