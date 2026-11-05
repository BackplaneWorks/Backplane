// Does the policy resolve a bot target and a room target from the events the hubs write? usage: bun policy_room.mjs BIN PORT   (synthetic; capped)
import {spawn} from 'node:child_process';import net from 'node:net';
const [bin,port]=process.argv.slice(2);
const p=spawn('sh',['-c',`ulimit -s 1048576; exec ${bin} ${port}`],{stdio:['ignore','pipe','inherit']});
await new Promise(r=>p.stdout.once('data',r));
const s=net.connect(Number(port),'127.0.0.1');await new Promise(r=>s.once('connect',r));
let buf='',w=null;s.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);w?.(l)}});
const ask=l=>new Promise(r=>{w=r;s.write(l+'\n')});
let at=1700000000;const E=o=>({...o,at:++at});
const BT='t1793875706-147',B='b1793875706-146',B2='b1793875706-149',BT2='t1793875706-148';
const log=[E({$:'ProjectCreated',id:'p1700000000-1',title:'soak',root:'/tmp/soak'}),
 E({$:'ThreadCreated',id:BT,project:'',title:'room-bot-a',env:'local',branch:'',worktree:'',provider:'claude',model:''}),E({$:'BotSet',id:B,name:'room-bot-a',thread:BT,look:3,persona:''}),
 E({$:'ThreadCreated',id:BT2,project:'',title:'room-bot-b',env:'local',branch:'',worktree:'',provider:'claude',model:''}),E({$:'BotSet',id:B2,name:'room-bot-b',thread:BT2,look:4,persona:''}),
 E({$:'RoomSet',id:'parity-room',name:'Parity room',members:`${B},${B2}`}),E({$:'RoomPosted',id:'r1793875706-150',room:'parity-room',from:'person',text:'hello room',hop:0})];
await ask('F '+JSON.stringify({t:'log',since:0,origin:'1',version:'x',info:{},items:log}));
const V=async(target)=>JSON.parse(await ask('V '+JSON.stringify({hub:'h',cid:'c',now:(at+100)*1000,session:{sel:'',rid:1,drafts:{},open:{},adding:false,panel:false,info:{}},target,action:'',value:''})));
for(const t of [{kind:'bot',id:B},{kind:'room',id:'parity-room'},{kind:'bot',id:'ghost'},{kind:'room',id:'ghost'},{kind:'recent',id:''}]){
  const r=await V(t);const j=r.overlay?.j||{};
  console.log(JSON.stringify(t),'-> remoteReady',r.remoteReady,'nextTarget',JSON.stringify(r.nextTarget),'| overlay.j.bot',j.bot?JSON.stringify(Object.keys(j.bot)).slice(0,80):j.bot,'| rooms',JSON.stringify((j.rooms||[]).length),'| sel',JSON.stringify(r.session?.sel),'| drafts @bot/@room/@bot.want',JSON.stringify([r.session?.drafts?.['@bot'],r.session?.drafts?.['@room'],r.session?.drafts?.['@bot.want']]));
}
p.kill();process.exit(0);
