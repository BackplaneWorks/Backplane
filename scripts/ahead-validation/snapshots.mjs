// Saved synthetic production trees for layout/calibration; never user data.
import {mkdirSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {json,plain} from '../../src/web/wire.js';
import {replay} from '../../test/tools/ahead_validation.mjs';
const F=(await import(pathToFileURL(resolve(process.argv[2])))).default,out=resolve(process.argv[3]);
mkdirSync(out,{recursive:true});
for(const mode of ['idle','running','long-label']){
  let u=replay(F,[{kind:'act',action:'select',value:'A'},{kind:'act',action:'draft',value:'Synthetic draft'}]).u;
  if(mode==='running')u=F.ui(F.received(u,json({t:'changes',items:[{$:'TurnChanged',thread:'A',state:'running',at:5}]})));
  if(mode==='long-label')u=F.ui(F.received(u,json({t:'changes',items:[{$:'ModesSet',id:'A',model:'a-very-long-public-synthetic-model-name-'.repeat(10),runtime:'',interaction:'',at:5}]})));
  writeFileSync(resolve(out,mode+'.json'),JSON.stringify(plain(F.composer(u)),null,2)+'\n');
}
