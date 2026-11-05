// bun chain-verify.mjs HOME CHAINDIR KILLS : verifyLog over the whole home with every acked path of every hop's ledger.
import {readdirSync,readFileSync,writeFileSync} from 'node:fs';
import {verifyLog} from './verify.mjs';
const [home,chain,kills]=process.argv.slice(2);
const acked=new Set();let failed=0;
for(const f of readdirSync(chain).filter(f=>/^ledger-hop\d+\.jsonl$/.test(f)))for(const l of readFileSync(`${chain}/${f}`,'utf8').split('\n')){if(!l)continue;try{const e=JSON.parse(l);if(e.failed)failed++;else acked.add(e.path)}catch{}}
const v=await verifyLog(home,{acked,expectFragments:Number(kills)});
const r={kills:Number(kills),lines:v.lines,valid:v.valid,bad:v.bad.length,badSample:v.bad.slice(0,3),tail:v.tail,acked:acked.size,ackFailed:failed,missing:v.missing.length,missingSample:v.missing.slice(0,5),multi:v.multi.length,dupEvents:v.dups.length,dupSample:v.dups.slice(0,3),idCollisions:v.idDups.length,ok:v.missing.length===0&&v.multi.length===0&&v.dups.length===0&&v.bad.length<=Number(kills)};
writeFileSync(`${chain}/verify-latest.json`,JSON.stringify(r,null,1));console.log(JSON.stringify(r));process.exit(r.ok?0:5);
