import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve,dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {createViewerVendorPatch} from './vendor-patches.mjs';
const root=dirname(fileURLToPath(import.meta.url)),out=resolve(process.argv[2]||join(root,'../../build/viewer/vendor'));
await mkdir(out,{recursive:true});
const samplingPatch=await createViewerVendorPatch(join(root,'node_modules/three-gpu-pathtracer'));
for(const [entry,name]of [['entry.js','viewer.js'],['node_modules/three-mesh-bvh/src/workers/generateMeshBVH.worker.js','bvh-worker.js'],['render-worker.js','render-worker.js']]){
 const built=await Bun.build({entrypoints:[join(root,entry)],target:'browser',format:'esm',minify:true,outdir:out,naming:name,plugins:entry==='entry.js'?[samplingPatch.plugin]:[]});
 if(!built.success)throw new AggregateError(built.logs,'Viewer vendor bundle failed');
}
const patches=[await samplingPatch.manifest()];
const packages=['three','three-gpu-pathtracer','three-mesh-bvh','xatlas-web'];
const licenses=[];for(const pkg of packages){const p=JSON.parse(await readFile(join(root,'node_modules',pkg,'package.json'),'utf8'));let license='';for(const name of ['LICENSE','LICENSE.md','LICENSE.txt']){try{license=await readFile(join(root,'node_modules',pkg,name),'utf8');break}catch{}}
 if(!license)throw Error(`Missing license ${pkg}`);licenses.push(`${pkg} ${p.version}\n${license}`);}
await writeFile(join(out,'LICENSES.txt'),licenses.join('\n\n'));
const files={};for(const name of ['viewer.js','bvh-worker.js','render-worker.js','LICENSES.txt']){const b=await readFile(join(out,name));files[name]={bytes:b.length,sha256:createHash('sha256').update(b).digest('hex')};}
await writeFile(join(out,'manifest.json'),JSON.stringify({versions:{three:'0.181.1',pathtracer:'0.0.24',bvh:'0.9.5'},patches,files},null,2)+'\n');
console.log(JSON.stringify({out,files}));
