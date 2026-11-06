// Record scenarios against a hub binary. usage:
//   bun record.mjs --bin BIN --out DIR --port 3820 [--only a,b] [--tag A]
// A fresh fixture home and a fresh hub per scenario; stand-in agents only, verified from /proc/<pid>/environ.
import {mkdirSync,writeFileSync,readFileSync,rmSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {makeStandins,startHub,stopHub} from '../perf/lib.mjs';
import {Rec,sleep} from './lib.mjs';
import {SCENARIOS} from './scenarios.mjs';
import {startFixtureHub} from './hubs.mjs';
import {R,WORK} from './hubs.mjs';
const arg=(k,d)=>{const i=process.argv.indexOf('--'+k);return i>0?process.argv[i+1]:d};
const bin=arg('bin',R+'/build/gpui/backplane-serve'),out=arg('out',R+'/build/gpui/parity/rec'),port=Number(arg('port',3820)),tag=arg('tag','A');
const only=(arg('only','')||'').split(',').filter(Boolean);
const ALL=['join','badauth','session:mobile','session:gpui','session:gpuid','reconnect','unresolved','settings','private','commands','bots','rooms','projecthome','settingspages'];
mkdirSync(out,{recursive:true});
export async function runOne(name,{bin,port,work}){
  const h=await startFixtureHub({bin,port,work,deleted:name==='unresolved',privateLines:name==='private'});
  const rec=new Rec();const [s,arg1]=name.split(':');
  await (arg1?SCENARIOS[s].bind(null,arg1):SCENARIOS[s])({hub:h,rec});
  const sessions=await h.finish();
  const bad=rec.frames.filter(f=>f.kind==='check'&&f.json.deltaRebuildEqualsWhole===false);
  const refused=rec.frames.filter(f=>f.kind==='check'&&f.json.viewRefused);
  if(refused.length)throw Error('a scenario view was refused, so it compared nothing: '+JSON.stringify(refused[0].json));
  const leaks=rec.frames.filter(f=>f.kind==='check'&&f.json.privateLeaks?.length);
  if(leaks.length)throw Error('private lines reached a client: '+JSON.stringify(leaks[0].json.privateLeaks.slice(0,5)));
  if(bad.length)throw Error('keyed delta: the body rebuilt from ops differs from the whole view');
  return {meta:{scenario:name,bin,home:'<home>',check:h.check,sessions,frames:rec.frames.length,hubUrl:h.url,work:h.work,port},frames:rec.frames};
}
if(import.meta.main){
  for(const n of ALL){if(only.length&&!only.includes(n))continue;
    const work=`${WORK}/work-${tag}`;
    const r=await runOne(n,{bin,port,work});
    writeFileSync(`${out}/${n.replace(':','-')}.json`,JSON.stringify(r));
    console.log(n,'frames',r.frames.length);
  }
  process.exit(0);
}
