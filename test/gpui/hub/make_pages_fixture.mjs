// bun make_pages_fixture.mjs : builds test/gpui/hub/fixtures/pages.jsonl (a synthetic log with cross-thread
// messages, long activities, reminders), lets the JS hub build its store.db from it, and writes pages.json:
// for every thread and several `before` ordinals, the page the JS store answers (the repo's TS equivalents of
// Ts.page, verified against the Bend hub in test/perf/bend-vs-ts). backplane-hub's page index must equal it.
import {readFileSync,writeFileSync,mkdirSync,rmSync} from 'node:fs';
import {Database} from 'bun:sqlite';
import {makeStandins,startHub,stopHub,sleep} from '../perf/lib.mjs';
import * as ts from '../../perf/bend-vs-ts/equivalents';
const R='/home/h/Documents/i2cjak/backplane-gpui', F=R+'/test/gpui/hub/fixtures';
const base=readFileSync(R+'/build/gpui/slim-run/msg20x2000.jsonl','utf8').trimEnd().split('\n').slice(0,21+800);
const out=[...base];let at=1785100000;
for(let i=0;i<30;i++)out.push(JSON.stringify({$:'MessagePosted',thread:'t'+(5+i%3),msg:`t:t${3+i%2}:c${i}`,role:'assistant',text:`from another thread ${i}`,at:at++}));
for(let i=0;i<40;i++)out.push(JSON.stringify({$:'ActivityLogged',thread:'t'+(2+i%2),act:'a'+i,tone:'tool',kind:'Bash',summary:('x'.repeat(600)+'é日🙂').slice(0,600+i)+'é日🙂',at:at++}));
for(let i=0;i<5;i++)out.push(JSON.stringify({$:'Reminded',id:'r'+i,bot:'b1',thread:'t1',text:'remember '+i,at:at++}));
for(let i=0;i<20;i++)out.push(JSON.stringify({$:'MessagePosted',thread:'t7',msg:'u'+i,role:'user',text:'日本語🙂 line\nwith newline '+i,at:at++}));
writeFileSync(F+'/pages.jsonl',out.join('\n')+'\n');
const root=R+'/build/gpui/hub-run/pagesfix';rmSync(root,{recursive:true,force:true});mkdirSync(root+'/home',{recursive:true});
writeFileSync(root+'/home/events.jsonl',out.join('\n')+'\n');makeStandins(root+'/bin');
const h=await startHub({bin:R+'/build/gpui/backplane-serve',home:root+'/home',port:3816,standins:root+'/bin'});
await sleep(4000);stopHub(h);await sleep(1500);
const db=new Database(root+'/home/store.db',{readonly:true,safeIntegers:true});
const threads=db.query("select thread, count(*) n from entry group by thread").values().map(r=>[String(r[0]),Number(r[1])]);
const pages=[];
for(const [t,n] of threads){
 const ords=db.query('select ord from entry where thread=?1 or src=?1 order by seq').values(t).map(r=>Number(r[0]));
 for(const before of new Set([0,ords[Math.floor(ords.length/3)]??0,ords[Math.floor(2*ords.length/3)]??0,ords[0]??0,(ords.at(-1)??0)+5])){
  const p=ts.page(db,t,before,32);
  pages.push({thread:t,before,ids:p.ids,below:p.below,above:p.above,rows:p.rows.map(r=>[Number(r[0]),Number(r[1]),r[2],r[3],r[4],r[5],r[6],r[7],r[8],Number(r[9])])});
 }}
// every entry, to check the index itself
const entries=db.query('select seq,ord,id,thread,src,kind,role,akind,text,at from entry order by seq').values().map(r=>[Number(r[0]),Number(r[1]),r[2],r[3],r[4],r[5],r[6],r[7],r[8],Number(r[9])]);
writeFileSync(F+'/pages.json',JSON.stringify({entries:entries.length,pages}));
writeFileSync(F+'/entries.json',JSON.stringify(entries));
console.log(JSON.stringify({lines:out.length,entries:entries.length,pages:pages.length,threads:threads.length}));process.exit(0);
