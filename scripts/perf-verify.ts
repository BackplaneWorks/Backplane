#!/usr/bin/env bun
// Verify supported local evidence against files actually being merged/shipped.
// This command never manufactures a build receipt for an existing binary.
import {createHash} from 'node:crypto';
import {closeSync,openSync,readSync,readFileSync,realpathSync,lstatSync} from 'node:fs';
import {resolve,relative} from 'node:path';
import {arch,cpus,hostname,platform,release} from 'node:os';
import {verifyPerformanceBundle,type CandidateBinding,type PerformanceBundle} from '../test/tools/perf_evidence';
import {authenticateBuild} from './perf-signature';

export function fileSha(path:string) {
  const hash=createHash('sha256'), chunk=Buffer.alloc(1024*1024), fd=openSync(path,'r');
  try { let n:number; while((n=readSync(fd,chunk,0,chunk.length,null))>0) hash.update(chunk.subarray(0,n)); }
  finally {closeSync(fd);}
  return hash.digest('hex');
}
function git(root:string,...args:string[]) {
  const env={...process.env};delete env.BACKPLANE_PERF_SIGNING_KEY;
  const result=Bun.spawnSync(['git','-C',root,...args],{env});
  if(result.exitCode!==0) throw new Error(`git ${args[0]} failed`);
  return result.stdout.toString();
}
export function sourceSha(root:string,includeUntracked=true) {
  const names=git(root,'ls-files','-z','--cached',...(includeUntracked?['--others','--exclude-standard']:[])).split('\0').filter(Boolean)
    .filter(n=>!/^\.bend-(check|test)\./.test(n));
  const hash=createHash('sha256');
  for(const name of [...new Set(names)].sort()) {
    const path=resolve(root,name);
    // Symlink targets are source inputs too; reading a dangling one fails.
    // Git preserves executability, not the builder's umask/group-write bits.
    // Normalize the latter so an identical checkout on the publish host binds.
    hash.update(name+'\0'+fileSha(path)+'\0'+String(lstatSync(path).mode&0o100?0o755:0o644)+'\0');
  }
  return hash.digest('hex');
}
export function harnessSha(root:string) {
  const names=git(root,'ls-files','-z','--cached','--others','--exclude-standard','--','test/tools/perf*','test/tools/present_observer*','scripts/perf-*').split('\0').filter(Boolean);
  const hash=createHash('sha256');
  for(const name of [...new Set(names)].sort()) hash.update(name+'\0'+fileSha(resolve(root,name))+'\0');
  return hash.digest('hex');
}
export function hostSha() {
  return createHash('sha256').update(JSON.stringify({hostname:hostname(),platform:platform(),arch:arch(),os:release(),
    cpu:cpus().map(c=>c.model)})).digest('hex');
}
export function buildInputs(root:string,versionStamp=process.env.BACKPLANE_VERSION_STAMP??'') {
  const located=Bun.which(process.env.CC??'clang');
  if(!located) throw new Error('configured native compiler is unavailable');
  const compiler=realpathSync(located);
  const env={...process.env,CC:compiler};
  delete env.BACKPLANE_PERF_SIGNING_KEY;
  const key=Bun.spawnSync(['scripts/inputs-key.sh','performance-build-v2'],{cwd:root,env});
  const inputKey=key.stdout.toString().trim();
  if(key.exitCode!==0 || !/^[a-f0-9]{40}$/.test(inputKey)) throw new Error('current Bend/compiler dependency key missing');
  const environment={CC:compiler,CFLAGS:process.env.CFLAGS??'',LDFLAGS:process.env.LDFLAGS??'',
    CPATH:process.env.CPATH??'',LIBRARY_PATH:process.env.LIBRARY_PATH??'',
    C_INCLUDE_PATH:process.env.C_INCLUDE_PATH??'',CPLUS_INCLUDE_PATH:process.env.CPLUS_INCLUDE_PATH??'',
    OBJC_INCLUDE_PATH:process.env.OBJC_INCLUDE_PATH??'',COMPILER_PATH:process.env.COMPILER_PATH??'',
    BACKPLANE_CFLAGS:process.env.BACKPLANE_CFLAGS??'-O3',BACKPLANE_X11:process.env.BACKPLANE_X11??'',
    BACKPLANE_GOOGLE_CLIENT_ID:process.env.BACKPLANE_GOOGLE_CLIENT_ID??'',
    BACKPLANE_GOOGLE_CLIENT_SECRET:process.env.BACKPLANE_GOOGLE_CLIENT_SECRET??'',
    BACKPLANE_GOOGLE_BAKE:process.env.BACKPLANE_GOOGLE_BAKE??'',
    BEND_LIB:process.env.BEND_LIB??'',BENDTT:process.env.BENDTT??'',BACKPLANE_VERSION_STAMP:versionStamp};
  const common=git(root,'rev-parse','--path-format=absolute','--git-common-dir').trim();
  const bake=[resolve(root,'.env'),resolve(common,'../.env')].map(p=>{
    try {return fileSha(p);} catch(error:any) {if(error.code==='ENOENT') return 'absent';throw error;}
  }).join(':');
  return {compiler_path:compiler,compiler_sha256:fileSha(compiler),bend_inputs_key:inputKey,
    environment_sha256:createHash('sha256').update(JSON.stringify(environment)).digest('hex'),
    private_build_inputs_sha256:createHash('sha256').update(bake).digest('hex'),version_stamp:versionStamp};
}

export function candidateBinding(root:string,binary:string,fixture:string,receiptFile:string,
    target:CandidateBinding['platform']):CandidateBinding {
  const receipt=authenticateBuild(root,JSON.parse(readFileSync(receiptFile,'utf8')));
  const commit=git(root,'rev-parse','HEAD').trim(), source=sourceSha(root);
  if(receipt.schema!==2 || receipt.commit!==commit || receipt.source_before!==source || receipt.source_after!==source)
    throw new Error('build receipt does not cover exact current candidate and inputs');
  if(receipt.tree!==git(root,'rev-parse','HEAD^{tree}').trim() || git(root,'status','--porcelain').trim())
    throw new Error('native evidence requires the exact clean committed tree');
  if(typeof receipt.emitted_c_path!=='string' || !/^[a-f0-9]{64}$/.test(receipt.emitted_c_sha256)
      || fileSha(resolve(root,receipt.emitted_c_path))!==receipt.emitted_c_sha256)
    throw new Error('native emitted C differs from build receipt');
  if(process.env.BACKPLANE_VERSION_STAMP && receipt.version_stamp!==process.env.BACKPLANE_VERSION_STAMP)
    throw new Error('requested version stamp differs from build receipt');
  for(const [key,value] of Object.entries(buildInputs(root,receipt.version_stamp)))
    if(receipt[key]!==value) throw new Error(`current native build input differs: ${key}`);
  const measuredBinary=fileSha(binary);
  if(receipt.binary_sha256!==measuredBinary) throw new Error('native binary differs from build receipt');
  return {commit,source_sha256:source,binary_sha256:measuredBinary,
    compiler_sha256:receipt.compiler_sha256,fixture_sha256:fileSha(fixture),harness_sha256:harnessSha(root),
    host_sha256:hostSha(),platform:target};
}
export function verifyFiles(root:string,evidence:string,binary:string,fixture:string,receiptFile:string,
    target:CandidateBinding['platform'],required:string[]) {
  const expected=candidateBinding(root,binary,fixture,receiptFile,target);
  const envelope=JSON.parse(readFileSync(evidence,'utf8'));
  // The immutable raw observations are hashed separately. The summary's
  // accepted:true/checks fields are ignored; budgets are recomputed from raw.
  const rawPath=resolve(evidence,'..',envelope.raw.path);
  if(fileSha(rawPath)!==envelope.raw.sha256) throw new Error('raw evidence hash mismatch');
  const bundle=JSON.parse(readFileSync(rawPath,'utf8')) as PerformanceBundle;
  const tracePath=resolve(evidence,'..',envelope.trace.path);
  if(fileSha(tracePath)!==envelope.trace.sha256) throw new Error('trace evidence hash mismatch');
  const trace:any[]=[];
  for(const line of readFileSync(tracePath,'utf8').split('\n')) {
    const trimmed=line.replace(/^,/, '').trim();
    if(!trimmed || trimmed==='[' || trimmed===']') continue;
    const event=JSON.parse(trimmed.replace(/,$/,''));
    if(event.ph==='X') trace.push(event);
  }
  trace.sort((a,b)=>a.ts-b.ts);
  for(const phase of bundle.phases) {
    const spans=trace.filter(e=>e.ts>=phase.start_ms*1000 && e.ts<phase.end_ms*1000)
      .map(e=>({name:e.name,at_ms:e.ts/1000,duration_ms:e.dur/1000}));
    if(JSON.stringify(spans)!==JSON.stringify(phase.spans)) throw new Error(`raw spans differ from trace: ${phase.name}`);
  }
  return {expected,raw:rawPath,started_at:bundle.started_at,completed_at:bundle.completed_at,
    ...verifyPerformanceBundle(bundle,expected,required)};
}

if(import.meta.main) {
  try {
    const [evidence,binary,fixture,receipt,target,...features]=process.argv.slice(2);
    if(!receipt || !['linux-native','ios-device','ios-simulator'].includes(target) || !features.length)
      throw new Error('usage: bun scripts/perf-verify.ts EVIDENCE BINARY FIXTURE BUILD-RECEIPT PLATFORM FEATURE...');
    const root=git(process.cwd(),'rev-parse','--show-toplevel').trim();
    // Record the path actually checked without trusting its filename/SHA label.
    const result=verifyFiles(root,resolve(evidence),realpathSync(binary),realpathSync(fixture),resolve(receipt),target as CandidateBinding['platform'],features);
    console.log(JSON.stringify({...result,binary:relative(root,realpathSync(binary))},null,2));
    process.exitCode=result.exitCode;
  } catch(error) {console.error(`performance evidence REJECTED: ${String(error)}`);process.exitCode=1;}
}
