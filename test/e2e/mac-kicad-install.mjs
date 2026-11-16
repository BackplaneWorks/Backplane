// Settings > CAD > Install, as the person presses it on a Mac: node mac-kicad-install.mjs HUB WORK
// A hub on a fresh home with no KiCad (BACKPLANE_KICAD_NO_SYSTEM=1) runs its own install driver (kicad.install {consent:true}): the pinned
// darwin-arm64 package fetched, checked against the release's SHA256SUMS and the pin, unpacked and moved into <home>/kicad/<version>.
// Prints `RT=<the installed runtime folder>` last; exits non-zero when it did not install.
import {existsSync,mkdirSync,readFileSync,readdirSync,rmSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {startHub,stopHub,client,makeStandins,sleep} from '../perf/lib.mjs';
const [bin,work]=process.argv.slice(2);process.env.NO_SCOPE='1';
rmSync(work,{recursive:true,force:true});mkdirSync(`${work}/user`,{recursive:true});makeStandins(`${work}/bin`); // the hub runs stand-in agents only
const h=await startHub({bin,home:`${work}/home`,port:3895,standins:`${work}/bin`,log:`${work}/hub.log`,extraEnv:{HOME:`${work}/user`,BACKPLANE_KICAD_NO_SYSTEM:'1'},unsetEnv:['BACKPLANE_KICAD_CLI']});
let bad=0;const check=(n,ok,x)=>{console.log(`${ok?'ok  ':'FAIL'} ${n}${ok?'':' '+JSON.stringify(x)?.slice(0,500)}`);if(!ok)bad++};
const c=await client(h,{});
const refused=await c.rpc('kicad.install',{consent:false},30000);
check('without consent the install is refused, naming the version and where it comes from',refused?.o?.ok===false&&/consent/.test(JSON.stringify(refused?.o)),refused?.o);
const t0=Date.now();const r=await c.rpc('kicad.install',{consent:true},60000);
check('Install starts',r?.o?.ok===true,r?.o);
const kd=`${h.home}/kicad`;let rt='';
for(let i=0;i<180&&!rt;i++){await sleep(5000);
  for(const v of existsSync(kd)?readdirSync(kd):[]){if(!v.startsWith('.')&&existsSync(`${kd}/${v}/bin/kicad-cli`))rt=`${kd}/${v}`}}
const mins=((Date.now()-t0)/60000).toFixed(1);
check(`the runtime is installed by the hub (${mins} min)`,!!rt,existsSync(`${kd}/.install.log`)?readFileSync(`${kd}/.install.log`,'utf8').slice(-1500):'no install log');
if(rt){
  const q=execFileSync('xattr',['-r',rt],{encoding:'utf8'}).split('\n').filter(l=>/quarantine/.test(l)).length;check('nothing is quarantined',q===0,q);
  const m=JSON.parse(readFileSync(`${rt}/manifest.json`,'utf8'));console.log(`manifest: ${m.version} ${m.platform} minOs ${m.minOs} ${m.capabilities.length} capabilities`);
  for(const t of ['kicad-cli','kicad','pcbnew','eeschema'])check(`bin/${t} is there`,existsSync(`${rt}/bin/${t}`));
  check('bin/kicad-cli runs',/10\.99/.test(execFileSync(`${rt}/bin/kicad-cli`,['--version'],{encoding:'utf8'})));
  try{execFileSync('codesign',['--verify','--deep','--strict',`${rt}/KiCad.app`]);check('KiCad.app\'s signature verifies',true)}catch(e){check('KiCad.app\'s signature verifies',false,String(e.stderr||e))}
  const info=await c.rpc('kicad.info',{},30000);console.log('kicad.info:',JSON.stringify(info?.o).slice(0,300));
}
stopHub(h);console.log(bad?`${bad} FAILED`:'all passed');if(rt)console.log(`RT=${rt}`);process.exit(bad?1:0);
