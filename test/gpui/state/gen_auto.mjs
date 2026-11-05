// Synthetic logs for the autosettle decision: bun gen_auto.mjs OUTDIR -> OUTDIR/auto-*.jsonl (+ auto.now: the times to probe at).
import {mkdirSync,writeFileSync} from 'node:fs';
const out=process.argv[2]||'.';mkdirSync(out,{recursive:true});
const T0=1794000000;let k=0;const E=(o,at)=>JSON.stringify({...o,at:at??T0+(k++)});
const P=E({$:'ProjectCreated',id:'p1',title:'proj',root:'/r'});
const mk=(id,extra={})=>E({$:'ThreadCreated',id,project:'p1',title:id,env:'local',branch:'',worktree:'',provider:'claude',model:'',...extra},T0);
const msg=(th,id,role,text,at)=>E({$:'MessagePosted',thread:th,msg:id,role,text},at);
const turn=(th,state,at)=>E({$:'TurnChanged',thread:th,state},at);
const set=(key,value,at=T0)=>E({$:'SettingSet',key,value},at);
const reads=[];const done=(th,at)=>(reads.push(th),[mk(th),msg(th,'m'+th+'-1','user','do it',at),msg(th,'a'+th,'assistant','done',at+5),turn(th,'completed',at+6)]);
const ls=[P];
// personal threads: old / recent, read / unread, audience, a message from person@ envelope
ls.push(...done('old',T0+10),...done('fresh',T0+20000));
ls.push(...done('read',T0+10),set('autosettle.read.read','aread'),set('autosettle.output.read','aread'));
ls.push(...done('aud',T0+10),set('autosettle.audience.aud','worker'));
ls.push(mk('env'),msg('env','menv','user','[message from person@box] hi',T0+10),msg('env','aenv','assistant','ok',T0+15),turn('env','completed',T0+16));
ls.push(mk('bot1'),msg('bot1','mb','user','[message from botty] hi',T0+10),msg('bot1','ab','assistant','ok',T0+15),turn('bot1','completed',T0+16));
ls.push(mk('noinput'),msg('noinput','an','assistant','hello',T0+10),turn('noinput','completed',T0+11));
// workers: launched, parent setting, run*, mailcoord-*, with and without reports
ls.push(...done('w-launched',T0+10),E({$:'ThreadLaunched',id:'w-launched',by:'boss',report:true}));
ls.push(...done('w-noreport',T0+10),E({$:'ThreadLaunched',id:'w-noreport',by:'boss',report:false}));
ls.push(...done('run7',T0+10),...done('mailcoord-x-1',T0+10),...done('w-parent',T0+10),set('thread.parent.w-parent','boss'));
ls.push(mk('boss'),msg('boss','mboss','user','go',T0+5),turn('boss','completed',T0+6));
ls.push(...done('w-rep2',T0+10),E({$:'ThreadLaunched',id:'w-rep2',by:'boss2',report:true}),mk('boss2'),msg('boss2','t:w-rep2:9','user','report',T0+9000),turn('boss2','completed',T0+9001));
ls.push(...done('w-del',T0+10),E({$:'ThreadLaunched',id:'w-del',by:'boss3',report:true}),set('autosettle.report.w-del',String(T0+9999)),mk('boss3'));
// tasks, locks, asks, queues, rows, secrets, running, pending
ls.push(...done('tp',T0+10),mk('tc'),E({$:'TaskDelegated',task:'k1',parent:'tp',child:'tc',depth:1,provider:'claude',model:''}),turn('tc','running',T0+30));
ls.push(...done('tp2',T0+10),mk('tc2'),E({$:'TaskDelegated',task:'k2',parent:'tp2',child:'tc2',depth:1,provider:'claude',model:''}),E({$:'TaskStateSet',task:'k2',state:'done',summary:'x'}),E({$:'TaskDelivered',task:'k2'}));
ls.push(...done('tp3',T0+10),mk('tc3'),E({$:'TaskDelegated',task:'k3',parent:'tp3',child:'tc3',depth:1,provider:'claude',model:''}),E({$:'TaskStateSet',task:'k3',state:'done',summary:'x'}));
ls.push(...done('lk',T0+10),E({$:'LockChanged',name:'merge',lock:{holder:{id:'a',kind:'thread',thread:'lk',note:'',since:1,lease:2},waiters:[],ready:false}}));
ls.push(...done('lkw',T0+10),E({$:'LockChanged',name:'build',lock:{holder:null,waiters:[{id:'b',thread:'lkw'}],ready:false}}));
ls.push(...done('ask',T0+10),E({$:'AskOpened',id:'q1',thread:'ask',kind:'k',tool:'t',detail:'',options:''}));
ls.push(...done('askc',T0+10),E({$:'AskOpened',id:'q2',thread:'askc',kind:'k',tool:'t',detail:'',options:''}),E({$:'AskClosed',id:'q2',answer:'a'}));
ls.push(...done('qd',T0+10),E({$:'TurnQueued',thread:'qd',msg:'mq',text:'later'}));
ls.push(...done('bg',T0+10),set('bg.bg','job1\tsleep\tx'),...done('watch',T0+10),set('watch.watch','wk1\tpr'),...done('ord',T0+10),set('order.wait.ord','1'));
ls.push(...done('sec',T0+10),set('secret.ask.k',JSON.stringify({thread:'sec'})));
ls.push(...done('pend',T0+10),E({$:'PendingChanged',thread:'pend',count:2}));
// shelves and overrides
ls.push(...done('pinned',T0+10),E({$:'ThreadPinned',id:'pinned',pinned:true}),...done('arch',T0+10),E({$:'ThreadArchived',id:'arch',archived:true}));
ls.push(...done('setd',T0+10),E({$:'ThreadSettled',id:'setd',manual:true}),...done('snz',T0+10),E({$:'ThreadSnoozed',id:'snz',until:T0+40000}));
ls.push(...done('forced',T0+10),E({$:'ThreadUnsettled',id:'forced'}));
// wakes: thread wait, work wait, file, pr, unknown kind, retired owner, expired work
ls.push(...done('wk-owner',T0+10),...done('wk-target',T0+10),turn('wk-target','running',T0+11));
ls.push(E({$:'WakeSet',id:'w1',by:'wk-owner',kind:'thread',target:'wk-target',events:['done'],once:true,note:''}));
ls.push(E({$:'WakeSet',id:'w2',by:'setd',kind:'work',target:'nowork',events:['x'],once:true,note:''},T0));
ls.push(E({$:'WakeSet',id:'w3',by:'setd',kind:'file',target:'/f',events:[],once:true,note:''}),E({$:'WakeSet',id:'w4',by:'setd',kind:'weird',target:'z',events:[],once:true,note:''}));
ls.push(E({$:'WakeSet',id:'w5',by:'pinned',kind:'pr',target:'7',events:[],once:true,note:''}),E({$:'WakeSet',id:'w6',by:'bg',kind:'work',target:'job1',events:[],once:true,note:''}));
// liveness records and restart-retry counts the progress writes read
ls.push(set('liveness.tc',JSON.stringify({last:T0+10,nudged:T0+500,stalled:true,reported:true,reason:'stuck'})),set('liveness.tp',JSON.stringify({last:T0+60000,nudged:0,stalled:false,reported:false,reason:'x'})),set('restart.retry.tc','2'),set('restart.retry.n.w-launched','3'),set('provider.resume.w-parent','1'),set('thread.config.hold.w-rep2','on'));
// legacy and defaults
ls.push(set('autosettle.version','1'));
writeFileSync(`${out}/auto-base.jsonl`,ls.join('\n')+'\n');
// the same with other windows: lifecycle on, a one-minute idle window, personal window zero
const tune=(name,kv)=>writeFileSync(`${out}/${name}.jsonl`,[...ls,...Object.entries(kv).map(([a,b])=>set(a,b,T0+100))].join('\n')+'\n');
tune('auto-read',Object.fromEntries(reads.map(th=>['autosettle.read.'+th,'a'+th])));
tune('auto-read-lifecycle',{...Object.fromEntries(reads.map(th=>['autosettle.read.'+th,'a'+th])),'settle.done':'on'});
tune('auto-lifecycle',{'settle.done':'on'});tune('auto-short',{'settle.idle.minutes':'1','settle.personal.minutes':'2'});tune('auto-zero',{'settle.personal.minutes':'0'});
writeFileSync(`${out}/auto.now`,[T0+700,T0+3000,T0+20000,T0+30000,T0+60000,T0+200000].join('\n')+'\n');
console.log('ok');
