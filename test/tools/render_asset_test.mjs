// Exercise the real host effects only on a temporary project/cache. No hub,
// native CAD editor, installed service, network or authentication is involved.
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createHash} from 'node:crypto';
const dir=fs.mkdtempSync(join(tmpdir(),'bp-render-asset-')),project=join(dir,'project'),outside=join(dir,'outside'),cache=join(dir,'cache');
fs.mkdirSync(project);fs.mkdirSync(outside);
const originalXdg=process.env.XDG_CACHE_HOME;process.env.XDG_CACHE_HOME=cache;
const effects=new Map();let readHook=null;
const hookedFs={...fs,readSync(...args){const n=fs.readSync(...args);readHook?.(...args);return n;}};
const code=fs.readFileSync('src/server/effects/render_asset.js','utf8').replace(/CID\(([^)]+)\)/g,'"$1"');
new Function('require','io_eff',code)(n=>n==='node:fs'?hookedFs:require(n),(name,fn)=>effects.set(name,fn));
const call=(name,...args)=>JSON.parse(effects.get('RenderAssetFx.'+name)(...args));
const revision=p=>{const s=fs.statSync(p,{bigint:true});return[s.dev,s.ino,s.size,s.mtimeNs/1000000000n,s.mtimeNs%1000000000n,s.ctimeNs/1000000000n,s.ctimeNs%1000000000n].join(':');};
const digest=b=>createHash('sha256').update(b).digest('hex');
function glb(extra={},fill=0,binSize=524288) {
 const json=Buffer.from(JSON.stringify({asset:{version:'2.0',generator:'test',...extra},nodes:[{name:'R1',mesh:0}],materials:[{pbrMetallicRoughness:{baseColorFactor:[.1,.2,.3,1],metallicFactor:.7,roughnessFactor:.25}}],buffers:[{byteLength:binSize}]}));
 const n=Math.ceil(json.length/4)*4,b=Buffer.alloc(12+8+n+8+binSize,fill);b.writeUInt32LE(0x46546c67,0);b.writeUInt32LE(2,4);b.writeUInt32LE(b.length,8);b.writeUInt32LE(n,12);b.writeUInt32LE(0x4e4f534a,16);b.fill(32,20,20+n);json.copy(b,20);b.writeUInt32LE(binSize,20+n);b.writeUInt32LE(0x004e4942,24+n);return b;
}
const source=join(project,'source.glb'),board=join(project,'main.kicad_pcb'),step=join(project,'part.step'),exported=join(dir,'export.glb');
const bytes=glb();fs.writeFileSync(source,bytes);fs.writeFileSync(board,'(kicad_pcb (version 20250114))');fs.writeFileSync(step,'ISO-10303-21; END-ISO-10303-21;');fs.writeFileSync(exported,bytes);
const publish=(s=source,g=s,k='glb',canonical=board,rev=revision(s))=>call('publish',project,s,canonical,k,rev,g,'');
try {
 const a=call('authorize',project,source,board,'mech',revision(source));assert.equal(a.source,source);assert.equal(a.kind,'glb');
 assert(call('authorize',project,source,board,'mech','').error);assert(call('authorize',project,source,board,'invalid',revision(source)).error);
 const escape=join(outside,'model.glb');fs.writeFileSync(escape,bytes);
 assert(call('authorize',project,'../outside/model.glb',board,'mech',revision(escape)).error,'path traversal refused');
 fs.symlinkSync(escape,join(project,'escape.glb'));assert(call('authorize',project,'escape.glb',board,'mech',revision(escape)).error,'symlink escape refused');
 const externalBoard=join(outside,'configured.kicad_pcb');fs.writeFileSync(externalBoard,'(kicad_pcb)');
 assert.equal(call('authorize',project,externalBoard,externalBoard,'3d',revision(externalBoard)).source,externalBoard,'exact explicitly configured external board is allowed');
 assert(call('authorize',project,externalBoard,board,'3d',revision(externalBoard)).error);
 assert(call('authorize',project,step,board,'mech','old revision').error);
 const originalStat=revision(source),one=publish();assert(one.asset,JSON.stringify(one));
 assert.equal(one.asset.bytes,bytes.length);assert.equal(one.asset.sha256,digest(bytes));assert.equal(one.asset.orientation,'gltf-y-up');assert.equal(one.asset.revision,originalStat);
 const lookup=call('lookup',one.asset.sha256);assert.equal(lookup.size,bytes.length);assert.deepEqual(fs.readFileSync(lookup.path),bytes,'original authored metadata/binary bytes preserved');assert.equal(revision(source),originalStat,'source unchanged');assert.equal(fs.statSync(lookup.path).mode&0o222,0);assert.equal(fs.statSync(join(cache,'backplane-bend/render-assets')).mode&0o077,0);
 const firstRevision=revision(lookup.path),again=publish();assert.equal(again.asset.sha256,one.asset.sha256);assert.equal(revision(lookup.path),firstRevision,'cache hit does not overwrite immutable file');
 const memo=(k='glb',rev=revision(source))=>effects.get('RenderAssetFx.memo')(project,k,rev);
 assert.deepEqual(JSON.parse(memo()).asset,again.asset,'a published revision is answered from the memo');
 assert.equal(memo('mech'),'','another kind misses');assert.equal(memo('glb',''),'','no revision misses');assert.equal(memo('glb','0:0:0:0:0:0:0'),'','another revision misses');
 assert.equal(effects.get('RenderAssetFx.memo')(outside,'glb',revision(source)),'','another project misses');
 {const cached=join(cache,'backplane-bend','render-assets',again.asset.sha256+'.glb');fs.chmodSync(cached,0o644);assert.equal(memo(),'','a writable cached copy misses');fs.chmodSync(cached,0o444);assert.ok(memo());}
 {const before=revision(source);fs.utimesSync(source,new Date(),new Date(Date.now()+5000));assert.notEqual(revision(source),before);assert.equal(memo('glb',before),'','a source no longer at the revision misses');assert.equal(memo(),'','its new revision was never published');}
 const stepped=glb({extras:{unit:'mm',up:'Z'}},2);fs.writeFileSync(exported,stepped);const b=publish(step,exported,'mech');assert.equal(b.asset.orientation,'step-z-up-mm');assert.equal(b.asset.sha256,digest(stepped));
 for(const id of ['../source.glb','A'.repeat(64),'0'.repeat(63),one.asset.sha256+'.glb'])assert(call('lookup',id).error);
 assert(call('lookup','f'.repeat(64)).error,'missing id has no arbitrary path fallback');
 const privateDir=join(cache,'backplane-bend/render-assets'),linkId='e'.repeat(64);fs.symlinkSync(source,join(privateDir,linkId+'.glb'));assert(call('lookup',linkId).error);fs.unlinkSync(join(privateDir,linkId+'.glb'));
 fs.linkSync(source,join(privateDir,linkId+'.glb'));assert(call('lookup',linkId).error);fs.unlinkSync(join(privateDir,linkId+'.glb'));
 const corrupt=join(project,'corrupt.glb');for(const bad of [Buffer.from('not a GLB'),Buffer.from(bytes)]){if(bad.length>20)bad.writeUInt32LE(23,8);fs.writeFileSync(corrupt,bad);assert(publish(corrupt).error);}
 const huge=join(project,'huge.glb');fs.writeFileSync(huge,bytes.subarray(0,20));fs.truncateSync(huge,64*1024*1024+1);assert(publish(huge).error);
 const before=revision(board);fs.writeFileSync(exported,bytes);
 readHook=(fd,b,offset,length,pos)=>{if(length===262144){readHook=null;fs.writeFileSync(board,'(kicad_pcb (version 20250115))');}};
 assert.match(publish(board,exported,'3d',board,before).error,/revision changed/,'source change during byte copy refused');
 const originalExport=revision(exported);
 readHook=(fd,b,offset,length,pos)=>{if(length===262144){readHook=null;const f=fs.openSync(exported,'r+');try{fs.writeSync(f,Buffer.from([12]),0,1,bytes.length-1);}finally{fs.closeSync(f);}}};
 assert.match(publish(board,exported,'3d').error,/changed while publishing/,'export file change during byte copy refused');assert.notEqual(revision(exported),originalExport);
 readHook=null;assert.equal(fs.readdirSync(privateDir).filter(n=>n.startsWith('.asset-')).length,0,'failed work cleans its own staging files');
 // Repeated distinct assets stay bounded; only this module's SHA-named files
 // are retired. The current asset remains available even on a reused old id.
 const keep=join(privateDir,'person-notes.txt');fs.writeFileSync(keep,'keep');let last;
 for(let i=0;i<10;i++){fs.writeFileSync(source,glb({},i,16));last=publish();assert(last.asset);}
 assert(fs.readdirSync(privateDir).filter(n=>/^[0-9a-f]{64}\.glb$/.test(n)).length<=8);assert(call('lookup',last.asset.sha256).path);assert.equal(fs.readFileSync(keep,'utf8'),'keep');
 // Read/permission failures yield bounded structured errors, never stdout
 // stacks or binary contents. Authentication belongs to the root HTTP route.
 fs.unlinkSync(source);assert(publish(source,exported,'glb',board,'missing').error);assert(Buffer.byteLength(JSON.stringify(call('lookup','d'.repeat(64))))<1000);
 console.log('ok real render-asset effects: root/canonical authorization, exact revisions, original GLB bytes+SHA, orientation, immutable private cache, source/export races, bad headers, no links/traversal, size/retention bounds and cleanup');
} finally {readHook=null;if(originalXdg===undefined)delete process.env.XDG_CACHE_HOME;else process.env.XDG_CACHE_HOME=originalXdg;fs.rmSync(dir,{recursive:true,force:true});}
