// verifyLog(home, {acked}) : stream events.jsonl once and check the durability contract.
//  - every line is JSON, except torn fragments (a crash's half line, closed off by the next write) and a final unterminated tail
//  - no duplicate ids: MessagePosted (thread,msg), ActivityLogged (thread,act), ProjectCreated id and root, ThreadCreated id
//  - every acked write (ProjectCreated root = acked path) is present exactly once
//  - the last TurnChanged state of every thread
import {createReadStream} from 'node:fs';
import {StringDecoder} from 'node:string_decoder';
export async function verifyLog(home,{acked=new Set(),ackedThreads=new Set(),expectFragments=null}={}){
 const seen={msg:new Set(),act:new Set(),pid:new Set(),tid:new Set()},roots=new Map(),dups=[],idDups=[],bad=[],turn=new Map();
 let lines=0,valid=0,tail='',tailBad=false;
 const handle=(line,idx)=>{lines++;
  let e;try{e=JSON.parse(line)}catch{bad.push({line:idx,head:line.slice(0,60),len:line.length});return}
  valid++;
  switch(e.$){
   case 'MessagePosted':{const k=e.thread+'\0'+e.msg;if(seen.msg.has(k))dups.push(['MessagePosted',e.thread,e.msg]);else seen.msg.add(k);break}
   case 'ActivityLogged':{const k=e.thread+'\0'+e.act;if(seen.act.has(k))dups.push(['ActivityLogged',e.thread,e.act]);else seen.act.add(k);break}
   case 'ProjectCreated':{if(seen.pid.has(e.id))idDups.push(['ProjectCreated',e.id]);else seen.pid.add(e.id);roots.set(e.root,(roots.get(e.root)??0)+1);break}
   case 'ThreadCreated':{if(seen.tid.has(e.id))idDups.push(['ThreadCreated',e.id]);else seen.tid.add(e.id);break}
   case 'TurnChanged':turn.set(e.thread,e.state);break}};
 await new Promise((res,rej)=>{const s=createReadStream(`${home}/events.jsonl`,{highWaterMark:4<<20});const dec=new StringDecoder('utf8');let carry='',idx=0;
  s.on('data',b=>{const t=carry+dec.write(b);const parts=t.split('\n');carry=parts.pop();for(const l of parts){if(l.length)handle(l,idx);idx++}});
  s.on('end',()=>{tail=carry;res()});s.on('error',rej)});
 if(tail.length){tailBad=true}
 const missingThreads=[...ackedThreads].filter(t=>!seen.tid.has(t));
 const missing=[],multi=[];for(const p of acked){const n=roots.get(p)??0;if(n===0)missing.push(p);else if(n>1)multi.push([p,n])}
 const unexpectedBad=bad.length-(expectFragments??0);
 return {missingThreads,idDups,lines,valid,bad,tail:tail.length?{len:tail.length,head:tail.slice(0,60)}:null,dups,missing,multi,turn,ackedChecked:acked.size,
  ok:dups.length===0&&idDups.length===0&&missingThreads.length===0&&missing.length===0&&multi.length===0&&(expectFragments===null?true:unexpectedBad<=0)}}
