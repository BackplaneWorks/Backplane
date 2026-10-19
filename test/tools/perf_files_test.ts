// Exercise file bindings and relocatable evidence, using synthetic observations.
// No executable here is represented as a native acceptance build.
import assert from 'node:assert/strict';
import {createHash,generateKeyPairSync} from 'node:crypto';
import {cpSync,mkdirSync,mkdtempSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {candidateBinding,fileSha,sourceSha,verifyFiles,buildInputs} from '../../scripts/perf-verify';
import {attestBuild} from '../../scripts/perf-signature';
import type {PerformanceBundle,RawPhase} from './perf_evidence';

const root=mkdtempSync(join(tmpdir(),'bp-perf-files-'));
const keys=mkdtempSync(join(tmpdir(),'bp-perf-test-keys-'));
const savedEnv={...process.env};
function git(...args:string[]) {
  const p=Bun.spawnSync(['git','-C',root,...args]);assert.equal(p.exitCode,0,p.stderr.toString());return p.stdout.toString().trim();
}
const hash=(value:string)=>createHash('sha256').update(value).digest('hex');
const json=(file:string,value:unknown)=>writeFileSync(file,JSON.stringify(value));
try {
  const pair=generateKeyPairSync('ed25519');
  writeFileSync(join(keys,'public.pem'),pair.publicKey.export({type:'spki',format:'pem'}));
  writeFileSync(join(keys,'private.pem'),pair.privateKey.export({type:'pkcs8',format:'pem'}),{mode:0o600});
  writeFileSync(join(keys,'compiler'),'synthetic compiler fixture',{mode:0o755});
  process.env.CC=join(keys,'compiler');process.env.BACKPLANE_PERF_TRUST_KEY=join(keys,'public.pem');
  process.env.BACKPLANE_PERF_SIGNING_KEY=join(keys,'private.pem');
  git('init','-q');writeFileSync(join(root,'.gitignore'),'build/\n.env\n');writeFileSync(join(root,'source.txt'),'candidate\n');
  mkdirSync(join(root,'scripts'));writeFileSync(join(root,'scripts/inputs-key.sh'),'#!/bin/sh\necho '+hash('test toolchain').slice(0,40)+'\n',{mode:0o755});
  git('add','.');git('-c','user.name=i2cjak','-c','user.email=i2cjak@users.noreply.github.com','commit','-qm','isolated file-verifier fixture');
  const build=join(root,'build');mkdirSync(build);
  const binary=join(build,'binary'),fixture=join(build,'fixture'),receipt=join(build,'receipt.json'),emitted=join(build,'all.c');
  writeFileSync(binary,'synthetic test artifact');writeFileSync(fixture,'synthetic test input');
  writeFileSync(emitted,'synthetic C fixture');
  const record={schema:2,commit:git('rev-parse','HEAD'),tree:git('rev-parse','HEAD^{tree}'),source_before:sourceSha(root),source_after:sourceSha(root),...buildInputs(root),
    binary_sha256:fileSha(binary),emitted_c_path:'build/all.c',emitted_c_sha256:fileSha(emitted)};
  const signed=attestBuild(root,record);json(receipt,signed);
  const expected=candidateBinding(root,binary,fixture,receipt,'linux-native');
  const phases:RawPhase[]=['continuous-scroll','input','thread-switch','background'].map((name,index)=>{
    const start=index*21000,end=start+20000;
    const count=name==='continuous-scroll'?1200:name==='input'?100:10;
    const step=20000/count;
    const demands=Array.from({length:count},(_,i)=>{
      const region=name==='thread-switch'?['timeline','subagents','rooms','bots','status'][i%5]:'timeline';
      return {id:`${name}-${i}`,revision:`${name}-${i}`,region,features:region==='timeline'?['timeline','long-history']:[region],
        contentHash:hash(`content-${i}`),pixelHash:hash(`pixel-${i}`),scheduled:start+i*step,sent:start+i*step+.1,observed:start+i*step+.2};
    });
    const drawn=demands.map((d,i)=>({frame:i+1,at:d.sent+2,revision:d.revision,region:d.region,
      contentHash:d.contentHash,pixelHash:d.pixelHash,features:d.features}));
    const content={boundary:'xserver-readback' as const,start,end,refreshHz:60,
      refresh:{source:'xvfb-reference' as const,hz:60,intervals_ms:[]},demands,drawn,
      readbacks:drawn.map(f=>({at:f.at+.5,region:f.region,pixelHash:f.pixelHash})),
      presented:drawn.map(f=>({frame:f.frame,at:f.at+1}))};
    const spans=[...Array.from({length:400},(_,i)=>({name:'ui.frame',at_ms:start+i*49,duration_ms:5})),
      ...Array.from({length:100},(_,i)=>({name:'hub.agent_line',at_ms:start+i*199,duration_ms:1})),
      ...(name==='input'?Array.from({length:100},(_,i)=>({name:'ui.input',at_ms:start+i*199,duration_ms:5})):[])].sort((a,b)=>a.at_ms-b.at_ms);
    return {name,start_ms:start,end_ms:end,spans,
      streams:Array.from({length:10},(_,i)=>({id:`stream${i}`,received_ms:Array.from({length:40},(_,j)=>start+j*500)})),
      opens:name==='thread-switch'?Array.from({length:10},(_,i)=>({expected:`t${i}`,drawn:`t${i}`,duration_ms:5})):[],
      ...(name==='continuous-scroll'?{cadence:content}:name==='background'?{}:{content})};
  });
  const now=Date.now();
  const bundle:PerformanceBundle={schema:1,built:expected,measured:expected,started_at:now-100000,completed_at:now-1000,
    history_bytes:40_000_000,phases};
  const evidence=join(build,'evidence');mkdirSync(evidence);
  const raw=join(evidence,'raw.json'),trace=join(evidence,'trace.json'),envelope=join(evidence,'evidence.json');
  const events=phases.flatMap(p=>p.spans.map(s=>({ph:'X',name:s.name,ts:s.at_ms*1000,dur:s.duration_ms*1000})));
  writeFileSync(trace,'[\n'+events.map(e=>JSON.stringify(e)).join(',\n')+'\n]\n');json(raw,bundle);
  json(envelope,{schema:1,raw:{path:'raw.json',sha256:fileSha(raw)},trace:{path:'trace.json',sha256:fileSha(trace)}});
  const verify=(file=envelope)=>verifyFiles(root,file,binary,fixture,receipt,'linux-native',['timeline']);
  assert.equal(verify().accepted,true,'synthetic valid file bindings');
  json(receipt,record);assert.throws(()=>verify(),/configured-builder signature/);
  json(receipt,{...signed,compiler_sha256:hash('forged compiler')});assert.throws(()=>verify(),/configured-builder signature/);
  json(receipt,signed);
  writeFileSync(emitted,'modified C');assert.throws(()=>verify(),/emitted C differs/);writeFileSync(emitted,'synthetic C fixture');
  writeFileSync(join(keys,'compiler'),'modified compiler');assert.throws(()=>verify(),/current native build input differs: compiler_sha256/);
  writeFileSync(join(keys,'compiler'),'synthetic compiler fixture');
  process.env.BACKPLANE_CFLAGS='-O0';assert.throws(()=>verify(),/current native build input differs: environment_sha256/);
  if(savedEnv.BACKPLANE_CFLAGS===undefined) delete process.env.BACKPLANE_CFLAGS;else process.env.BACKPLANE_CFLAGS=savedEnv.BACKPLANE_CFLAGS;
  for(const key of ['CPATH','LIBRARY_PATH','C_INCLUDE_PATH','CPLUS_INCLUDE_PATH','OBJC_INCLUDE_PATH','COMPILER_PATH']) {
    process.env[key]='/changed/compiler/search/path';
    assert.throws(()=>verify(),/current native build input differs: environment_sha256/,key);
    if(savedEnv[key]===undefined) delete process.env[key];else process.env[key]=savedEnv[key];
  }
  writeFileSync(join(root,'.env'),'TEST_PRIVATE_INPUT=changed');assert.throws(()=>verify(),/private_build_inputs_sha256/);rmSync(join(root,'.env'));
  const moved=join(build,'copied');cpSync(evidence,moved,{recursive:true});
  assert.equal(verify(join(moved,'evidence.json')).accepted,true,'relative raw files survive package copying');
  writeFileSync(binary,'changed after staging');assert.throws(()=>verify(),/differs from build receipt/);
  writeFileSync(binary,'synthetic test artifact');
  writeFileSync(join(root,'source.txt'),'changed input');assert.throws(()=>verify(),/exact current candidate/);
  writeFileSync(join(root,'source.txt'),'candidate\n');
  writeFileSync(join(moved,'trace.json'),'[]');assert.throws(()=>verify(join(moved,'evidence.json')),/trace evidence hash mismatch/);
  const altered=JSON.parse(readFileSync(raw,'utf8'));altered.phases[0].spans[0].duration_ms=0;json(raw,altered);
  assert.throws(()=>verify(),/raw evidence hash mismatch/);
  // Even updating the summary hash cannot hide disagreement with the trace.
  const meta=JSON.parse(readFileSync(envelope,'utf8'));meta.raw.sha256=fileSha(raw);json(envelope,meta);
  assert.throws(()=>verify(),/raw spans differ from trace/);
  console.log('PASS file verifier: builder signature required; changed C/compiler/environment/private inputs/binary/source/raw/trace rejected; copied bundle works');
} finally {
  for(const key of Object.keys(process.env)) if(!(key in savedEnv)) delete process.env[key];
  Object.assign(process.env,savedEnv);rmSync(root,{recursive:true,force:true});rmSync(keys,{recursive:true,force:true});
}
