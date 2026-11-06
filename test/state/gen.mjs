// Synthetic logs for the state families (no real data): bun gen.mjs OUTDIR  ->  OUTDIR/<family>.jsonl
// Each is folded by State.replay and by the Rust fold; run.sh compares them with --skip none.
import {mkdirSync,writeFileSync} from 'node:fs';
const out=process.argv[2]||'.';mkdirSync(out,{recursive:true});
let at=1793900000;const E=(o)=>JSON.stringify({...o,at:o.at??at++});
const base=[E({$:'ProjectCreated',id:'p1',title:'proj',root:'/r'}),E({$:'ThreadCreated',id:'t1',project:'p1',title:'T',env:'local',branch:'',worktree:'',provider:'claude',model:''}),E({$:'ThreadCreated',id:'t2',project:'p1',title:'U',env:'local',branch:'',worktree:'',provider:'codex',model:'m'})];
const logs={};
// uber: in order, out of order, gaps, losers, lease terms, fired slots, a dot in an origin, odd stamps
{const u=(origin,n,stamp,op)=>E({$:'UberOp',uid:'U1',origin,n,stamp,op:typeof op==='string'?op:JSON.stringify(op)});
 const st=(s,c,o)=>String(s).padStart(10,'0')+'.'+String(c).padStart(6,'0')+'.'+o;
 logs.uber=[...base,E({$:'UberSet',bot:'b1',uid:'U1'}),
  u('m1',1,st(10,0,'m1'),{k:'space',spec:'a'}),u('m1',2,st(12,0,'m1'),{k:'space',spec:'b'}),u('m2',1,st(11,0,'m2'),{k:'space',spec:'c'}),
  u('m2',3,st(13,0,'m2'),{k:'page',html:'x'}),u('m2',2,st(9,0,'m2'),{k:'page',html:'old'}),u('m2',3,st(13,0,'m2'),{k:'page',html:'x'}),
  u('m1',3,st(20,0,'m1'),{k:'lease',holder:'m1',term:2}),u('m2',4,st(30,0,'m2'),{k:'lease',holder:'m2',term:1}),u('m1',4,st(31,0,'m1'),{k:'lease',holder:'m1',term:'2'}),
  u('m1',5,st(32,0,'m1'),{k:'mem',key:'k',kind:'fact',text:'t',tags:''}),u('m1',6,st(33,0,'m1'),{k:'forget',key:'k'}),
  u('m1',7,st(34,0,'m1'),{k:'routine',id:'r1',name:'n',cron:'* * * * *',prompt:'p',on:true,pin:''}),u('m1',8,st(35,0,'m1'),{k:'routine-del',id:'r1'}),
  u('m1',9,st(36,0,'m1'),{k:'fired',routine:'r1',slot:5}),u('m2',5,st(37,0,'m2'),{k:'fired',routine:'r1',slot:4}),u('m2',6,st(38,0,'m2'),{k:'fired',routine:'r1',slot:9}),
  u('m1',10,st(39,0,'m1'),{k:'bind',machine:'mm',bot:'b',thread:'t',name:'n'}),u('m1',11,st(40,0,'m1'),'not json'),u('m1',12,st(41,0,'m1'),{k:'who',name:'x',look:3,persona:'p'}),
  u('a.b',1,st(50,0,'a.b'),{k:'space',spec:'dot'}),u('a.b',2,st(51,0,'a.b'),{k:'space',spec:'dot2'}),u('',1,st(52,0,''),{k:'space',spec:'noorigin'}),
  u('m3',1,'weird',{k:'who',name:'w',look:1,persona:''}),u('m3',2,'',{k:'who',name:'w2',look:1,persona:''}),
  E({$:'UberOp',uid:'U1',origin:'m4',n:1,stamp:st(60,0,'m4'),op:'{"k":"unknown"}'})];}
// wakes and locks
logs.wake=[...base,
 E({$:'WakeSet',id:'w1',by:'t1',kind:'pr',target:'5',events:['merged','closed'],once:true,note:'n'}),E({$:'WakeSet',id:'w2',by:'t1',kind:'thread',target:'t2',events:[],once:false,note:'quote " \\ \n\t\u0008 é'}),
 E({$:'WakeSet',id:'w3',by:'t2',kind:'file',target:'/a/b',events:['x',3,null],note:''}),E({$:'WakeSet',id:'w1',by:'t1',kind:'pr',target:'5',events:['merged'],once:true,note:'again'}),
 E({$:'WakeDropped',id:'w2'}),E({$:'WakeDropped',id:'ghost'}),E({$:'WakeDropped',id:'w1'}),E({$:'WakeSet',id:'w4',by:'',kind:'',target:''})];
logs.lock=[...base,
 E({$:'LockChanged',name:'merge',lock:{holder:{id:'a',kind:'thread',thread:'t1',note:'n',since:5,lease:9},waiters:[{id:'b',kind:'client',thread:'',note:'',since:6,lease:0},{id:'c'}],ready:false}}),
 E({$:'LockChanged',name:'build',lock:{holder:null,waiters:[],ready:true,outside:{pid:4242,cmd:'make "x"',since:7}}}),
 E({$:'LockChanged',name:'merge',lock:{holder:null,waiters:[],ready:false}}),E({$:'LockChanged',name:'odd',lock:{holder:'str',waiters:{},outside:3}}),E({$:'LockChanged',name:'none'})];
// tokens: epochs, months, models, known flags, unknown thread, repeated key
{const t=(o)=>E({$:'TurnUsage',thread:'t1',provider:'claude',model:'opus',key:'k',input:10,cread:20,cwrite:30,output:40,reasoning:5,cost:700,known:'icwotc',basis:'b',...o});
 logs.tok=[...base,t({}),t({key:'k2',at:1793900100}),t({thread:'t2',provider:'codex',model:''}),t({at:1796000000,known:'i'}),t({at:951782400+86400}),t({thread:'ghost',known:''}),
  E({$:'SettingSet',key:'tok.epoch',value:'e1'}),t({}),t({model:'a/b|c',provider:'grok'}),E({$:'SettingSet',key:'tok.recount',value:'r'}),t({at:1735689600}),t({at:1767225599})];}
// deploy
{const a=(by,cover)=>E({$:'DeployAsked',by,note:'',commit:'',cover});const b=(commit,state,detail='')=>E({$:'DeployBuilt',commit,state,version:'',detail});
 logs.deploy=[...base,a('-',''),a('t1',''),b('c1','building'),b('c1','pid','123 x'),a('t2',''),a('t3','building'),a('t4','staged'),a('t1',''),b('c1','staged'),
  b('c2','building'),b('c2','live'),b('c2','failed'),a('t5',''),b('c3','building'),b('c3','live'),b('c3','skipped'),b('c4','staged'),b('c4','live'),b('c9','weird'),a('x','live')];}
// todos: tool plans, person edits, agent lists, snapshots, turns
{const tc=(who,data,thread='t1')=>E({$:'TodosChanged',thread,who,data:typeof data==='string'?data:JSON.stringify(data)});
 const L=(...xs)=>xs.map(([id,text,status,form=''])=>({id,text,form,status}));
 logs.todos=[...base,
  tc('tools',[{name:'TaskCreate',id:'c1',input:{subject:'a',activeForm:'doing a'}},{name:'TaskCreate',id:'c2',input:{description:'b'}},{name:'TaskNamed',use:'c1',number:'1'},{name:'TaskNamed',use:'c2',number:'2'}]),
  tc('tools',[{name:'TaskUpdate',input:{taskId:'1',status:'in_progress'}},{name:'TaskUpdate',input:{taskId:'2',subject:'B2',status:'weird'}}]),
  tc('person',{op:'add',id:'p1',text:'mine'}),tc('person',{op:'move',id:'p1',to:0}),tc('person',{op:'set',id:'1',text:'mine1',status:'completed'}),
  tc('tools',[{name:'TodoWrite',input:{todos:[{content:'a',status:'completed',activeForm:'x'},{content:'b',status:'pending'},{content:'c',status:'bogus'}]}}]),
  tc('person',{op:'delete',id:'2'}),tc('person',{op:'nothing'}),tc('agent',L(['1','a','completed'],['2','b','pending'],['3','c','pending'])),tc('agent',L(['1','a','completed'],['3','c','in_progress'])),
  tc('tools',[{name:'TaskUpdate',input:{taskId:'3',status:'deleted'}},{name:'TaskCreate',id:'c9',input:{subject:'z'}}]),tc('tools','not json'),tc('person','not json'),tc('agent','not json'),
  tc('snapshot',{current:'[]',base:'[]',visible:'0'}),tc('tools',[{name:'TaskCreate',id:'c3',input:{subject:'fresh'}}]),
  E({$:'TurnChanged',thread:'t1',state:'running'}),tc('tools',[{name:'TaskCreate',id:'c4',input:{subject:'after turn'}}]),E({$:'TurnChanged',thread:'t1',state:'completed'}),
  tc('person',{op:'add',id:'q',text:'q'},'t2'),tc('agent',L(['q','Q','pending']),'t2'),tc('weird-who',L(['1','x','pending']),'t2')];}
// reminders and a mix of everything
logs.remind=[...base,E({$:'Reminded',id:'r1',bot:'b',thread:'t1',text:'x'}),E({$:'Reminded',id:'r1',bot:'b',thread:'t1',text:'x',at:5}),E({$:'Reminded',id:'r2',bot:'b',thread:'ghost',text:'y'}),
  E({$:'MessagePosted',thread:'t1',msg:'m1',role:'user',text:'hi'}),E({$:'Reminded',id:'r3',bot:'b',thread:'t1',text:'z'})];
logs.mix=[...logs.uber,...logs.wake.slice(3),...logs.lock.slice(3),...logs.tok.slice(3),...logs.deploy.slice(3),...logs.todos.slice(3),...logs.remind.slice(3)];
for(const [k,v] of Object.entries(logs))writeFileSync(`${out}/${k}.jsonl`,v.join('\n')+'\n');
console.log(Object.keys(logs).join(' '));
