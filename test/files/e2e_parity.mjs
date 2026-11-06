// node/bun e2e_parity.mjs RUST_HUB_BIN JS_HUB_BIN OUT.json
// Starts both hubs (own ports 3850/3851, temp homes, stand-in agents) on the same tiny project, asks both the same
// files.dir and /img questions (hostile ones included) and reports every difference. Desktop-safe: finished binaries only.
import {mkdirSync,rmSync,writeFileSync,symlinkSync,existsSync} from 'node:fs';
import {makeStandins,startHub,stopHub,client,sleep} from '../perf/lib.mjs';
const [rustBin,jsBin,out]=process.argv.slice(2);
const R='/home/h/Documents/i2cjak/backplane-gpui/build/gpui/files-run';rmSync(R,{recursive:true,force:true});mkdirSync(R,{recursive:true});
makeStandins(R+'/bin');
const proj=R+'/proj',outside=R+'/outside';
for(const d of [proj+'/sub/deep',proj+'/zdir',proj+'/.git',outside])mkdirSync(d,{recursive:true});
const png=Buffer.from('89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d49444154789c6360000002000001e221bc330000000049454e44ae426082','hex');
writeFileSync(proj+'/a.png',png);writeFileSync(proj+'/A.PNG',png);writeFileSync(proj+'/note.md','# hi\nline\n');writeFileSync(proj+'/b.kicad_pcb','(kicad_pcb)');writeFileSync(proj+'/sub/c.txt','0123456789');writeFileSync(proj+'/.env','SECRET=1');
writeFileSync(proj+'/doc.pdf','%PDF-1.4\n');writeFileSync(proj+'/key.pem','KEY');writeFileSync(proj+'/empty.png','');writeFileSync(outside+'/secret.txt','outside');writeFileSync(outside+'/s.png',png);
symlinkSync(outside,proj+'/escape');symlinkSync(outside+'/s.png',proj+'/linked.png');symlinkSync(proj+'/note.md',proj+'/linknote.md');symlinkSync('/etc',proj+'/etc');
const log=[{$:'ProjectCreated',id:'p1',title:'proj',root:proj,at:1},{$:'ThreadCreated',id:'t1',project:'p1',title:'T',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:2}].map(x=>JSON.stringify(x)).join('\n')+'\n';
const hubs=[];
for(const [name,bin,port] of [['rust',rustBin,3850],['js',jsBin,3851]]){const home=`${R}/${name}-home`;mkdirSync(home,{recursive:true});writeFileSync(home+'/events.jsonl',log);
 hubs.push({name,h:await startHub({bin,home,port,standins:R+'/bin'})})}
const dirs=['','sub','sub/deep','zdir','..','../outside','sub/..','sub/../..','/etc','/','escape','escape/','etc','missing','.git','a.png','%2e%2e','sub//deep','./sub','sub\0x'];
const imgs=p=>[[p,{}],[p,{range:'bytes=2-4'}],[p,{range:'bytes=-3'}],[p,{range:'bytes=100-'}],[p,{range:'bytes=0-1,3-4'}],[p,{range:'junk'}]];
const paths=[proj+'/a.png',proj+'/A.PNG',proj+'/note.md',proj+'/sub/c.txt',proj+'/doc.pdf',proj+'/b.kicad_pcb',proj+'/key.pem',proj+'/.env',proj+'/empty.png',proj+'/linked.png',proj+'/linknote.md',proj+'/escape/s.png',outside+'/s.png',outside+'/secret.txt',proj+'/missing.png',proj+'/sub','/etc/passwd','/etc/hostname.png','relative.png','~/nonexistent.png',proj+'/../outside/s.png','file://'+proj+'/a.png',proj.replace(/\//g,'%2F')+'%2Fa.png'];
const results=[];
for(const {name,h} of hubs){
 const c=await client(h,{view:true});const r={name,files:[],img:[]};
 for(const th of ['t1','nope'])for(const d of dirs){const x=await c.rpc('files.dir',{thread:th,path:d},5000);const o=x?.o;r.files.push({thread:th,path:d,ok:o?.ok,result:o?.result,error:o?.error})}
 for(const p of paths)for(const [pp,{range}] of imgs(p)){const res=await fetch(`${h.url}/img?path=${encodeURIComponent(pp)}`,{headers:{authorization:'Bearer '+h.token,...(range?{range}:{})}});const b=Buffer.from(await res.arrayBuffer());
  r.img.push({path:pp,range:range??'',status:res.status,ctype:res.headers.get('content-type'),cr:res.headers.get('content-range'),ar:res.headers.get('accept-ranges'),cc:res.headers.get('cache-control'),nosniff:res.headers.get('x-content-type-options'),len:res.headers.get('content-length'),body:b.length>200?b.length+':'+b.subarray(0,16).toString('hex'):b.toString('latin1')})}
 const noauth=await fetch(`${h.url}/img?path=${encodeURIComponent(proj+'/a.png')}`);r.noauth=noauth.status;
 c.ws.close();results.push(r)}
for(const {h} of hubs)stopHub(h);
const [a,b]=results;const diffs=[];
const norm=r=>JSON.stringify(r,(k,v)=>k==='fmRoot'?undefined:v);
a.files.forEach((x,i)=>{if(norm(x)!==norm(b.files[i]))diffs.push({kind:'files.dir',rust:x,js:b.files[i]})});
a.img.forEach((x,i)=>{if(JSON.stringify(x)!==JSON.stringify(b.img[i]))diffs.push({kind:'img',rust:x,js:b.img[i]})});
if(a.noauth!==b.noauth)diffs.push({kind:'noauth',rust:a.noauth,js:b.noauth});
writeFileSync(out,JSON.stringify({files:a.files.length,img:a.img.length,diffs:diffs.length,detail:diffs},null,1));
console.log(`files.dir cases ${a.files.length}, /img cases ${a.img.length}, differences ${diffs.length}`);
for(const d of diffs.slice(0,12))console.log(JSON.stringify(d).slice(0,500));
process.exit(0);
