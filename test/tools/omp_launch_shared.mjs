import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {createRequire} from 'node:module';
const require = createRequire(import.meta.url);
export function expectedArgs(mcp, mode, sessions, files, model='', effort='') {
  return ['BACKPLANE_OMP_MCP='+mcp, 'omp', '--mode', 'rpc-ui', '--approval-mode', mode,
    '--session-dir', sessions, '--continue', '-e', files+'/backplane.js', '--append-system-prompt', files+'/system-prompt.txt',
    ...(model ? ['--model',model] : []), ...(effort ? ['--thinking',effort] : [])];
}
export function launchFiles({which=()=>'/fixture/omp', files=fs}={}) {
  const effects=new Map();
  const source=fs.readFileSync('src/server/effects/omp-launch.js','utf8').replace(/CID\(([^)]+)\)/g,'"$1"');
  new Function('require','Bun','globalThis','io_eff','io_done','io_fail',source)(
    n=>n==='node:fs'?files:require(n), {which}, {BP_H:{errno:e=>e.code==='EACCES'?13:5}},
    (id,fn)=>effects.set(id,fn), value=>({ok:true,value}), errno=>({ok:false,errno}));
  return {prepare:effects.get('OmpLaunch.prepare'), remove:effects.get('OmpLaunch.remove')};
}
export function errnoApi() {
  const source=fs.readFileSync('src/server/effects/loop.js','utf8');
  const start=source.indexOf('const BP_LINUX_ERRNO ='), end=source.indexOf('// Handles');
  return new Function('require','BP_WIN','io_strerror',source.slice(start,end)+';return {errno:bp_errno,text:io_strerror};')(require,true,()=>{});
}
export function errnoTests() {
  const api=errnoApi();
  for(const [code,num] of [['ENAMETOOLONG',36],['E2BIG',7],['ENOENT',2],['EACCES',13]]) {
    assert.equal(api.errno({code,errno:-4094}),num);
    assert(!api.text(num).includes('Input/output error'));
  }
}
export function fileTests() {
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'bp-omp-files-'));
  const api=launchFiles();
  try {
    const prompt=('skills \"quotes\" 日本 😀\n').repeat(6000);
    const a=api.prepare(root,prompt,'export default function() {}');
    const b=api.prepare(root,'second turn','export default function() {}');
    assert(a.ok&&b.ok);assert.notEqual(a.value,b.value);
    assert.equal(fs.readFileSync(path.join(a.value,'system-prompt.txt'),'utf8'),prompt);
    assert.equal(fs.readFileSync(path.join(b.value,'system-prompt.txt'),'utf8'),'second turn');
    for(const dir of [a.value,b.value]) {
      assert(!fs.lstatSync(dir).isSymbolicLink());
      if(process.platform!=='win32') {
        assert.equal(fs.statSync(dir).mode&0o777,0o700);
        assert.equal(fs.statSync(path.join(dir,'system-prompt.txt')).mode&0o777,0o600);
      }
    }
    api.remove(a.value);assert(!fs.existsSync(a.value));assert(fs.existsSync(b.value));api.remove(b.value);
    const missing=launchFiles({which:()=>null}).prepare(root,prompt,'extension');
    assert.deepEqual(missing,{ok:false,errno:2});
    let lookup;
    const checked=launchFiles({which:(name,options)=>{lookup={name,options};return '/fixture/omp';}}).prepare(root,prompt,'extension');
    assert.equal(lookup.name,'omp');
    const key=Object.keys(process.env).find(k=>k.toUpperCase()==='PATH');
    assert.equal(lookup.options.PATH,key?process.env[key]:'');
    assert(checked.ok);api.remove(checked.value);
    let writes=0;
    const fail=launchFiles({files:{...fs,writeFileSync(...args){
      if(++writes===2) throw Object.assign(Error('fixture denied'),{code:'EACCES'});
      return fs.writeFileSync(...args);
    }}}).prepare(root,prompt,'extension');
    assert.deepEqual(fail,{ok:false,errno:13});
    assert.deepEqual(fs.readdirSync(path.join(root,'omp')),[],'partial writes are removed');
  } finally {fs.rmSync(root,{recursive:true,force:true});}
}
