// Actual native-listener boundary. Synthetic tailscale CLI fixtures supply
// socket whois identities and this listener's own IP/DNS; no real tailnet,
// account, provider, user home or UI is used. --baseline retains the old401.
// bun pair_owner_auth_e2e.ts BINARY WIREDIR [--baseline] [--keep] [--fixture-only]
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,readdirSync,existsSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
const args=process.argv.slice(2), [binary,wire]=args.filter(x=>!x.startsWith('--'));
if(!binary||!wire)throw Error('BINARY WIREDIR required');
for(const f of readdirSync(wire).filter(f=>f.endsWith('.js')))(0,eval)(readFileSync(join(wire,f),'utf8'));
const W=(globalThis as any).Wire, baseline=args.includes('--baseline');
const root=mkdtempSync(join(tmpdir(),'bp-owner-pair-auth-')), home=join(root,'hub'), fake=join(root,'bin'), userHome=join(root,'user');
for(const p of [home,fake,userHome])mkdirSync(p,{recursive:true,mode:0o700});
const whoisLog=join(root,'whois.log');
const status={Self:{HostName:'auth-alpha',DNSName:'hub.fixture.ts.net.',UserID:1,TailscaleIPs:['127.0.0.2'],Online:true},Peer:{},User:{'1':{LoginName:'OWNER-FIXTURE'}},CurrentTailnet:{MagicDNSSuffix:'fixture.ts.net'}};
writeFileSync(join(fake,'tailscale'),`#!/bin/sh
case "$1" in
 ip) printf '%s\\n' '127.0.0.2';;
 status) printf '%s\\n' '${JSON.stringify(status)}';;
 whois) printf '%s\\n' "$*" >> '${whoisLog}'
 case "$*" in *127.0.0.4*) printf '%s\\n' '{"UserProfile":{"LoginName":"UNTRUSTED-FIXTURE"}}';; *) printf '%s\\n' '{"UserProfile":{"LoginName":"OWNER-FIXTURE"}}';; esac;;
 *) exit 0;;
esac
`,{mode:0o755});
for(const n of ['claude','codex','grok','muse','opencode','pi','omp'])writeFileSync(join(fake,n),'#!/bin/sh\nexit 127\n',{mode:0o755});
const dummy=Bun.serve({hostname:'127.0.0.3',port:0,fetch(req){return new Response(W.encode(JSON.stringify({backplane:'0.12.4',busy:0,commit:'AUTH-FIXTURE'})),{headers:{'content-type':'application/cbor'}})}});
const dummyUrl=`http://127.0.0.3:${dummy.port}`;
const lease=Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}});const port=lease.port;lease.stop(true);
const err=Bun.file(join(root,'hub.stderr'));
const child=Bun.spawn([resolve(binary),'--home',home,'--port',String(port),'--tailscale'],{env:{...process.env,HOME:userHome,PATH:`${fake}:/usr/bin:/bin`,DISPLAY:'',WAYLAND_DISPLAY:'',BACKPLANE_AGENTS:fake,BACKPLANE_NAME:'auth-alpha',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_TELEMETRY:'off',BACKPLANE_KEYRING_LOCKED:'0',BACKPLANE_PEERS:dummyUrl},stdout:Bun.file(join(root,'hub.stdout')),stderr:err});
let ws:WebSocket|undefined, machines:any, failed=false;const timings:any={};
const check=(name:string,ok:boolean,got?:unknown)=>{console.log(`${ok?'ok':'FAIL'} ${name}${ok?'':': '+JSON.stringify(got)}`);if(!ok)failed=true;};
const sleep=(n:number)=>new Promise(r=>setTimeout(r,n));
async function until(ms:number,f:()=>any){const t=performance.now();while(performance.now()-t<ms){const x=await f();if(x)return x;if(child.exitCode!==null)throw Error('fixture hub exited');await sleep(30);}return null;}
// Bun 1.3.10 ignores node:net's localAddress. A Python socket explicitly
// binds before connect, and each tailnet request verifies its own whois row.
const socketRequest=`import base64,json,socket,sys
p=json.load(sys.stdin)
with socket.socket(socket.AF_INET,socket.SOCK_STREAM) as s:
 s.settimeout(10)
 s.bind((p['source'],0))
 s.connect((p['destination'],p['port']))
 local=s.getsockname()[0]
 peer=s.getpeername()[0]
 s.sendall(p['request'].encode('utf-8'))
 chunks=[]
 while True:
  data=s.recv(65536)
  if not data: break
  chunks.append(data)
 print(json.dumps(dict(local=local,peer=peer,response64=base64.b64encode(b''.join(chunks)).decode('ascii'))))
`;
function decodeHttp(response64:string){
 const all=Buffer.from(response64,'base64'), split=all.indexOf(Buffer.from('\r\n\r\n'));
 const head=(split<0?all:all.subarray(0,split)).toString('ascii');
 const status=Number(/^HTTP\/1\.1\s+(\d+)/.exec(head)?.[1]||0);
 if(!status||split<0)throw Error('missing HTTP reply head');
 const bytes=all.subarray(split+4);
 return {status,bytes,body:bytes.toString('utf8')};
}
function whoisRows(source:string){return existsSync(whoisLog)?readFileSync(whoisLog,'utf8').split('\n').filter(l=>l===`whois --json ${source}`).length:0;}
async function raw(host:string,source:string,method:string,path:string,fields:[string,string][]=[],onLoopback=false){
 const t=performance.now(), destination=onLoopback?'127.0.0.1':'127.0.0.2', before=whoisRows(source);
 const request=`${method} ${path} HTTP/1.1\r\nHost: ${host}\r\n${fields.some(([k])=>k.toLowerCase()==='connection')?'':'Connection: close\r\n'}Content-Length: 0\r\n${fields.map(([k,v])=>`${k}: ${v}\r\n`).join('')}\r\n`;
 const client=Bun.spawn(['python3','-c',socketRequest],{stdin:new Response(JSON.stringify({source,destination,port,request})),stdout:'pipe',stderr:'pipe'});
 const [rc,out,err]=await Promise.all([client.exited,new Response(client.stdout).text(),new Response(client.stderr).text()]);
 if(rc!==0)throw Error('bound socket fixture failed: '+err);
 const response=JSON.parse(out);
 if(response.local!==source||response.peer!==destination)throw Error('fixture socket source/destination mismatch');
 if(!onLoopback&&whoisRows(source)!==before+1)throw Error(`request did not verify actual socket source ${source}`);
 return {...decodeHttp(response.response64),ms:performance.now()-t};
}
function peerFiles(){const p=join(home,'secrets/peers');return existsSync(p)?readdirSync(p).length:0;}
try{
 const up=await until(25000,async()=>{try{return (await raw(`127.0.0.2:${port}`,'127.0.0.1','GET','/hello')).status===200}catch{return false}});check('dedicated tailnet listener ready',!!up);if(!up)throw Error('no tailnet listener');
 if(args.includes('--fixture-only')){
  for(const source of ['127.0.0.1','127.0.0.4']){
   const x=await raw(`127.0.0.2:${port}`,source,'GET','/hello');
   const body=JSON.parse(W.decode(new Uint8Array(x.bytes)));
   check(`binary CBOR hello and actual socket source ${source}`,x.status===200&&typeof body.backplane==='string');
  }
  console.log(JSON.stringify({mode:'fixture-only',passed:!failed,scope:'Only binary HTTP transport/source binding against a disposable hub; not pairing or queue qualification'}));
 }else{
 ws=new WebSocket(`ws://127.0.0.1:${port}/ws`);ws.binaryType='arraybuffer';ws.onmessage=e=>{try{const m=JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));if(m.t==='info'&&m.info?.machines){const x=m.info.machines;machines=typeof x==='string'?JSON.parse(x):x;}}catch{}};
 const row=await until(150000,()=>machines?.peers?.find((x:any)=>x.url===dummyUrl));check('actor discovers the synthetic requester first',!!row);if(!row)throw Error('synthetic requester not discovered');
 const path=`/bots/pair?name=${encodeURIComponent(row.name)}&url=${encodeURIComponent(dummyUrl)}`;
 const before=peerFiles();
 const negatives:[string,string,string,[string,string][],boolean][]=[
 ['untrusted socket cannot spoof own Host',`127.0.0.2:${port}`,'127.0.0.4',[],false],
 ['rebound foreign Host refused','attacker.invalid','127.0.0.1',[],false],
 ['lookalike DNS refused',`hub.fixture.ts.net.attacker.invalid:${port}`,'127.0.0.1',[],false],
 ['wrong port refused',`hub.fixture.ts.net:${port+1}`,'127.0.0.1',[],false],
 ['foreign Origin refused',`127.0.0.2:${port}`,'127.0.0.1',[['Origin','https://attacker.invalid']],false],
 ['browser Origin cannot use native-only grant',`127.0.0.2:${port}`,'127.0.0.1',[['Origin',`http://127.0.0.2:${port}`]],false],
 ['proxy asserted identity refused',`127.0.0.2:${port}`,'127.0.0.1',[['Tailscale-User-Login','OWNER-FIXTURE']],false],
 ['cross-site metadata refused',`127.0.0.2:${port}`,'127.0.0.1',[['Sec-Fetch-Site','cross-site']],false],
 ['forwarded authority cannot grant','attacker.invalid','127.0.0.1',[['X-Forwarded-Host',`127.0.0.2:${port}`]],false],
 ['duplicate Host refused',`127.0.0.2:${port}`,'127.0.0.1',[['Host','attacker.invalid']],false],
 ['main listener cannot inherit tailnet aliases',`127.0.0.2:${port}`,'127.0.0.1',[],true],
 ];
 for(const [name,host,source,headers,loopback] of negatives){const x=await raw(host,source,'POST',path,headers,loopback);check(name,x.status===401,x.status);}
 for(const p of ['/debug/perf','/mcp/AUTH-FIXTURE','/ws']){const x=await raw(`127.0.0.2:${port}`,'127.0.0.1',p.startsWith('/mcp/')?'POST':'GET',p,p==='/ws'?[['Upgrade','websocket'],['Connection','Upgrade'],['Sec-WebSocket-Version','13'],['Sec-WebSocket-Key','dGhlIHNhbXBsZSBub25jZQ==']]:[]);check(`ordinary ${p} gate unchanged`,x.status===401,x.status);}
 check('refusals created no invitation secrets',peerFiles()===before);
 for(const host of [`127.0.0.2:${port}`,`hub.fixture.ts.net:${port}`,'127.0.0.2','hub.fixture.ts.net']){
  const x=await raw(host,'127.0.0.1','POST',path);const name=host.startsWith('127')?'IP':'DNS';
  if(baseline){check(`baseline ${name} owner pairing defect reproduced`,x.status===401,x.status);}
  else{let invite='';try{invite=JSON.parse(x.body).invite||''}catch{}check(`verified owner pairs at own ${name} authority`,x.status===200&&!!invite,x.status);}
 }
 const denied=await raw(`127.0.0.2:${port}`,'127.0.0.4','POST',path);check('owner success never grants untrusted socket',denied.status===401,denied.status);
 check('both untrusted requests checked actual .4 socket source',whoisRows('127.0.0.4')===2);
 const hello=await raw(`127.0.0.2:${port}`,'127.0.0.1','GET','/hello');timings.helloAfterRefusalsMs=hello.ms;check('service responds after refused requests',hello.status===200&&hello.ms<1000,hello.ms);
 console.log(JSON.stringify({mode:baseline?'old-401-control':'corrected-native-listener',passed:!failed,sourceKind:'synthetic socket whois fixture; not a live tailnet/phone claim',timings}));
 }
}catch(error){
 failed=true;console.error('fixture error: '+String(error));
}finally{
 ws?.close();dummy.stop(true);if(child.exitCode===null)child.kill('SIGTERM');await Promise.race([child.exited,sleep(2000)]);if(child.exitCode===null){child.kill('SIGKILL');await child.exited;}
 if(failed||args.includes('--keep'))console.log('synthetic fixture retained: '+root);else rmSync(root,{recursive:true,force:true});
}
if(failed)process.exit(1);
