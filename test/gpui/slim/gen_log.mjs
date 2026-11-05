// A synthetic event log for the policy checks: P projects and T threads each (enough for the sidebar to need a second page), a few messages per thread.
// usage: bun gen_log.mjs [P] [T] > log.txt     (no real data)
const P=Number(process.argv[2]||12),T=Number(process.argv[3]||12);const t0=1700000000;let at=t0,n=0;const out=[];const e=o=>out.push(JSON.stringify({...o,at:++at}));
for(let p=0;p<P;p++){const pid=`p17000000${String(p).padStart(2,'0')}-${p+1}`;e({$:'ProjectCreated',id:pid,title:'project '+p,root:'/tmp/gen/'+p});
  for(let k=0;k<T;k++){const id=`t17000${String(p).padStart(2,'0')}${String(k).padStart(3,'0')}-${n+10}`;e({$:'ThreadCreated',id,project:pid,title:`thread ${p}.${k}`,env:'local',branch:'',worktree:'',provider:'claude',model:''});
    for(let m=0;m<3;m++){e({$:'MessagePosted',thread:id,msg:`m${t0+n}-${n+100}`,role:m%2?'assistant':'user',text:`message ${m} of ${id}`});n++}}}
console.log(out.join('\n'));
