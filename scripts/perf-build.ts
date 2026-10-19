#!/usr/bin/env bun
// One acceptance build with an immutable source -> C -> compiler -> binary
// receipt. Ordinary developer builds/caches remain available separately.
import {writeFileSync,rmSync} from 'node:fs';
import {resolve,basename,relative} from 'node:path';
import {fileSha,sourceSha,buildInputs} from './perf-verify';
import {attestBuild,signingConfiguration} from './perf-signature';
const root=resolve(import.meta.dir,'..');
function run(argv:string[]) {
  const env={...process.env};delete env.BACKPLANE_PERF_SIGNING_KEY;
  const r=Bun.spawnSync(argv,{cwd:root,env});
  if(r.exitCode!==0) throw new Error(`${argv[0]} failed`);
  return r.stdout.toString();
}
try {
  const [source,out]=process.argv.slice(2);
  if(!source||!out) throw new Error('usage: bun scripts/perf-build.ts SOURCE OUT');
  // Refuse an unconfigured builder before deleting artifacts or compiling.
  signingConfiguration(root);
  if(run(['git','status','--porcelain']).trim()) throw new Error('acceptance build requires a clean committed candidate');
  const receipt=resolve(root,out+'.perf-build.json');
  rmSync(receipt,{force:true});
  rmSync(resolve(root,out),{force:true});
  const before=sourceSha(root),commit=run(['git','rev-parse','HEAD']).trim();
  const context=buildInputs(root);
  const environment={...process.env,CC:context.compiler_path,BACKPLANE_EMIT_CACHE:'0',BACKPLANE_OBJ_CACHE:'0',BACKPLANE_PREBUILT:'0',BACKPLANE_EMIT_ONLY:'0'};
  // Benchmark/build children never receive the signing-key configuration.
  delete environment.BACKPLANE_PERF_SIGNING_KEY;
  const job=Bun.spawn(['scripts/build-app.sh',source,out],{cwd:root,stdin:'inherit',stdout:'inherit',stderr:'inherit',
    env:environment});
  if(await job.exited!==0) throw new Error('native acceptance build failed');
  const after=sourceSha(root);
  if(before!==after || run(['git','rev-parse','HEAD']).trim()!==commit || JSON.stringify(buildInputs(root))!==JSON.stringify(context))
    throw new Error('source/compiler inputs changed during native build');
  const work=resolve(root,'build/cc',basename(out));
  const record={schema:2,commit,tree:run(['git','rev-parse','HEAD^{tree}']).trim(),source_before:before,source_after:after,...context,
    compiler_version:run([context.compiler_path,'--version']).split('\n')[0],
    emitted_c_path:relative(root,resolve(work,'all.c')),emitted_c_sha256:fileSha(resolve(work,'all.c')),binary_sha256:fileSha(resolve(root,out)),
    completed_at:Date.now()};
  writeFileSync(receipt,JSON.stringify(attestBuild(root,record),null,2)+'\n');
  console.log(`native performance build receipt: ${receipt}`);
} catch(e) {console.error(`performance build REJECTED: ${String(e)}`);process.exitCode=1;}
