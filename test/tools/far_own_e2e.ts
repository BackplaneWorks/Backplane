// End to end: which linked hubs count as the owner's machines (client.bend's
// Far.mine, server.bend's Far.own). A hub that holds an invite and says it is
// one of the owner's machines (the address of one in its POST /bots/link) is
// refused on /far/*; a link becomes the owner's only when the hub verified it:
// paired through POST /bots/pair ("pair"), or a signed request of the link's
// came from one of the owner's devices as Tailscale says ("tailnet": also how
// links made before an upgrade verify themselves). A synthetic tailscale CLI
// says this hub is 127.0.0.2 and whose each socket source is (127.0.0.4 is
// someone else's device, any other address the owner's); no real tailnet,
// account or provider is used, and the hub's home is a temporary folder.
//
//   bun test/tools/far_own_e2e.ts BINARY [--baseline] [--keep]
//
// --baseline: against a build from before the fix, expect the old answer
// (the claiming link let in) so the test is known to tell them apart.
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,existsSync,rmSync,chmodSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createHmac} from 'node:crypto';
import {connect} from '../../skills/backplane-e2e/rpc.mjs';
const args=process.argv.slice(2), [binary]=args.filter(x=>!x.startsWith('--'));
if(!binary)throw Error('BINARY required');
const baseline=args.includes('--baseline');
const root=mkdtempSync(join(tmpdir(),'bp-far-own-')), home=join(root,'hub'), fake=join(root,'bin'), userHome=join(root,'user');
for(const p of [home,fake,userHome])mkdirSync(p,{recursive:true,mode:0o700});
const status={Self:{HostName:'victim',DNSName:'victim.fixture.ts.net.',UserID:1,TailscaleIPs:['127.0.0.2'],Online:true},Peer:{},User:{'1':{LoginName:'OWNER-FIXTURE'}},CurrentTailnet:{MagicDNSSuffix:'fixture.ts.net'}};
writeFileSync(join(fake,'tailscale'),`#!/bin/sh
case "$1" in
 ip) printf '%s\\n' '127.0.0.2';;
 status) printf '%s\\n' '${JSON.stringify(status)}';;
 whois) case "$*" in *127.0.0.4*) printf '%s\\n' '{"UserProfile":{"LoginName":"UNTRUSTED-FIXTURE"}}';; *) printf '%s\\n' '{"UserProfile":{"LoginName":"OWNER-FIXTURE"}}';; esac;;
 *) exit 0;;
esac
`,{mode:0o755});
for(const n of ['claude','codex','grok','muse','opencode','pi','omp'])writeFileSync(join(fake,n),'#!/bin/sh\nexit 127\n',{mode:0o755});
// "desk": one of the owner's machines this hub finds (BACKPLANE_PEERS), a
// stand-in that answers /hello and nothing else
const hello=new Uint8Array([0xa1,0x69,...Buffer.from('backplane'),0x65,...Buffer.from('0.1.0')]);
const desk=Bun.serve({hostname:'127.0.0.3',port:Number(process.env.FAR_OWN_DESK_PORT||0),fetch(req){return new URL(req.url).pathname==='/hello'?new Response(hello,{headers:{'content-type':'application/cbor'}}):new Response('no',{status:404})}});
const deskUrl=`http://127.0.0.3:${desk.port}`;
const port=Number(process.env.FAR_OWN_PORT||0)||(()=>{const l=Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}});const p=l.port;l.stop(true);return p;})();
const child=Bun.spawn([resolve(binary),'--home',home,'--port',String(port),'--tailscale'],{env:{...process.env,HOME:userHome,PATH:`${fake}:/usr/bin:/bin`,DISPLAY:'',WAYLAND_DISPLAY:'',BACKPLANE_AGENTS:fake,BACKPLANE_NAME:'victim',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_TELEMETRY:'off',BACKPLANE_KEYRING_LOCKED:'0',BACKPLANE_PEERS:deskUrl},stdout:Bun.file(join(root,'hub.stdout')),stderr:Bun.file(join(root,'hub.stderr'))});
let failed=false;
const check=(name:string,ok:boolean,got?:unknown)=>{console.log(`${ok?'ok':'FAIL'} ${name}${ok?'':': '+JSON.stringify(got)}`);if(!ok)failed=true;};
const sleep=(n:number)=>new Promise(r=>setTimeout(r,n));
async function until(ms:number,f:()=>any){const t=performance.now();while(performance.now()-t<ms){const x=await f();if(x)return x;if(child.exitCode!==null)throw Error('fixture hub exited');await sleep(200);}return null;}
// a socket bound to a chosen source address (Bun ignores localAddress), so
// the hub's whois sees the device the test plays
const socketRequest=`import base64,json,socket,sys
p=json.load(sys.stdin)
with socket.socket(socket.AF_INET,socket.SOCK_STREAM) as s:
 s.settimeout(15)
 s.bind((p['source'],0))
 s.connect((p['destination'],p['port']))
 s.sendall(base64.b64decode(p['request']))
 chunks=[]
 while True:
  data=s.recv(65536)
  if not data: break
  chunks.append(data)
 print(base64.b64encode(b''.join(chunks)).decode('ascii'))
`;
async function raw(source:string,method:string,path:string,headers:[string,string][]=[],body=''){
 const b=Buffer.from(body,'utf8');
 const head=`${method} ${path} HTTP/1.1\r\nHost: 127.0.0.2:${port}\r\nConnection: close\r\nContent-Length: ${b.length}\r\n${headers.map(([k,v])=>`${k}: ${v}\r\n`).join('')}\r\n`;
 const request=Buffer.concat([Buffer.from(head,'utf8'),b]).toString('base64');
 const c=Bun.spawn(['python3','-c',socketRequest],{stdin:new Response(JSON.stringify({source,destination:'127.0.0.2',port,request})),stdout:'pipe',stderr:'pipe'});
 const [rc,out,err]=await Promise.all([c.exited,new Response(c.stdout).text(),new Response(c.stderr).text()]);
 if(rc!==0)throw Error('bound socket failed: '+err);
 const all=Buffer.from(out.trim(),'base64'), split=all.indexOf(Buffer.from('\r\n\r\n'));
 return {status:Number(/^HTTP\/1\.1\s+(\d+)/.exec(all.subarray(0,Math.max(split,0)).toString('ascii'))?.[1]||0),body:all.subarray(split+4).toString('utf8')};
}
// a signed request of a link's, as another hub sends it (docs/bots.md)
function signed(source:string,path:string,link:string,secret:string,body:string,method='POST'){
 const ts=Math.floor(Date.now()/1000)+'', sig='sha256='+createHmac('sha256',Buffer.from(secret,'hex')).update(`${ts}.${body}`).digest('hex');
 return raw(source,method,path,[['Content-Type','application/json'],['X-Backplane-Peer',link],['X-Backplane-Timestamp',ts],['X-Backplane-Signature',sig]],body);
}
// /far/log as a linked hub pulls: the first ask builds the feed ("503 not
// ready", past the gate), so ask again until it answers otherwise
async function pull(source:string,link:string,secret:string){
 // (each ask its own body, unique across pulls too: a signature is
 // accepted once, and two pulls can fall in the same second)
 const nonce=()=>`${Date.now()}-${Math.random().toString(36).slice(2)}`;
 let r=await signed(source,'/far/log',link,secret,JSON.stringify({since:0,ask:0,nonce:nonce()}));
 for(let i=1;i<50&&r.status===503;i++){await sleep(200);r=await signed(source,'/far/log',link,secret,JSON.stringify({since:0,ask:i,nonce:nonce()}));}
 return r;
}
// an invite this hub made and recorded, as Bots > link a machine makes one
// (bots.invite over the main listener with the token; a hub links only
// with an invite it recorded)
async function invite(){
 const c=await connect(port,readFileSync(join(home,'token'),'utf8').trim());
 try{const r=await c.call('bots.invite',{}), m=/^bp1:.*:(p[0-9a-f]+):([0-9a-f]{64})$/.exec(r.invite||'');if(!m)throw Error('no invite: '+JSON.stringify(r));return {link:m[1],secret:m[2]};}
 finally{c.close();}
}
function own(link:string){
 let how='';
 for(const l of readFileSync(join(home,'events.jsonl'),'utf8').split('\n'))if(l.includes(`"peer.own.${link}"`)){try{how=JSON.parse(l).value??JSON.parse(l).v??how;}catch{const m=/"value":"([a-z]*)"/.exec(l);if(m)how=m[1];}}
 return how;
}
const claim=JSON.stringify({url:deskUrl,name:'127.0.0.3'});
try{
 check('tailnet listener ready',!!await until(30000,async()=>{try{return (await raw('127.0.0.1','GET','/hello')).status===200}catch{return false}}));
 // the owner's device asks to pair as desk once the hub has found desk: the
 // invite's link is recorded as the owner's before it is answered
 const pair=await until(150000,async()=>{const r=await raw('127.0.0.1','POST',`/bots/pair?name=127.0.0.3&url=${encodeURIComponent(deskUrl)}`);return r.status===200?r:null;});
 check('hub found desk and the owner\'s device got an invite',!!pair,pair);
 const inv=JSON.parse(pair?.body||'{}').invite||'', m=/^bp1:(.*):([A-Za-z0-9_-]+):([0-9a-f]+)$/.exec(inv);
 check('the invite reads',!!m,inv);
 // someone else's device holds an invite (made for them) and says it is desk
 const E=await invite(), evil=E.secret;
 const l1=await signed('127.0.0.4','/bots/link',E.link,evil,claim);check('an invite\'s holder links',l1.status===200,l1);
 await sleep(500);
 const f1=await pull('127.0.0.4',E.link,evil);
 if(baseline)check('baseline: the link that says it is desk was let in',f1.status===200,f1.status);
 else{
  check('a link that only says it is desk is refused on /far/log',f1.status===403,f1);
  const r1=await signed('127.0.0.4','/far/rpc',E.link,evil,JSON.stringify({id:'1',m:'fs.list',p:{path:'/'}}));check('and on /far/rpc',r1.status===403,r1);
  check('nothing recorded it as the owner\'s',own(E.link)==='',own(E.link));
  // the pair invite, linked from a machine Tailscale does not vouch for (a
  // LAN BACKPLANE_PEERS machine): its link is the owner's from the pairing
  const l2=await signed('127.0.0.4','/bots/link',m![2],m![3],claim);check('the paired machine links',l2.status===200,l2);
  await sleep(500);
  check('paired through /bots/pair: recorded',own(m![2])==='pair',own(m![2]));
  const f2=await pull('127.0.0.4',m![2],m![3]);check('the paired machine may pull',f2.status===200,f2.status);
  // a link made before the hub recorded this (no record), from one of the
  // owner's devices: its first signed request over the tailnet verifies it
  const O=await invite(), old=O.secret;
  const l3=await signed('127.0.0.4','/bots/link',O.link,old,claim);check('an older link',l3.status===200,l3);
  await sleep(500);
  const f3=await pull('127.0.0.4',O.link,old);check('unverified, it is refused',f3.status===403,f3.status);
  const d3=await signed('127.0.0.5','/bots/dir',O.link,old,'','GET');check('it asks for the directory from the owner\'s device',d3.status===200,d3);
  await sleep(500);
  check('Tailscale vouched: recorded',own(O.link)==='tailnet',own(O.link));
  const f4=await pull('127.0.0.4',O.link,old);check('then it may pull',f4.status===200,f4.status);
  // someone else's device never verifies a link, however often it asks
  for(let i=0;i<2;i++)await signed('127.0.0.4','/bots/dir',E.link,evil,'','GET');
  await sleep(500);
  check('someone else\'s device verified nothing',own(E.link)==='',own(E.link));
  // a forged signature from the owner's device verifies nothing either
  const forged=await signed('127.0.0.5','/bots/dir',E.link,'00'.repeat(32),'','GET');check('a bad signature is refused',forged.status===401,forged.status);
  check('and recorded nothing',own(E.link)==='',own(E.link));
 }
}catch(e){failed=true;console.error('fixture error: '+String(e));}
finally{
 desk.stop(true);if(child.exitCode===null)child.kill('SIGTERM');await Promise.race([child.exited,sleep(3000)]);if(child.exitCode===null){child.kill('SIGKILL');await child.exited;}
 if(failed||args.includes('--keep'))console.log('fixture kept: '+root);else rmSync(root,{recursive:true,force:true});
}
if(failed)process.exit(1);
