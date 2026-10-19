// Bind the final publishable Linux package bytes to a passed native workload.
// Archive payloads are read, never executed (including the AppImage runtime).
import {openSync,closeSync,readSync,statSync,lstatSync,readdirSync,readFileSync,writeFileSync,existsSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {arch} from 'node:os';
import {authenticateBuild,attestBuild} from './perf-signature';
import {fileSha,verifyFiles,sourceSha,harnessSha} from './perf-verify';
import type {CandidateBinding} from '../test/tools/perf_evidence';

export function packageNames(tag:string,target:string) {
  if(!/^[a-zA-Z0-9._+-]+$/.test(tag) || !['linux-x64','linux-arm64'].includes(target)) throw new Error('invalid release tag/platform');
  const version=tag.replace(/^v/,'').replaceAll('-','~'),arch=target==='linux-x64'?'amd64':'arm64';
  return [`backplane-${tag}-${target}.tar.gz`,`backplane-${tag}-${target}.AppImage`,`backplane_${version}_${arch}.deb`];
}
export function squashOffset(path:string) {
  const fd=openSync(path,'r'),size=statSync(path).size,buffer=Buffer.alloc(65536+96),matches:number[]=[];
  try {
    for(let start=0;start<size;start+=65536) {
      const count=readSync(fd,buffer,0,buffer.length,start);
      for(let i=buffer.indexOf('hsqs',0,'ascii');i>=0 && i<Math.min(count-95,65536);i=buffer.indexOf('hsqs',i+4,'ascii')) {
        const block=buffer.readUInt32LE(i+12),bytes=buffer.readBigUInt64LE(i+40);
        if(buffer.readUInt16LE(i+28)===4 && block>=4096 && block<=1048576 && (block&(block-1))===0
            && bytes>=96n && bytes<=BigInt(size-start-i)) matches.push(start+i);
      }
    }
  } finally {closeSync(fd);}
  if(matches.length!==1) throw new Error('AppImage must have exactly one readable SquashFS payload');
  return matches[0];
}
type PayloadEntry={path:string;kind:'file'|'directory';access:number;size?:number;sha256?:string};
function treeEntries(path:string,name=''):PayloadEntry[] {
  const stat=lstatSync(path);
  if((!stat.isDirectory() && !stat.isFile()) || stat.mode&0o7000) throw new Error('built payload must contain regular files/directories');
  const entry:PayloadEntry={path:name,kind:stat.isDirectory()?'directory':'file',access:stat.mode&0o555};
  if(stat.isFile()) return [{...entry,size:stat.size,sha256:fileSha(path)}];
  return [entry,...readdirSync(path).sort().flatMap(n=>treeEntries(join(path,n),name?`${name}/${n}`:n))];
}
export function releasePayload(root:string,dist:string):PayloadEntry[] {
  const entries:PayloadEntry[]=[{path:'',kind:'directory',access:lstatSync(dist).mode&0o555}];
  for(const name of ['backplane','backplane-serve','web']) entries.push(...treeEntries(join(dist,name),name));
  for(const name of ['backplane-browser','backplane-step2glb','backplane-voice','backplane-kipc','licenses'])
    if(existsSync(join(dist,name))) entries.push(...treeEntries(join(dist,name),name));
  entries.push(...treeEntries(join(root,'LICENSE'),'LICENSE'),...treeEntries(join(root,'deploy/backplane.desktop'),'backplane.desktop'));
  entries.push({path:'icon',kind:'directory',access:lstatSync(join(root,'assets/icon')).mode&0o555});
  for(const name of ['backplane.svg','backplane-256.png']) entries.push(...treeEntries(join(root,'assets/icon',name),`icon/${name}`));
  for(const name of ['backplane','backplane-serve','backplane-browser','backplane-step2glb','backplane-voice','backplane-kipc']) {
    const e=entries.find(e=>e.path===name);
    if(e && (e.kind!=='file' || (e.access&0o111)!==0o111)) throw new Error(`built ${name} is not a regular executable`);
  }
  return entries.sort((a,b)=>a.path.localeCompare(b.path));
}
function canonicalEntries(entries:PayloadEntry[]) {
  return JSON.stringify([...entries].sort((a,b)=>a.path.localeCompare(b.path))
    .map(e=>[e.path,e.kind,e.access,e.size??null,e.sha256??null]));
}
function samePayload(actual:PayloadEntry[],expected:PayloadEntry[],label:string) {
  if(canonicalEntries(actual)!==canonicalEntries(expected)) throw new Error(`${label} differs from completed-build payload (content/type/mode/member set)`);
}
function subtree(entries:PayloadEntry[],prefix:string) {
  return entries.filter(e=>e.path===prefix || e.path.startsWith(prefix+'/'))
    .map(e=>({...e,path:e.path===prefix?'':e.path.slice(prefix.length+1)}));
}
export function verifyCompletedPayload(root:string,dist:string,candidate:CandidateBinding):PayloadEntry[] {
  const completed=authenticateBuild(root,JSON.parse(readFileSync(join(dist,'backplane.perf-dist.json'),'utf8')));
  if(completed.schema!==1 || completed.kind!=='completed-dist-payload'
      || JSON.stringify(completed.candidate)!==JSON.stringify(candidate)) throw new Error('completed dist provenance mismatch');
  samePayload(releasePayload(root,dist),completed.entries,'finished dist');
  return completed.entries;
}
async function archiveEntries(argv:string[],input?:ReadableStream<Uint8Array>):Promise<PayloadEntry[]> {
  const env={...process.env};delete env.BACKPLANE_PERF_SIGNING_KEY;
  const child=Bun.spawn(argv,{stdin:input??'ignore',stdout:'pipe',stderr:'ignore',env});
  try {
    const text=await new Response(child.stdout).text();
    if(await child.exited!==0) throw new Error(`cannot read regular package payload with ${argv[0]}`);
    return JSON.parse(text);
  } finally {if(child.exitCode===null) child.kill();await child.exited;}
}
export async function verifyPackageContents(dir:string,tag:string,target:string,binarySha:string,
    expected:PayloadEntry[],staging:string) {
  const names=packageNames(tag,target);
  for(const name of ['backplane','backplane-serve']) {
    const entry=expected.find(e=>e.path===name);
    if(!entry || entry.kind!=='file' || entry.sha256!==binarySha || (entry.access&0o111)!==0o111)
      throw new Error('completed payload does not contain the measured regular executable');
  }
  const reader=resolve(import.meta.dir,'perf-payload.py');
  const tar=await archiveEntries(['python3',reader,'tar',join(dir,names[0])]);
  samePayload(tar,expected.map(e=>({...e,path:e.path?`backplane-${target}/${e.path}`:`backplane-${target}`})),'tar');
  const appTree=treeEntries(join(staging,'AppDir'));
  const launcher=appTree.find(e=>e.path==='AppRun');
  if(!launcher || launcher.kind!=='file' || (launcher.access&0o111)!==0o111) throw new Error('AppImage launcher is not a regular executable');
  samePayload(subtree(appTree,'usr/lib/backplane'),expected,'AppImage staged installed payload');
  const app=await archiveEntries(['python3',reader,'squash',join(dir,names[1]),String(squashOffset(join(dir,names[1])))]);
  samePayload(app,appTree,'AppImage');
  const debTree=treeEntries(join(staging,'deb'));
  for(const name of ['backplane','backplane-serve']) {
    const launcher=debTree.find(e=>e.path===`usr/bin/${name}`);
    if(!launcher || launcher.kind!=='file' || (launcher.access&0o111)!==0o111) throw new Error('deb launcher is not a regular executable');
  }
  samePayload(subtree(debTree,'opt/backplane'),expected,'deb staged installed payload');
  const readerEnv={...process.env};delete readerEnv.BACKPLANE_PERF_SIGNING_KEY;
  const members=Bun.spawnSync(['ar','t',join(dir,names[2])],{env:readerEnv});
  if(members.exitCode!==0 || members.stdout.toString().trim().split('\n').join(',')!=='debian-binary,control.tar.gz,data.tar.gz')
    throw new Error('unexpected deb member set');
  const version=Bun.spawnSync(['ar','p',join(dir,names[2]),'debian-binary'],{env:readerEnv});
  if(version.exitCode!==0 || version.stdout.toString()!=='2.0\n') throw new Error('invalid deb version');
  const controlTree=treeEntries(join(staging,'control'));
  if(controlTree.length!==2 || !controlTree.some(e=>e.path==='control' && e.kind==='file'))
    throw new Error('unexpected deb control/scripts payload');
  for(const [section,tree] of [['data.tar.gz',debTree],['control.tar.gz',controlTree]] as const) {
    const env={...process.env};delete env.BACKPLANE_PERF_SIGNING_KEY;
    const ar=Bun.spawn(['ar','p',join(dir,names[2]),section],{stdin:'ignore',stdout:'pipe',stderr:'ignore',env});
    try {
      const entries=await archiveEntries(['python3',reader,'tar','-'],ar.stdout);
      if(await ar.exited!==0) throw new Error('cannot read deb payload');
      samePayload(entries,[...tree],`deb ${section}`);
    } finally {if(ar.exitCode===null) ar.kill();await ar.exited;}
  }
  return Object.fromEntries(names.map(name=>[name,fileSha(join(dir,name))]));
}
function git(root:string,...args:string[]) {
  const env={...process.env};delete env.BACKPLANE_PERF_SIGNING_KEY;
  const p=Bun.spawnSync(['git','-C',root,...args],{env});
  if(p.exitCode!==0) throw new Error('cannot bind package to candidate commit');return p.stdout.toString().trim();
}
export function verifyPackageRecord(root:string,dir:string,tag:string,target:string) {
  const signed=JSON.parse(readFileSync(join(dir,`backplane-${tag}-${target}.perf-package.json`),'utf8'));
  const record=authenticateBuild(root,signed),names=packageNames(tag,target);
  const candidate=record.candidate,hash=(value:unknown)=>typeof value==='string' && /^[a-f0-9]{64}$/.test(value);
  if(record.schema!==1 || record.kind!=='native-performance-package' || record.tag!==tag || record.target!==target
      || record.commit!==git(root,'rev-parse','HEAD') || record.tree!==git(root,'rev-parse','HEAD^{tree}')
      || !Number.isFinite(record.accepted_at) || record.accepted_at>Date.now() || Date.now()-record.accepted_at>24*3600*1000
      || !Number.isFinite(record.measurement_started_at) || !Number.isFinite(record.measurement_completed_at)
      || record.measurement_completed_at<record.measurement_started_at || record.measurement_completed_at>record.accepted_at
      || Date.now()-record.measurement_started_at>24*3600*1000
      || !candidate || candidate.commit!==record.commit || candidate.platform!=='linux-native'
      || candidate.source_sha256!==sourceSha(root,false) || candidate.harness_sha256!==harnessSha(root)
      || !['source_sha256','binary_sha256','compiler_sha256','fixture_sha256','harness_sha256','host_sha256'].every(k=>hash(candidate[k]))
      || !hash(record.evidence_sha256)
      || !hash(record.payload_manifest_sha256)
      || !record.packages || JSON.stringify(Object.keys(record.packages).sort())!==JSON.stringify(names.sort()))
    throw new Error('missing/stale/wrong-candidate package attestation');
  for(const name of names) if(record.packages[name]!==fileSha(join(dir,name)))
    throw new Error(`publishable package changed after acceptance: ${name}`);
  return record;
}
export function verifyPublication(root:string,dir:string,tag:string) {
  const allowed=[...packageNames(tag,'linux-x64'),...packageNames(tag,'linux-arm64'),
    `backplane-${tag}-linux-x64.perf-package.json`,`backplane-${tag}-linux-arm64.perf-package.json`,
    `backplane-${tag}-darwin-arm64.tar.gz`,`backplane-${tag}-android.apk`].sort();
  if(JSON.stringify(readdirSync(dir).sort())!==JSON.stringify(allowed)
      || allowed.some(name=>!lstatSync(join(dir,name)).isFile()))
    throw new Error('release artifact set is missing, unexpected or contains nonregular files');
  return ['linux-x64','linux-arm64'].map(target=>verifyPackageRecord(root,dir,tag,target));
}
if(import.meta.main) {
  try {
    const [mode,tag,target,output]=process.argv.slice(2),root=resolve(import.meta.dir,'..');
    const nativeResult=()=>verifyFiles(root,join(root,'build/release-performance/evidence.json'),join(root,'dist/backplane'),
      process.env.BACKPLANE_PERF_FIXTURE??'',join(root,'dist/backplane.perf-build.json'),'linux-native',[]);
    if(mode==='dist' && !tag) {
      const result=nativeResult();if(!result.accepted) throw new Error('strict native evidence did not pass');
      const record={schema:1,kind:'completed-dist-payload',candidate:result.expected,entries:releasePayload(root,join(root,'dist'))};
      writeFileSync(join(root,'dist/backplane.perf-dist.json'),JSON.stringify(attestBuild(root,record),null,2)+'\n');
    } else if(mode==='verify-all' && target && !output) {
      verifyPublication(root,resolve(target),tag);
    } else if(mode==='seal' && !output) {
      packageNames(tag,target);
      if((target==='linux-x64'?'x64':'arm64')!==arch()) throw new Error('release target differs from measured native host architecture');
      const result=nativeResult();
      if(!result.accepted) throw new Error('strict native evidence did not pass');
      const entries=verifyCompletedPayload(root,join(root,'dist'),result.expected);
      const packages=await verifyPackageContents(join(root,'dist'),tag,target,result.expected.binary_sha256,
        entries,join(root,'build/pkg-linux'));
      const record={schema:1,kind:'native-performance-package',tag,target,commit:git(root,'rev-parse','HEAD'),
        tree:git(root,'rev-parse','HEAD^{tree}'),accepted_at:Date.now(),candidate:result.expected,packages,
        measurement_started_at:result.started_at,measurement_completed_at:result.completed_at,
        evidence_sha256:fileSha(join(root,'build/release-performance/evidence.json')),
        payload_manifest_sha256:fileSha(join(root,'dist/backplane.perf-dist.json'))};
      writeFileSync(join(root,'dist',`backplane-${tag}-${target}.perf-package.json`),JSON.stringify(attestBuild(root,record),null,2)+'\n');
    } else if(mode==='verify' && output) {
      verifyPackageRecord(root,resolve(output),tag,target);
    } else throw new Error('usage: perf-package.ts dist | seal TAG TARGET | verify TAG TARGET DIRECTORY | verify-all TAG DIRECTORY');
    console.log(`PASS exact measured Linux release packages ${tag} ${target}`);
  } catch(error) {console.error(`performance package REJECTED: ${String(error)}`);process.exitCode=1;}
}
