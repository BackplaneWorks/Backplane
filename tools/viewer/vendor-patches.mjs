import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';

const sha256=value=>createHash('sha256').update(value).digest('hex');
export const stratifiedAxesPatch=Object.freeze({
  id:'stratified-axes-0.0.24',
  package:'three-gpu-pathtracer',
  version:'0.0.24',
  module:'src/shader/rand/stratified.glsl.js',
  upstreamSha256:'44219b54f1f67f49e6e684852492c521a7417f8f0d73b3653b9c2e6b54c92bf3',
  from:'ivec2 uv = ivec2( v, sobolBounceIndex );',
  to:'ivec2 uv = ivec2( sobolBounceIndex, v );',
  expectedMatches:1,
});

// Texture columns hold bounce depth; rows hold random dimension. The pinned
// shader reverses them, reading dimensions 14/15 outside a 14-column texture
// with our 5+4 bounce configuration. Transform only these verified bytes.
export function patchStratifiedAxes(source,version){
  const patch=stratifiedAxesPatch;
  assert.equal(version,patch.version,'Viewer sampling patch package version mismatch');
  const matches=source.split(patch.from).length-1;
  assert.equal(matches,patch.expectedMatches,'Viewer sampling patch literal count mismatch');
  assert.equal(sha256(source),patch.upstreamSha256,'Viewer sampling patch upstream SHA-256 mismatch');
  const contents=source.replace(patch.from,patch.to);
  return {contents,manifest:{...patch,appliedMatches:matches,patchedSha256:sha256(contents)}};
}

export async function createViewerVendorPatch(packageRoot){
  const root=resolve(packageRoot),modulePath=resolve(root,stratifiedAxesPatch.module),packagePath=resolve(root,'package.json');
  const [source,packageBytes]=await Promise.all([readFile(modulePath,'utf8'),readFile(packagePath)]);
  const pkg=JSON.parse(packageBytes);
  assert.equal(pkg.name,stratifiedAxesPatch.package,'Viewer sampling patch package name mismatch');
  const patched=patchStratifiedAxes(source,pkg.version);
  let interceptions=0;
  return {
    plugin:{name:stratifiedAxesPatch.id,setup(build){
      build.onLoad({filter:/three-gpu-pathtracer\/src\/shader\/rand\/stratified\.glsl\.js$/},async args=>{
        assert.equal(resolve(args.path),modulePath,'Viewer sampling patch module path mismatch');
        assert.equal(++interceptions,1,'Viewer sampling patch intercepted more than once');
        assert.equal(await readFile(modulePath,'utf8'),source,'Viewer sampling patch input changed during build');
        return {loader:'js',contents:patched.contents};
      });
    }},
    async manifest(){
      assert.equal(interceptions,1,'Viewer sampling patch must be intercepted exactly once');
      const [afterSource,afterPackage]=await Promise.all([readFile(modulePath,'utf8'),readFile(packagePath)]);
      assert.equal(afterSource,source,'Viewer dependency shader changed during build');
      assert.deepEqual(afterPackage,packageBytes,'Viewer dependency package changed during build');
      return {...patched.manifest,interceptions,inputUnchanged:true};
    },
  };
}
