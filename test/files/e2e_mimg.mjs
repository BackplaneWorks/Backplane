// bun e2e_mimg.mjs RUST_HUB JS_HUB OUT.json : GET /message-image on both hubs (own ports 3850/3851, stand-in agents, synthetic log, Boat or desktop).
// Messages in the log name images; each case asks both hubs the same thing and compares status, headers and body bytes.
import {mkdirSync,rmSync,writeFileSync,symlinkSync,createWriteStream} from 'node:fs';
import {createHash} from 'node:crypto';
import {makeStandins,startHub,stopHub} from '../perf/lib.mjs';
const [rustBin,jsBin,out]=process.argv.slice(2);
const R=(process.env.PARITY_WORK||'/tmp/mimg-work')+'/mimg';rmSync(R,{recursive:true,force:true});mkdirSync(R+'/img/sub',{recursive:true});makeStandins(R+'/bin');
const be=(n,l=4)=>{const b=Buffer.alloc(l);b.writeUIntBE(n,0,l);return b};
const png=(w,h)=>Buffer.concat([Buffer.from('89504e470d0a1a0a','hex'),be(13),Buffer.from('IHDR'),be(w),be(h),Buffer.from([8,6,0,0,0]),Buffer.alloc(20)]);
const gif=Buffer.concat([Buffer.from('GIF89a'),Buffer.from([4,0,3,0,0,0,0]),Buffer.alloc(8)]);
const jpg=Buffer.from([0xff,0xd8,0xff,0xc0,0,17,8,0,20,0,30,3,1,0x22,0,2,0x11,0,3,0x11,0,0xff,0xd9]);
const bmp=(()=>{const b=Buffer.alloc(60);b.write('BM');b.writeUInt32LE(40,14);b.writeInt32LE(5,18);b.writeInt32LE(-6,22);return b})();
const webp=(()=>{const b=Buffer.alloc(40);b.write('RIFF');b.write('WEBP',8);b.write('VP8X',12);b.writeUIntLE(9,24,3);b.writeUIntLE(9,27,3);return b})();
const files={'a.png':png(4,4),'b.gif':gif,'c.jpg':jpg,'d.bmp':bmp,'e.webp':webp,'svg.png':Buffer.from('<svg xmlns="http://www.w3.org/2000/svg"/>'),'huge.png':png(50000,50000),'zero.png':Buffer.alloc(0),'sub/f.png':png(8,8),'gone.png':png(2,2),'UP.PNG':png(3,3)};
for(const [n,b] of Object.entries(files))writeFileSync(`${R}/img/${n}`,b);
writeFileSync(`${R}/img/big.png`,Buffer.concat([png(10,10),Buffer.alloc(25<<20)]));
symlinkSync(`${R}/img/a.png`,`${R}/img/link.png`);symlinkSync(`${R}/img/sub`,`${R}/img/linkdir`);
const P=n=>`${R}/img/${n}`;
const names=['a.png','b.gif','c.jpg','d.bmp','e.webp','svg.png','huge.png','zero.png','sub/f.png','gone.png','UP.PNG','big.png','link.png','linkdir/f.png'];
const ev=[{$:'ProjectCreated',id:'p1',title:'p',root:R,at:1},{$:'ThreadCreated',id:'t1',project:'p1',title:'T',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:2},{$:'ThreadCreated',id:'t2',project:'p1',title:'D',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:3},{$:'ThreadDeleted',id:'t2',at:4}];
names.forEach((n,i)=>ev.push({$:'MessagePosted',thread:'t1',msg:`m${i}`,role:'assistant',text:`look at ${P(n)} now`,at:10+i}));
ev.push({$:'MessagePosted',thread:'t1',msg:'mu',role:'user',text:`user says ${P('a.png')}`,at:40},{$:'MessagePosted',thread:'t2',msg:'md',role:'assistant',text:`deleted ${P('a.png')}`,at:41},
  {$:'MessagePosted',thread:'t1',msg:'mrel',role:'assistant',text:'rel images/x.png and ~/nothere.png and /etc/passwd.png',at:42},{$:'MessagePosted',thread:'t1',msg:'mtilde',role:'assistant',text:`../x.png ${R}/img/../img/a.png`,at:43});
const log=ev.map(x=>JSON.stringify(x)).join('\n')+'\n';
const cases=[];
names.forEach((n,i)=>cases.push({name:`m${i} ${n}`,entry:`m${i}`,path:P(n)}));
cases.push({name:'path not named by the message',entry:'m0',path:P('b.gif')},{name:'user message',entry:'mu',path:P('a.png')},{name:'deleted thread',entry:'md',path:P('a.png')},{name:'unknown entry',entry:'nope',path:P('a.png')},{name:'empty entry',entry:'',path:P('a.png')},
  {name:'dotdot spelled',entry:'mtilde',path:`${R}/img/../img/a.png`},{name:'not an image ext',entry:'mrel',path:'/etc/passwd.png'},{name:'relative',entry:'mrel',path:'images/x.png'},{name:'tilde',entry:'mrel',path:'~/nothere.png'},
  {name:'backslash',entry:'m0',path:P('a.png')+'\\'},{name:'no path',entry:'m0',path:''},{name:'no token',entry:'m0',path:P('a.png'),tok:null},{name:'wrong token',entry:'m0',path:P('a.png'),tok:'wrong'},{name:'bearer',entry:'m0',path:P('a.png'),bearer:true});
const sha=b=>createHash('sha256').update(b).digest('hex');
const res={};
for(const [name,bin,port] of [['rust',rustBin,3850],['js',jsBin,3851]]){
 const home=`${R}/${name}-home`;mkdirSync(home,{recursive:true});writeFileSync(home+'/events.jsonl',log);
 const h=await startHub({bin,home,port,standins:R+'/bin'});const r=[];
 for(const c of cases){
  const q=`entry=${encodeURIComponent(c.entry)}&path=${encodeURIComponent(c.path)}`;
  const tok=c.tok===undefined?h.token:c.tok;
  const url=`${h.url}/message-image?${q}${tok&&!c.bearer?`&token=${encodeURIComponent(tok)}`:''}`;
  const rs=await fetch(url,{headers:c.bearer?{authorization:'Bearer '+h.token}:{}});const b=Buffer.from(await rs.arrayBuffer());
  const H=k=>rs.headers.get(k);
  r.push({case:c.name,status:rs.status,ctype:H('content-type'),len:H('content-length'),cc:H('cache-control'),ar:H('accept-ranges'),nosniff:H('x-content-type-options'),sha:b.length>200?sha(b):b.toString('latin1')});
 }
 stopHub(h);res[name]=r;
}
const diffs=[];res.rust.forEach((x,i)=>{if(JSON.stringify(x)!==JSON.stringify(res.js[i]))diffs.push({rust:x,js:res.js[i]})});
writeFileSync(out,JSON.stringify({cases:cases.length,diffs:diffs.length,detail:diffs,rust:res.rust},null,1));
console.log(`message-image cases ${cases.length}, differences ${diffs.length}`);for(const d of diffs)console.log(JSON.stringify(d).slice(0,500));
process.exit(0);
