// bun e2e_git.mjs RUST_HUB_BIN JS_HUB_BIN OUT.json : git.status, git.diff, git.revert, history.refs on both hubs (ports 3856/3857)
import {mkdirSync,rmSync,writeFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {makeStandins,startHub,stopHub,client} from '../perf/lib.mjs';
const [rustBin,jsBin,out]=process.argv.slice(2);
const R='/home/h/Documents/i2cjak/backplane-gpui/build/gpui/files-run/git';rmSync(R,{recursive:true,force:true});mkdirSync(R,{recursive:true});makeStandins(R+'/bin');
const g=(cwd,...a)=>execFileSync('git',a,{cwd,env:{...process.env,GIT_AUTHOR_NAME:'t',GIT_AUTHOR_EMAIL:'t@t',GIT_COMMITTER_NAME:'t',GIT_COMMITTER_EMAIL:'t@t',GIT_CONFIG_GLOBAL:'/dev/null',GIT_CONFIG_SYSTEM:'/dev/null'}}).toString();
const repo=R+'/repo',plain=R+'/plain';mkdirSync(repo);mkdirSync(plain);g(repo,'init','-q');writeFileSync(repo+'/a.txt','one\n');writeFileSync(repo+'/b.kicad_pcb','(kicad_pcb)\n');g(repo,'add','.');g(repo,'commit','-qm','init');g(repo,'tag','v1');
writeFileSync(repo+'/a.txt','two\n');writeFileSync(repo+'/new.txt','n\n');
const log=[{$:'ProjectCreated',id:'p1',title:'r',root:repo,at:1},{$:'ProjectCreated',id:'p2',title:'n',root:plain,at:1},{$:'ThreadCreated',id:'t1',project:'p1',title:'T',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:2},{$:'ThreadCreated',id:'t2',project:'p2',title:'N',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:3}].map(x=>JSON.stringify(x)).join('\n')+'\n';
const res={};
for(const [name,bin,port] of [['rust',rustBin,3856],['js',jsBin,3857]]){
 const home=`${R}/${name}-home`;mkdirSync(home,{recursive:true});writeFileSync(home+'/events.jsonl',log);
 const h=await startHub({bin,home,port,standins:R+'/bin'});const c=await client(h,{view:true});const r={};
 const ask=async(k,m,p)=>{const x=await c.rpc(m,p,10000);r[k]=x?.o?{ok:x.o.ok,result:x.o.result,error:x.o.error}:'no reply'};
 await ask('status','git.status',{thread:'t1'});await ask('statusPlain','git.status',{thread:'t2'});await ask('statusNope','git.status',{thread:'nope'});
 await ask('diffNow','git.diff',{thread:'t1',from:0,to:-1});await ask('diffNoGit','git.diff',{thread:'t2',from:0,to:-1});await ask('diff01','git.diff',{thread:'t1',from:0,to:1});
 await ask('refs','history.refs',{thread:'t1'});await ask('refsPlain','history.refs',{thread:'t2'});await ask('revertMissing','git.revert',{thread:'t1',turn:5});
 c.ws.close();stopHub(h);res[name]=r}
const diffs=[];const norm=x=>JSON.stringify(x,(k,v)=>k==='refs'&&typeof v==='string'?v.split('\n').map(l=>l.split('\t').slice(0,2).join('\t')).join('\n'):v);
for(const k of Object.keys(res.rust))if(norm(res.rust[k])!==norm(res.js[k]))diffs.push({k,rust:res.rust[k],js:res.js[k]});
writeFileSync(out,JSON.stringify({res,diffs},null,1));console.log('cases',Object.keys(res.rust).length,'differences',diffs.length);for(const d of diffs)console.log(JSON.stringify(d).slice(0,700));process.exit(0);
