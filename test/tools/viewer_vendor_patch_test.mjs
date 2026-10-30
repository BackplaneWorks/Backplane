import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdtemp,readFile,writeFile,mkdir,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {stratifiedAxesPatch as pin,patchStratifiedAxes,createViewerVendorPatch} from '../../tools/viewer/vendor-patches.mjs';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..'),source=await readFile(resolve(root,'tools/viewer/node_modules',pin.package,pin.module),'utf8');
const sha=value=>createHash('sha256').update(value).digest('hex');
const patched=patchStratifiedAxes(source,pin.version);
assert.equal(patched.manifest.upstreamSha256,sha(source));
assert.equal(patched.manifest.patchedSha256,sha(patched.contents));
assert.equal(patched.contents.replace(pin.to,pin.from),source,'exactly the reviewed lookup changes');
assert.throws(()=>patchStratifiedAxes(source,'0.0.25'),/version mismatch/);
assert.throws(()=>patchStratifiedAxes(source+'\n',pin.version),/SHA-256 mismatch/);
assert.throws(()=>patchStratifiedAxes(source+pin.from,pin.version),/literal count mismatch/);
assert.throws(()=>patchStratifiedAxes(patched.contents,pin.version),/literal count mismatch/);
const temporary=await mkdtemp(resolve(tmpdir(),'bp-viewer-vendor-patch-'));
try{
  const packageRoot=resolve(temporary,pin.package),modulePath=resolve(packageRoot,pin.module),packagePath=resolve(packageRoot,'package.json');
  await mkdir(dirname(modulePath),{recursive:true});await writeFile(modulePath,source);
  const packageBytes=JSON.stringify({name:pin.package,version:pin.version});await writeFile(packagePath,packageBytes);
  const capture=patch=>{let load;patch.plugin.setup({onLoad(options,callback){assert(options.filter.test(modulePath));load=callback;}});return load;};
  const patch=await createViewerVendorPatch(packageRoot),load=capture(patch);
  await assert.rejects(()=>patch.manifest(),/exactly once/);
  await assert.rejects(()=>load({path:modulePath+'.other'}),/path mismatch/);
  assert.deepEqual(await load({path:modulePath}),{loader:'js',contents:patched.contents});
  const manifest=await patch.manifest();assert.equal(manifest.interceptions,1);assert.equal(manifest.inputUnchanged,true);assert.equal(manifest.patchedSha256,sha(patched.contents));
  await assert.rejects(()=>load({path:modulePath}),/more than once/);
  assert.equal(await readFile(modulePath,'utf8'),source);assert.equal(await readFile(packagePath,'utf8'),packageBytes);
  const changed=await createViewerVendorPatch(packageRoot),changedLoad=capture(changed);await writeFile(modulePath,source+'\n');
  await assert.rejects(()=>changedLoad({path:modulePath}),/input changed/);await writeFile(modulePath,source);
  const changedAfter=await createViewerVendorPatch(packageRoot);await capture(changedAfter)({path:modulePath});await writeFile(packagePath,packageBytes+'\n');
  await assert.rejects(()=>changedAfter.manifest(),/package changed/);await writeFile(packagePath,JSON.stringify({name:'different',version:pin.version}));
  await assert.rejects(()=>createViewerVendorPatch(packageRoot),/name mismatch/);
}finally{await rm(temporary,{recursive:true,force:true});}
console.log('Viewer pinned sampling patch: positive, provenance and fail-closed negative checks PASS');
