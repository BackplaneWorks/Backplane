// bun chain-verify.mjs HOME CHAINDIR HOP : verifyLog over the whole home with every acked write (project root AND its thread) of every hop's ledger.
// Exit 5 when anything fails. A missing hop ledger or an unparseable row is a failure, never skipped.
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {verifyLog} from './verify.mjs';
const [home,chain,hopArg]=process.argv.slice(2);const hop=Number(hopArg);
const acked=new Set(),threads=new Set();let failed=0;const problems=[];
for(let h=1;h<=hop;h++){const f=`${chain}/ledger-hop${h}.jsonl`;if(!existsSync(f)){problems.push('missing ledger '+f);continue}
 for(const l of readFileSync(f,'utf8').split('\n')){if(!l)continue;let e;try{e=JSON.parse(l)}catch{problems.push(`unparseable ledger row in hop ${h}: ${l.slice(0,60)}`);continue}
  if(e.failed)failed++;else if(typeof e.path==='string'){acked.add(e.path);if(e.thread)threads.add(e.thread)}else problems.push(`bad ledger row in hop ${h}`)}}
const v=await verifyLog(home,{acked,ackedThreads:threads,expectFragments:hop});
const r={hop,lines:v.lines,valid:v.valid,bad:v.bad.length,badSample:v.bad.slice(0,3),tail:v.tail,acked:acked.size,ackedThreads:threads.size,ackFailed:failed,missing:v.missing.length,missingSample:v.missing.slice(0,5),missingThreads:v.missingThreads.length,multi:v.multi.length,dupEvents:v.dups.length,dupSample:v.dups.slice(0,3),idCollisions:v.idDups.length,problems,
 ok:problems.length===0&&v.ok&&v.bad.length<=hop};
writeFileSync(`${chain}/verify-latest.json`,JSON.stringify(r,null,1));console.log(JSON.stringify(r));process.exit(r.ok?0:5);
