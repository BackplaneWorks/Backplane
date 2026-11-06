// Standalone preflight on a bare Xvfb: bun input/selftest.mjs  (Boat)
import {writeFileSync,mkdirSync} from 'node:fs';
import {startDisplay,installMissing} from '../lib/capture.mjs';import {startIme} from './ime.mjs';import {preflight} from './preflight.mjs';
const out='out/e2e/input-selftest';mkdirSync(out,{recursive:true});const R={install:installMissing()};
const dsp=await startDisplay(63);let ime=null;
try{ime=await startIme(dsp.display);R.preflight=await preflight({display:dsp.display,env:dsp.env,ime,out})}
catch(e){R.preflight={pass:false,failed:['crash'],error:String(e.stack||e).slice(0,400)}}
finally{try{ime?.stop()}catch{}dsp.stop()}
for(const [k,v] of Object.entries(R.preflight.checks||{}))console.log((v.ok?'ok   ':'FAIL ')+k+(v.detail?' '+JSON.stringify(v.detail).slice(0,150):''));
writeFileSync(out+'/result.json',JSON.stringify(R,null,1));console.log(R.preflight.pass?'preflight: all checks ok':'preflight: HARNESS FAILURES '+R.preflight.failed.join(','));process.exit(R.preflight.pass?0:3);
