// Isolated fake runtime integration: temp home, free port, fake kicad-cli.
// The hubs it spawns are stopped at the end, like the other e2e tests.
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
const hubs: ReturnType<typeof Bun.spawn>[] = [];
// a thrown wait still stops the hubs this test started
const bail = (e: unknown) => { console.log('FAIL e2e completed: ' + String(e)); for (const h of hubs) if (h.exitCode === null) h.kill('SIGTERM'); process.exit(1); };
process.on('uncaughtException', bail); process.on('unhandledRejection', bail);
const [binary = 'build/backplane-export', wire = 'build/wire'] = process.argv.slice(2);
for (const f of readdirSync(wire).filter(f => f.endsWith('.js'))) (0, eval)(readFileSync(join(wire, f), 'utf8'));
const W = (globalThis as any).Wire;
const root = mkdtempSync(join(tmpdir(), 'bp-export-e2e-'));
const home = join(root, 'home'), project = join(root, 'design'), rt = join(root, 'runtime'), fake = join(root, 'fake');
for (const d of [home, project, join(rt, 'bin'), fake]) mkdirSync(d, {recursive: true});
const pro = join(project, 'board.kicad_pro');
writeFileSync(pro, '{}'); writeFileSync(join(project, 'board.kicad_pcb'), '(kicad_pcb)');
writeFileSync(join(project, '.backplane.json'), JSON.stringify({project: 'board.kicad_pro'}));
writeFileSync(join(rt, 'manifest.json'), JSON.stringify({sourceRepository: 'Backplane_KiCad', version: '10.99.0-bp.1', kicadVersion: '10.99.0', apiVersion: 1, capabilities: ['export-v10']}));
const mode = join(root, 'mode'), argv = join(root, 'argv'); writeFileSync(mode, 'empty');
const cli = join(rt, 'bin/kicad-cli');
writeFileSync(cli, `#!/usr/bin/env bun
import {readFileSync,writeFileSync,appendFileSync,mkdirSync,existsSync,readdirSync} from 'node:fs';
import {join,basename} from 'node:path';
const a=process.argv.slice(2); if(a[0]==='version'){console.log('10.99.0-bp.1');process.exit(0)}
if(a[0]!=='project'||a[1]!=='downgrade')process.exit(2);
appendFileSync(process.env.EXPORT_ARGV,JSON.stringify(a)+'\\n');
const mode=readFileSync(process.env.EXPORT_MODE,'utf8').trim(), dry=a.includes('--dry-run');
const entries=mode==='empty'?[]:mode==='block'?[{category:'block',feature:'Styled endings',detail:'Cannot represent',count:1}]:[{category:'lower',feature:'Text',detail:'Converted',count:1},{category:'drop',feature:'Metadata',detail:'Removed',count:1},{category:'keep',feature:'Companion',detail:'board.backplane.json',count:1}];
writeFileSync(a[a.indexOf('--report-json')+1],JSON.stringify({target:'10.0',blocked:mode==='block',lossy:mode!=='empty',entries}));
if(!dry){const out=a[a.indexOf('-o')+1]; if(existsSync(out)&&readdirSync(out).length)process.exit(8);if(mode!=='empty'&&!a.includes('--force'))process.exit(9);if(mode==='block'&&!a.includes('--drop-approximations'))process.exit(10);mkdirSync(out,{recursive:true});writeFileSync(join(out,'board.kicad_pro'),'{}');}
`); chmodSync(cli, 0o755);
for (const n of ['claude','codex','grok','muse','agy','curl']) {const p=join(fake,n);writeFileSync(p,'#!/bin/sh\nexit 1\n');chmodSync(p,0o755)}
// An installed stock-10 stand-in verifies the detached launch gets only
// the copied project's path. Runtime discovery stays pinned by the env.
const opened = join(root, 'opened');
writeFileSync(join(fake,'kicad-cli'), '#!/bin/sh\n[ "$1" = version ] || exit 2\nprintf \'10.0.7\\n\'\n');chmodSync(join(fake,'kicad-cli'),0o755);
writeFileSync(join(fake,'kicad'), '#!/bin/sh\nprintf \'%s\\n\' "$1" >> "$EXPORT_OPENED"\n');chmodSync(join(fake,'kicad'),0o755);
const skills=join(home,'.cache/backplane/kistack/ecf16c119c7ed610f5f251c479b03884b8d98b09/skills/pcb');mkdirSync(skills,{recursive:true});writeFileSync(join(skills,'SKILL.md'),'---\nname: fixture\ndescription: fixture\n---\n');
const listen=Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}});const port=listen.port;listen.stop(true);if(port===3787)throw Error('refuse live port');
const proc=Bun.spawn([resolve(binary),'--foreground','--home',home,'--port',String(port),'--no-tailscale'],{env:{...process.env,HOME:home, XDG_CACHE_HOME:join(home,'.cache'),PATH:`${fake}:${process.env.PATH}`,DISPLAY:'',WAYLAND_DISPLAY:'',BACKPLANE_HOME:home,BACKPLANE_KICAD:rt,BACKPLANE_KICAD_CLI:'',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:join(home,'locks'),EXPORT_MODE:mode,EXPORT_ARGV:argv,EXPORT_OPENED:opened},stdout:Bun.file(join(home,'stdout.log')),stderr:Bun.file(join(home,'stderr.log'))});hubs.push(proc);
let failed=false; function check(n:string,b:boolean,v?:any){console.log(`${b?'ok':'FAIL'} ${n}${b?'':': '+JSON.stringify(v)}`);if(!b)failed=true}
const sleep=(ms:number)=>new Promise(r=>setTimeout(r,ms));
async function until<T>(f:()=>T|undefined,ms=40000):Promise<T>{const end=Date.now()+ms;while(Date.now()<end){const v=f();if(v!==undefined)return v;await sleep(40)}throw Error('timeout; hub stderr: '+readFileSync(join(home,'stderr.log'),'utf8').slice(-4000))}
await until(()=>existsSync(join(home,'token'))?true:undefined);
let ready=false;for(let i=0;i<400;i++){try{ready=(await fetch(`http://127.0.0.1:${port}/hello`)).ok}catch{}if(ready)break;await sleep(50)}if(!ready)throw Error('hub unavailable');
const seen:any[]=[], changes:any[]=[];const ws=new WebSocket(`ws://127.0.0.1:${port}/ws`);ws.binaryType='arraybuffer';ws.onmessage=e=>{const o=JSON.parse(W.decode(new Uint8Array(e.data)));seen.push(o);changes.push(...(o.items??[]))};await new Promise<void>((r,j)=>{ws.onopen=()=>r();ws.onerror=j});
let id=0;async function rpc(m:string,p:any={}){const n=++id;ws.send(W.encode(JSON.stringify({id:n,m,p})));return await until(()=>seen.find(o=>o.t==='reply'&&String(o.id)===String(n)))}
await until(()=>seen.find(o=>(o.t==='info'||o.t==='log')&&o.info?.['kicad.status']==='ok'));
await rpc('project.add',{path:project});
const pid=(await until(()=>changes.find(c=>c.$==='ProjectCreated'&&c.root===project))).id;
await rpc('thread.create',{project:pid,title:'export test',env:'local'});
const thread=(await until(()=>changes.find(c=>c.$==='ThreadCreated'&&c.project===pid)??changes.find(c=>c.$==='ThreadCreated'))).id;
check('thread created',!!thread);if(!thread)throw Error('missing thread');
const result=(r:any)=>r.result??r;
let r=result(await rpc('kicad.export10',{thread}));check('empty exports immediately',r.export10==='done',r);check('outside project',r.path.startsWith(home+'/exports/')&&!r.path.startsWith(project+'/'),r);
const out=r.path, base=dirname(out.endsWith('/')?out.slice(0,-1):out);
await until(()=>existsSync(opened)?true:undefined);check('opens copied project in stock 10',readFileSync(opened,'utf8').trim()===join(out,'board.kicad_pro'));
check('marker in parent only',existsSync(join(base,'.backplane-export'))&&!existsSync(join(out,'.backplane-export')));
let calls=readFileSync(argv,'utf8').trim().split('\n').map(l=>JSON.parse(l));check('empty dry then export without force',calls.length===2&&calls[0].includes('--dry-run')&&!calls[1].includes('--dry-run')&&!calls[1].includes('--force'),calls);
writeFileSync(join(out,'old-copy'),'old');r=result(await rpc('kicad.export10',{thread}));check('marked re-export clears old copy',r.export10==='done'&&!existsSync(join(out,'old-copy')),r);
writeFileSync(mode,'lossy');r=result(await rpc('kicad.export10',{thread}));check('lossy asks first',r.export10==='confirm'&&r.report.entries.length===3,r);const lossyReport=JSON.stringify(r.report);
r=result(await rpc('kicad.export10',{thread,anyway:true,report:lossyReport}));calls=readFileSync(argv,'utf8').trim().split('\n').map(l=>JSON.parse(l));let last=calls.at(-1);check('lossy force only',r.export10==='done'&&last.includes('--force')&&!last.includes('--drop-approximations'),{r,last});
writeFileSync(mode,'block');r=result(await rpc('kicad.export10',{thread}));check('block asks first',r.export10==='confirm',r);r=result(await rpc('kicad.export10',{thread,anyway:true,report:JSON.stringify(r.report)}));last=JSON.parse(readFileSync(argv,'utf8').trim().split('\n').at(-1)!);check('block force and drop',r.export10==='done'&&last.includes('--force')&&last.includes('--drop-approximations'),{r,last});
// A fresh report must match the one consented to.
writeFileSync(mode,'lossy');r=result(await rpc('kicad.export10',{thread}));writeFileSync(mode,'block');r=result(await rpc('kicad.export10',{thread,anyway:true,report:JSON.stringify(r.report)}));check('changed report asks again',r.export10==='confirm',r);
const unmarked=join(home,'exports/x');mkdirSync(unmarked,{recursive:true});writeFileSync(join(unmarked,'keep'),'keep');
// Target a second named project to precreate its exact expected base.
const p2=join(project,'other.kicad_pro');writeFileSync(p2,'{}');
function hash(s:string,h:number){for(const c of s)h=Math.imul(h^c.codePointAt(0)!,16777619)>>>0;return h}
const b2=join(home,`exports/other-${hash(p2,2166136261)}-${hash(p2,131)}`);mkdirSync(join(b2,'kicad10'),{recursive:true});writeFileSync(join(b2,'kicad10/keep'),'keep');writeFileSync(join(project,'.backplane.json'),JSON.stringify({project:'other.kicad_pro'}));r=result(await rpc('kicad.export10',{thread}));check('unmarked exact base refused',r.export10==='error'&&existsSync(join(b2,'kicad10/keep')),r);check('unrelated unmarked folder untouched',readFileSync(join(unmarked,'keep'),'utf8')==='keep');
// A symlinked exact base must not allow the clear to reach the design.
const p3=join(project,'linked.kicad_pro');writeFileSync(p3,'{}');
const b3=join(home,`exports/linked-${hash(p3,2166136261)}-${hash(p3,131)}`);symlinkSync(project,b3,'dir');
writeFileSync(join(project,'.backplane.json'),JSON.stringify({project:'linked.kicad_pro'}));r=result(await rpc('kicad.export10',{thread}));check('symlinked base refused',r.export10==='error'&&readFileSync(pro,'utf8')==='{}',r);
writeFileSync(join(project,'.backplane.json'),JSON.stringify({project:'board.kicad_pro'}));
const token=readFileSync(join(home,'token'),'utf8').trim();
async function mcp(force:boolean){const res=await fetch(`http://127.0.0.1:${port}/mcp/${thread}?token=${token}`,{method:'POST',headers:{'content-type':'application/json',accept:'application/json, text/event-stream'},body:JSON.stringify({jsonrpc:'2.0',id:1,method:'tools/call',params:{name:'kicad_export10',arguments:{path:pro,force}}})});const text=await res.text();try{return JSON.parse(text)}catch{throw Error('MCP '+res.status+' '+text)}}
const openCount=readFileSync(opened,'utf8').split('\n').length;const mr=await mcp(true);check('MCP exports without auto-open',!mr.result?.isError&&JSON.parse(mr.result.content[0].text).export10==='done',mr);
check('MCP did not launch stock',readFileSync(opened,'utf8').split('\n').length===openCount);
check('original project intact',readFileSync(pro,'utf8')==='{}'&&readFileSync(join(project,'board.kicad_pcb'),'utf8')==='(kicad_pcb)');
ws.close();
// stop only the hubs this test spawned (Bun.spawn children), as the other e2e tests do
for(const h of hubs){if(h.exitCode===null)h.kill('SIGTERM');await h.exited}
console.log('test root: '+root);process.exit(failed?1:0);
