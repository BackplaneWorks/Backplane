// bun e2e_review.mjs RUST_HUB_BIN JS_HUB_BIN KICAD_RUNTIME OUT.json [js|rust]: review.*, kicad.export10 on both hubs, one after the other on the
// same paths (ports 3880/3881), so path hashes and commit ids match. Stand-in agents; real kicad-cli from the runtime, read-only, with temp config.
// Safety: GIT_CEILING_DIRECTORIES keeps git out of this repository; push and gh reach only a local bare repo and a stand-in; DISPLAY is empty and the
// file opener a stand-in, so nothing opens a window.
import {mkdirSync,rmSync,writeFileSync,readFileSync,existsSync,readdirSync,cpSync,chmodSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {makeStandins,startHub,stopHub,client} from '../perf/lib.mjs';
const [rustBin,jsBin,runtime,out]=process.argv.slice(2);
const R='/home/h/Documents/i2cjak/backplane-gpui/build/gpui/files-run/review',PLAIN=(process.env.TMPDIR||'/tmp')+'/bp-files-review-plain';
const when='2026-01-02T03:04:05Z',G={GIT_AUTHOR_NAME:'t',GIT_AUTHOR_EMAIL:'t@t',GIT_COMMITTER_NAME:'t',GIT_COMMITTER_EMAIL:'t@t',GIT_AUTHOR_DATE:when,GIT_COMMITTER_DATE:when,GIT_CONFIG_GLOBAL:'/dev/null',GIT_CONFIG_SYSTEM:'/dev/null',GIT_CEILING_DIRECTORIES:R};
const g=(cwd,...a)=>execFileSync('git',a,{cwd,env:{...process.env,...G}}).toString().trim();
// a board without its (model ...) forms, by balanced parentheses outside strings
function noModels(t){let o='',i=0;for(;;){const m=t.indexOf('(model ',i);if(m<0)return o+t.slice(i);o+=t.slice(i,m);let d=0,q=false,j=m;for(;j<t.length;j++){const c=t[j];if(q){if(c==='\\')j++;else if(c==='"')q=false}else if(c==='"')q=true;else if(c==='(')d++;else if(c===')'&&--d===0){j++;break}}i=j}}
const sub=(d,f,a,b)=>{const p=`${d}/${f}`;const t=readFileSync(p,'utf8');if(!t.includes(a))throw Error(`no ${a} in ${f}`);writeFileSync(p,t.replace(a,b))};
function fresh(){
 for(const d of [`${R}/w`,PLAIN])rmSync(d,{recursive:true,force:true});
 mkdirSync(`${R}/w/fake`,{recursive:true});mkdirSync(PLAIN);makeStandins(`${R}/bin`);
 const fake=`${R}/w/fake`;
 writeFileSync(`${fake}/gh`,'#!/bin/sh\nprintf \'%s\\n\' "$@" >> "$FAKE_GH_LOG"\necho https://example.invalid/releases/x\n');chmodSync(`${fake}/gh`,0o755);
 // stock KiCad stand-ins first on PATH: the open script stops at them and never reaches /usr/bin/kicad, so no window can open
 writeFileSync(`${fake}/open`,'#!/bin/sh\nexit 0\n');chmodSync(`${fake}/open`,0o755);writeFileSync(`${fake}/kicad`,'#!/bin/sh\nexit 0\n');chmodSync(`${fake}/kicad`,0o755);writeFileSync(`${fake}/kicad-cli`,'#!/bin/sh\necho 10.0.7\n');chmodSync(`${fake}/kicad-cli`,0o755);
 const kx=`${R}/w/kxruntime`;mkdirSync(kx+'/bin',{recursive:true});cpSync(`${runtime}/manifest.json`,kx+'/manifest.json');
 writeFileSync(kx+'/bin/kicad-cli',`#!/bin/sh\nif [ "$1 $2" = "project downgrade" ]; then\n  printf '%s\\n' "$*" >> "${kx}/args"\n  rep=; out=; prev=; for a in "$@"; do [ "$prev" = --report-json ] && rep=$a; [ "$prev" = -o ] && out=$a; prev=$a; done\n  cp "${kx}/report.json" "$rep"\n  case " $* " in *' --dry-run '*) ;; *) mkdir -p "$out"; echo copy > "$out/x.kicad_pro";; esac\n  exit 0\nfi\nexec "${runtime}/bin/kicad-cli" "$@"\n`);chmodSync(kx+'/bin/kicad-cli',0o755);
 const demos=`${runtime}/share/kicad/demos`;
 const proj=(name,src,edit)=>{const d=`${R}/w/${name}`;cpSync(`${demos}/${src}`,d,{recursive:true});writeFileSync(`${d}/.gitignore`,'.backplane/releases\n*-backups\nfp-info-cache\n*.kicad_prl\n');for(const f of readdirSync(d))if(f.endsWith('.kicad_pcb'))writeFileSync(`${d}/${f}`,noModels(readFileSync(`${d}/${f}`,'utf8')));g(d,'init','-q','-b','main');g(d,'add','.');g(d,'commit','-qm','one');g(d,'tag','base');edit(d);g(d,'add','.');g(d,'commit','-qm','two','--allow-empty');return d};
  const ecc=proj('ecc83','ecc83',d=>sub(d,'ecc83-pp.kicad_sch','"Value" "1.5K"','"Value" "2.2K"'));
 const hier=proj('hier','complex_hierarchy',d=>{});
 const bare=`${R}/w/bare.git`;mkdirSync(bare);g(bare,'init','-q','--bare');g(ecc,'remote','add','origin',bare);
 writeFileSync(`${PLAIN}/note.txt`,'x');
 return {ecc,hier,bare};
}
function log({ecc,hier}){return [{$:'ProjectCreated',id:'p1',title:'e',root:ecc,at:1},{$:'ProjectCreated',id:'p2',title:'h',root:hier,at:1},{$:'ProjectCreated',id:'p3',title:'n',root:PLAIN,at:1},
 ...['t1:p1','t2:p2','t3:p3'].map((s,i)=>{const [id,project]=s.split(':');return {$:'ThreadCreated',id,project,title:'T',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:2+i}})].map(x=>JSON.stringify(x)).join('\n')+'\n'}
// values for x and y as text: F32.read then F32.show, stored in the log's summary
function corpus(){const v=['0','-0','0.1','0.2','0.3','1','1.5','10.5','20.25','100','123.456','16777216','16777217','1e21','1e22','1e-7','1.5e-7','0.000001','0.0000001','3.4028235e38','3.5e38','1e39','1e-45','1e-46','nan','NaN','inf','-inf','Infinity','abc','',' 1','+.5','5.','.5','1e3','1E-2','0x10','1_0','--1','1 ','12.345678901234','0.30000001192092896','99999.99','-127.00001'];
 let s=12345;const r=()=>(s=(Math.imul(s,1103515245)+12345)>>>0)/4294967296;
 for(let i=0;i<600;i++)v.push(String(Math.fround((r()-0.5)*Math.pow(10,Math.floor(r()*12)-4))));for(let i=0;i<300;i++)v.push((r()*400).toFixed(Math.floor(r()*8)));return v}
const norm=s=>String(JSON.stringify(s)).replace(/[^"\\]*\/bp-(review|release)\.[A-Za-z0-9]+/g,'/tmp/bp-$1.X').replace(/\.pending\.[A-Za-z0-9]+/g,'.pending.X').replace(/\br\d{9,}-\d+/g,'rID').replace(/(checks:[0-9a-f]{64}:)\d+/g,'$1N').replace(/"at":\d+/g,'"at":0').replace(/export10:\d+:\d+:\d+/g,'export10:ID').replace(/\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ/g,'DATE');
async function run(name,bin,port){
 const w=fresh();const home=`${R}/home`;rmSync(home,{recursive:true,force:true});mkdirSync(home,{recursive:true});writeFileSync(home+'/events.jsonl',log(w));
 const fh=`${R}/w/fakehome`;mkdirSync(fh,{recursive:true});const ghlog=`${R}/w/gh.log`;
 const baseEnv={...G,HOME:fh,XDG_CONFIG_HOME:fh+'/c',XDG_DATA_HOME:fh+'/d',XDG_CACHE_HOME:fh+'/k',XDG_STATE_HOME:fh+'/s',KICAD_CONFIG_HOME:fh+'/kc',BACKPLANE_KICAD_CLI:`${runtime}/bin/kicad-cli`,BACKPLANE_OPEN:`${R}/w/fake/open`,FAKE_GH_LOG:ghlog,PATH:`${R}/w/fake:${R}/bin:${process.env.PATH}`};
 const h=await startHub({bin,home,port,standins:`${R}/bin`,log:`${R}/${name}.log`,extraEnv:baseEnv});
 const env=readFileSync(`/proc/${h.pid}/environ`,'latin1');if(!env.includes(`BACKPLANE_AGENTS=${R}/bin`))throw Error('hub has no stand-in agents');
 const c=await client(h,{view:true});const r={},T=280000;
 const ask=async(k,m,p)=>{const x=await c.rpc(m,p,T);r[k]=x?.o?{ok:x.o.ok,result:x.o.result,error:x.o.error}:'no reply'};
 const head=g(w.ecc,'rev-parse','HEAD'),base=g(w.ecc,'rev-parse','base');const hd=name=>`git\t${w.ecc}/.git\t${name}\t${w.ecc.length}\t${w.ecc}/ecc83-pp.kicad_sch`;
 // comments
 const xs=corpus();for(let i=0;i<xs.length;i++){const x=await c.rpc('review.comment',{thread:'t1',op:'add',file:'f.kicad_pcb',commit:'',kind:'board',layer:'F.Cu',x:xs[i],y:xs[xs.length-1-i],item:'k',text:'t',parent:''},T);if(!x?.o?.ok)r['add'+i]=x?.o}
 for(const [k,x] of [['num',1.5],['numInt',30],['numBig',123456789012],['numNeg',-2.25]]) await ask('c.'+k,'review.comment',{thread:'t1',op:'add',file:'f',x,y:0,text:'t'});
 for(const [k,t] of Object.entries({quote:'a"b\\c',ctl:'a\u0001\n\r\tb',uni:'é😀ü',space:'  padded  ',long:'x'.repeat(5000)})) await ask('c.text.'+k,'review.comment',{thread:'t1',op:'add',file:t,commit:t,kind:t,layer:t,item:t,text:t,parent:''});
 for(const [k,p] of Object.entries({resolve:{op:'resolve',id:'c1'},reopen:{op:'reopen',id:'c1'},edit:{op:'edit',id:'c1',text:'new'},drop:{op:'drop',id:'c1'},noText:{op:'add',text:'  '},noId:{op:'resolve'},editBlank:{op:'edit',id:'c1',text:''},badOp:{op:'burn',id:'c1'},noThread:{thread:'',op:'drop',id:'c1'}})) await ask('c.op.'+k,'review.comment',{thread:'t1',...p});
 // BOMs
 await ask('bom.live','review.bom',{thread:'t1',a:'',b:''});await ask('bom.hier','review.bom',{thread:'t2',a:'',b:''});await ask('bom.none','review.bom',{thread:'t3',a:'',b:''});
 await ask('bom.head','review.bom',{thread:'t1',a:'',b:hd(head)});await ask('bom.diff','review.bom',{thread:'t1',a:hd(base),b:hd(head)});await ask('bom.same','review.bom',{thread:'t1',a:hd(head),b:hd(head)});await ask('bom.liveDiff','review.bom',{thread:'t1',a:hd(base),b:''});
 await ask('bom.otherRoot','review.bom',{thread:'t2',a:'',b:`${w.hier}/complex_hierarchy.kicad_sch`});await ask('bom.escape','review.bom',{thread:'t1',a:'',b:`${w.ecc}/../hier/complex_hierarchy.kicad_sch`});
 // checks
 await ask('chk.live','review.checks',{thread:'t1',commit:'',trigger:'manual'});await ask('chk.commit','review.checks',{thread:'t1',commit:head,trigger:'manual'});await ask('chk.hier','review.checks',{thread:'t2',commit:'',trigger:'manual'});
 await ask('chk.plain','review.checks',{thread:'t3',commit:'',trigger:'manual'});await ask('chk.badCommit','review.checks',{thread:'t1',commit:'nosuch',trigger:'manual'});await ask('chk.dash','review.checks',{thread:'t1',commit:'-x',trigger:'manual'});
 // ticks
 await ask('tick.0','review.tick',{thread:'t1',index:0});await ask('tick.1','review.tick',{thread:'t1',index:1});await ask('tick.0b','review.tick',{thread:'t1',index:0});await ask('tick.str','review.tick',{thread:'t1',index:'2'});await ask('tick.frac','review.tick',{thread:'t1',index:1.5});await ask('tick.word','review.tick',{thread:'t1',index:'x'});await ask('tick.far','review.tick',{thread:'t1',index:999});await ask('tick.huge','review.tick',{thread:'t1',index:'281474976710656'});await ask('tick.none','review.tick',{thread:'t1'});
 r.checklist=readFileSync(`${w.ecc}/.backplane/checklist.md`,'utf8');
 rmSync(`${w.ecc}/.backplane`,{recursive:true,force:true});
 // own checks: pending, the question once, then trusted
 mkdirSync(`${w.hier}/.backplane`);writeFileSync(`${w.hier}/.backplane/checks.json`,JSON.stringify({checks:[{name:'lint',run:'echo ran; touch ran.txt'},{name:'fail',run:'echo bad | cat; exit 3',on:['manual']},{run:'echo pr',on:['pr']}],gate:{checklist:true}}));
 await ask('own.pending','review.checks',{thread:'t2',commit:'',trigger:'manual'});await ask('own.again','review.checks',{thread:'t2',commit:'',trigger:'manual'});r.ownRan=existsSync(`${w.hier}/ran.txt`);
 const sha=x=>/"pending":"([0-9a-f]{64})"/.exec(JSON.stringify(x))?.[1];writeFileSync(home+'/checks.trusted',sha(r['own.pending'])+'\n');
 await ask('own.trusted','review.checks',{thread:'t2',commit:'',trigger:'manual'});await ask('own.pr','review.checks',{thread:'t2',commit:'',trigger:'pr'});r.ownRan2=existsSync(`${w.hier}/ran.txt`);rmSync(`${w.hier}/ran.txt`,{force:true});
 // releases: every refusal, then a clean one, then one published to the local bare repo through the stand-in gh
 await ask('rel.badTag','review.release',{thread:'t1',tag:'a b'});await ask('rel.plain','review.release',{thread:'t3',tag:'v1'});
 writeFileSync(`${w.ecc}/dirty.txt`,'x');await ask('rel.dirty','review.release',{thread:'t1',tag:'v1'});rmSync(`${w.ecc}/dirty.txt`);
 rmSync(`${w.hier}/.backplane`,{recursive:true,force:true});await ask('rel.gateFail','review.release',{thread:'t2',tag:'v1'});r.hierTags=g(w.hier,'tag').split('\n');
 const lst=d=>{const o=[];const go=(p,x)=>{for(const e of readdirSync(p,{withFileTypes:true}).sort((a,b)=>a.name<b.name?-1:1)){const q=x?`${x}/${e.name}`:e.name;e.isDirectory()?go(`${p}/${e.name}`,q):o.push(q)}};if(existsSync(d))go(d,'');return o};
 const rel=t=>`${w.ecc}/.backplane/releases/${t}`,meta=(t,k)=>{r[k+'Files']=lst(rel(t));r[k+'Notes']=existsSync(rel(t)+'/notes.md')?readFileSync(rel(t)+'/notes.md','utf8'):'';r[k+'Manifest']=existsSync(rel(t)+'/manifest.json')?JSON.parse(readFileSync(rel(t)+'/manifest.json','utf8')):null};
 await ask('rel.clean','review.release',{thread:'t1',tag:'v1'});meta('v1','v1');
 await ask('rel.again','review.release',{thread:'t1',tag:'v1'});
 writeFileSync(`${w.ecc}/ecc83-pp.kicad_pro`,readFileSync(`${w.ecc}/ecc83-pp.kicad_pro`,'utf8')+'\n');sub(w.ecc,'ecc83-pp.kicad_sch','"Value" "1.5K"','"Value" "3.3K"');g(w.ecc,'add','.');g(w.ecc,'commit','-qm','three');
 await ask('rel.movedTag','review.release',{thread:'t1',tag:'v1'});await ask('rel.publish','review.release',{thread:'t1',tag:'v2',publish:true});meta('v2','v2');
 r.remoteTags=g(w.ecc,'ls-remote','--tags','origin').split('\n').map(l=>l.split('\t')[1]);r.ghArgs=existsSync(ghlog)?readFileSync(ghlog,'utf8').replaceAll(w.ecc,'ROOT'):'';
 // KiCad 10 copies
 await ask('kx.first','kicad.export10',{thread:'t1'});for(let i=0;i<30&&r['kx.first']?.result?.export10==='unavailable';i++){await new Promise(z=>setTimeout(z,1000));await ask('kx.first','kicad.export10',{thread:'t1'})}
 const rep=r['kx.first']?.result?.report;await ask('kx.stale','kicad.export10',{thread:'t1',anyway:true,report:'{"entries":[]}'});await ask('kx.yes','kicad.export10',{thread:'t1',anyway:true,report:JSON.stringify(rep)});await ask('kx.plain','kicad.export10',{thread:'t3'});
 // deliberate differences, listed as such
 await ask('x.commentNoThread','review.comment',{thread:'nope',op:'drop',id:'c1'});await ask('x.bomNoThread','review.bom',{thread:'nope',a:'',b:''});await ask('x.bomForeignGit','review.bom',{thread:'t1',a:`git\t/tmp/evil.git\t${head}\t1\t/tmp/x`,b:''});const pwn=`/tmp/bp-pwn-${name}:ecc83-pp.kicad_sch`;rmSync(pwn,{force:true});await ask('x.bomOption','review.bom',{thread:'t1',a:hd(`--output=/tmp/bp-pwn-${name}`),b:''});r.xPwned=existsSync(pwn);rmSync(pwn,{force:true});
 c.ws.close();stopHub(h);await new Promise(z=>setTimeout(z,1500));
 const kx=`${R}/w/kxruntime`;const lossy='{"entries":[{"category":"lower","feature":"Text","detail":"Converted","count":2},{"category":"drop","feature":"Data","detail":"Removed","count":1}],"future":1}',block='{"entries":[{"category":"block","feature":"Ends","detail":"No","count":1},{"category":"newcat","feature":"F"}]}';
 writeFileSync(kx+'/report.json',lossy);
 const h2=await startHub({bin,home,port,standins:`${R}/bin`,log:`${R}/${name}.log`,extraEnv:{...baseEnv,BACKPLANE_KICAD_CLI:kx+'/bin/kicad-cli'}});
 const c2=await client(h2,{view:true});const ask2=async(k,p)=>{const x=await c2.rpc('kicad.export10',p,T);r[k]=x?.o?{ok:x.o.ok,result:x.o.result,error:x.o.error}:'no reply'};
 await ask2('kc.first',{thread:'t1'});for(let i=0;i<30&&r['kc.first']?.result?.export10==='unavailable';i++){await new Promise(z=>setTimeout(z,1000));await ask2('kc.first',{thread:'t1'})}
 const seen=JSON.stringify(r['kc.first']?.result?.report);await ask2('kc.noAnyway',{thread:'t1',report:seen});await ask2('kc.stale',{thread:'t1',anyway:true,report:'{"entries":[]}'});await ask2('kc.future',{thread:'t1',anyway:true,report:seen.replace('"future":1','"future":2')});await ask2('kc.yes',{thread:'t1',anyway:true,report:seen});
 writeFileSync(kx+'/report.json',block);await ask2('kc.block',{thread:'t1'});await ask2('kc.blockYes',{thread:'t1',anyway:true,report:JSON.stringify(r['kc.block']?.result?.report)});
 r.kcArgs=existsSync(kx+'/args')?readFileSync(kx+'/args','utf8').replaceAll(R,'R').replace(/ecc83-pp-\d+-\d+/g,'BASE'):'';
 c2.ws.close();stopHub(h2);await new Promise(z=>setTimeout(z,800));
 const ev=readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>JSON.parse(l));
 r.reviewActs=ev.filter(e=>e.$==='ActivityLogged'&&e.tone==='review').map(e=>[e.kind,e.summary]);r.asks=ev.filter(e=>e.$==='AskOpened');r.kxActs=ev.filter(e=>e.kind==='kicad.export10');
 return r}
const res={};for(const [n,b,p] of [['js',jsBin,3881],['rust',rustBin,3880]])if(!process.argv[6]||process.argv[6]===n)res[n]=await run(n,b,p);
writeFileSync(out+'.raw',JSON.stringify(res));
const diffs=[];let same=0;if(res.js&&res.rust)for(const k of new Set([...Object.keys(res.js),...Object.keys(res.rust)])){if(k==='reviewActs'){const a=res.js[k],b=res.rust[k];let d=0;for(let i=0;i<Math.max(a.length,b.length);i++){if(norm(a[i])===norm(b[i]))same++;else{d++;if(d<=20)diffs.push({k:`act${i}`,rust:b[i],js:a[i]})}}continue}
 if(norm(res.js[k])===norm(res.rust[k]))same++;else diffs.push({k,rust:res.rust[k],js:res.js[k]})}
writeFileSync(out,JSON.stringify({res,diffs,same},null,1));console.log('same',same,'differences',diffs.length,'acts',res.js?.reviewActs?.length,res.rust?.reviewActs?.length);for(const d of diffs)console.log(JSON.stringify({k:d.k,rust:d.rust,js:d.js}).slice(0,600));const must=(n,x)=>[[`${n}: clean release`,x['rel.clean']?.result?.report?.err===''&&x.v1Files?.includes('notes.md')&&x.v1Files?.length>=10],[`${n}: gate stops`,String(x['rel.gateFail']?.result?.report?.err).includes('the checks fail')&&x.hierTags?.join('')==='base'],[`${n}: published`,x['rel.publish']?.result?.report?.err===''&&x.remoteTags?.includes('refs/tags/v2')&&x.ghArgs?.includes('--verify-tag')],[`${n}: BOM change in notes`,x.v2Notes?.includes('## BOM changes')],[`${n}: consent asked`,x['kc.first']?.result?.export10==='confirm'&&x['kc.stale']?.result?.export10==='confirm'&&x['kc.noAnyway']?.result?.export10==='confirm'],[`${n}: consent taken`,x['kc.yes']?.result?.export10==='done'],[`${n}: block needs drop`,x.kcArgs?.includes('--drop-approximations')],[`${n}: real checks`,String(x['chk.hier']?.result?.report?.line).includes('DRC')]];
const fails=Object.entries(res).flatMap(([n,x])=>must(n,x)).filter(([,ok])=>!ok).map(([k])=>k);console.log('hollow:',fails.length?fails.join('; '):'none');process.exit(fails.length?1:0);
