// Focused harness: the actual hub filesystem/spawn effects, real OMP RPC,
// no model credentials, no stand-in OMP, PTY, elevation or Developer Mode.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {launchFiles,expectedArgs,errnoApi,fileTests,errnoTests} from './omp_launch_shared.mjs';
const require=createRequire(import.meta.url);
const out=path.resolve(process.argv[2]), report={status:'INCOMPLETE',scope:'production OMP launch files + hub pipe spawn + real OMP 18.16.0 RPC handshake; no inference',cleanup:[]};
const save=()=>fs.writeFileSync(path.join(out,'result.json'),JSON.stringify(report,null,2)+'\n');
const shell=path.join(process.env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe');
let api,globals,files;
try {
  assert.equal(process.platform,'win32');
  report.identity=JSON.parse(execFileSync(shell,['-NoProfile','-NonInteractive','-Command',
    "$i=[Security.Principal.WindowsIdentity]::GetCurrent();$p=[Security.Principal.WindowsPrincipal]::new($i);$d=Get-ItemPropertyValue 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AppModelUnlock' AllowDevelopmentWithoutDevLicense -ErrorAction SilentlyContinue;$profile=Get-ItemPropertyValue ('HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\ProfileList\\'+$i.User.Value) ProfileImagePath;@{name=$i.Name;sid=$i.User.Value;profile=$profile;admin=$p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator);developerMode=($d -eq 1)}|ConvertTo-Json -Compress"],{encoding:'utf8'}));
  assert.equal(report.identity.admin,false);assert.equal(report.identity.developerMode,false);
  report.privileges=execFileSync('whoami.exe',['/priv'],{encoding:'utf8'});
  assert(!report.privileges.includes('SeCreateSymbolicLinkPrivilege'));
  const profile=report.identity.profile;
  // Start-Process -Credential loads the profile but inherits runner env.
  for(const key of Object.keys(process.env)) if(/TOKEN|API_KEY|SECRET|PASSWORD|CREDENTIAL/i.test(key)) delete process.env[key];
  Object.assign(process.env,{HOME:profile,USERPROFILE:profile,APPDATA:path.join(profile,'AppData/Roaming'),LOCALAPPDATA:path.join(profile,'AppData/Local'),TEMP:path.join(out,'temp'),TMP:path.join(out,'temp'),BACKPLANE_HOME:path.join(out,'private home 日本'),BACKPLANE_TELEMETRY:'off'});
  fs.mkdirSync(process.env.TEMP,{recursive:true});
  // Only our copied Bun/OMP and the packaged-equivalent env.exe from Git.
  const userBin=path.join(profile,'.local/bin');fs.mkdirSync(userBin,{recursive:true});
  fs.copyFileSync(path.join(out,'bin/omp.exe'),path.join(userBin,'omp.exe'));
  process.env.PATH=userBin+';'+path.join(out,'bin')+';C:\\Program Files\\Git\\usr\\bin;'+path.join(process.env.SystemRoot,'System32')+';'+process.env.SystemRoot;
  const target=path.join(out,'symlink-source');fs.writeFileSync(target,'fixture');
  assert.throws(()=>fs.symlinkSync(target,target+'-link'),e=>['EPERM','EACCES'].includes(e.code));
  report.installedOMP=path.join(userBin,'omp.exe');
  report.version=execFileSync(report.installedOMP,['--version'],{encoding:'utf8',timeout:30000}).trim();
  assert(report.version.includes('18.16.0'));fileTests();errnoTests();
  const errno=errnoApi(), handles=new Map();let next=1;
  globals={BP_H:{Stream:class{feed(){}finish(){}wake(){}},add:s=>{handles.set(next,s);return next++;},ERRNO:{ENOTDIR:20,ENOENT:2},errno:errno.errno}};
  const source=fs.readFileSync('src/server/effects/host.js','utf8').replace(/CID\(([^)]+)\)/g,'"$1"');
  report.hostSHA256=createHash('sha256').update(source).digest('hex');
  api=new Function('require','process','globalThis','io_eff','io_park_promise','io_done','io_fail','io_tup',source+`;return {spawn:bp_proc_spawn,retire:proc_retire,barrier:()=>proc_await_retired('smoke','',null)};`)(
    require,process,globals,()=>{},(p,k,done)=>p.then(x=>done(x),e=>done(undefined,e)),v=>({ok:true,value:v}),(errno,message)=>({ok:false,errno,message}),(a,b)=>[a,b]);
  const prompt='BACKPLANE_949_START\n'+('skill \"quoted\" 日本 😀\n').repeat(6000)+'BACKPLANE_949_END';
  report.promptUTF16Length=prompt.length;
  const home=process.env.BACKPLANE_HOME;fs.mkdirSync(home,{recursive:true});
  // Negative control: same actual hub spawn, old inline prompt argv.
  const before=api.spawn('env',['omp','--mode','rpc-ui','--append-system-prompt',prompt],home,1);
  assert.equal(before.ok,false,'Old inline argv must exceed native CreateProcessW limit');
  assert([36,7].includes(before.errno),JSON.stringify(before));report.negativeControl={...before,text:errno.text(before.errno)};save();
  files=launchFiles({which:Bun.which});
  const installedPath=process.env.PATH;
  try {
    process.env.PATH=path.join(process.env.SystemRoot,'System32');
    assert.equal(files.prepare(home,prompt,'extension').errno,2,'A missing real CLI fails before env is spawned');
    report.missingCLIError='ENOENT';
  } finally {process.env.PATH=installedPath;}
  // A fixture extension proves OMP loaded the entire file into its actual
  // effective system prompt, after ready, without a model request.
  const proofPath=path.join(out,'prompt-received.json');
  const extension=`import {writeFileSync} from 'node:fs';export default function(pi){pi.on('session_start',(_e,ctx)=>{const s=ctx.getSystemPrompt().join('\\n');writeFileSync(${JSON.stringify(proofPath)},JSON.stringify({start:s.includes('BACKPLANE_949_START'),end:s.includes('BACKPLANE_949_END'),length:s.length}));});}`;
  const prepared=files.prepare(home,prompt,extension);assert(prepared.ok,JSON.stringify(prepared));
  const args=expectedArgs('','always-ask',path.join(home,'sessions'),prepared.value);
  assert(args.join(' ').length<4000);
  const started=api.spawn('env',args,home,1);assert(started.ok,JSON.stringify(started));
  report.startedPid=started.value[0];const child=globals.BP_PROCS.get(report.startedPid);
  let buffer='',stderr='',ready=false,state;
  const handshake=new Promise((resolve,reject)=>{
    child.stdout.on('data',chunk=>{buffer+=chunk.toString();let i;while((i=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,i);buffer=buffer.slice(i+1);let row;try{row=JSON.parse(line);}catch{continue;}
      if(row.type==='ready'){ready=true;report.ready=row;}
      if(row.id==='1'&&row.type==='response'){state=row;if(row.success)resolve();else reject(Error(JSON.stringify(row)));}
    }});
    child.stderr.on('data',chunk=>{stderr+=chunk.toString();});
    child.once('error',reject);child.once('exit',code=>{if(!state)reject(Error('OMP exited before get_state: '+code+' '+stderr));});
    child.stdin.write(JSON.stringify({id:'0',type:'set_event_filter',events:null,messageUpdates:'delta'})+'\n'+JSON.stringify({id:'1',type:'get_state'})+'\n');
  });
  let timer;
  try{await Promise.race([handshake,new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error('OMP handshake timeout: '+stderr)),90000);})]);}finally{clearTimeout(timer);}
  assert(ready);assert(state.success);report.state=state;
  report.promptReceived=JSON.parse(fs.readFileSync(proofPath,'utf8'));assert(report.promptReceived.start&&report.promptReceived.end);assert(report.promptReceived.length>=prompt.length);
  // EOF only after handshake; normal pipe lifetime, no early closure.
  child.stdin.end();
  let exitTimer;
  try{report.exitCode=await Promise.race([child.exited,new Promise((_,reject)=>{exitTimer=setTimeout(()=>reject(Error('OMP EOF exit timeout')),15000);})]);}finally{clearTimeout(exitTimer);}
  assert.equal(report.exitCode,0);files.remove(prepared.value);assert(!fs.existsSync(prepared.value));report.cleanup.push('private turn files removed after exit');
  report.status='PASS';
} catch(error) {report.status='FAIL';report.error=error.stack;process.exitCode=1;console.error(error);}
finally {
  if(api)for(const pid of globals.BP_PROCS?.keys()??[]){api.retire('smoke',pid);await api.barrier();}
  save();
}
