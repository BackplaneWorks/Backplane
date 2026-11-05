// A deterministic synthetic fixture home (no real conversations): one project, two threads, one long thread.
import {mkdirSync,writeFileSync,rmSync} from 'node:fs';
export function makeFixture(home,root='/tmp/parity-fixture',{deleted=false,privateLines=false}={}){
  rmSync(home,{recursive:true,force:true});mkdirSync(home,{recursive:true});
  const t0=1700000000,L=[];const P=o=>L.push(JSON.stringify(o));
  P({$:'ProjectCreated',id:'p1700000000-1',title:'fixture',root,at:t0});
  P({$:'ThreadCreated',id:'t1700000001-2',project:'p1700000000-1',title:'Fixture short',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:t0+1});
  P({$:'ThreadCreated',id:'t1700000002-3',project:'p1700000000-1',title:'Fixture long',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:t0+2});
  for(let i=0;i<120;i++){
    P({$:'MessagePosted',thread:'t1700000002-3',msg:`m17000001${String(i).padStart(2,'0')}-${i+10}`,role:i%2?'assistant':'user',text:(i%2?'Answer ':'Question ')+i+': the board layout keeps U3 near the 3V3 rail, and decoupling sits within 2 mm of each supply pin.',at:t0+10+i});
    if(i%7===3)P({$:'ActivityLogged',thread:'t1700000002-3',act:`g17000001${String(i).padStart(2,'0')}-${i+300}`,tone:'tool',kind:'Bash',summary:'ls -la src/core | head -'+i,at:t0+10+i});
  }
  // a thread that existed and was deleted (scenario `unresolved` only: the other recordings keep their fixture)
  if(deleted){P({$:'ThreadCreated',id:'t1700000003-4',project:'p1700000000-1',title:'Fixture deleted',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:t0+3});P({$:'ThreadDeleted',id:'t1700000003-4',at:t0+4});}
  // scenario `private`: lines clients must never see (Change.wire_line: devices lose their payload, a turn's input and a thread's riding mail are blank)
  if(privateLines){const th='t1700000002-3',a=t0+500;
    P({$:'DeviceRegistered',token:'PRIVTOKEN-0001-aaaaaaaaaaaaaaaa',platform:'ios',kind:'push',thread:th,env:'',bundle:'works.backplane.fixture',at:a});
    P({$:'DeviceDropped',token:'PRIVDROPPED-0002-bbbbbbbbbbbbbbbb',at:a+1});
    P({$:'SettingSet',key:'ride.'+th,value:'PRIVRIDE-0003 riding mail text',at:a+2});
    P({$:'SettingSet',key:'launch.input.'+th,value:'PRIVLAUNCH-0004 a turn input',at:a+3});
    P({$:'SettingSet',key:'fallback.input.'+th,value:'PRIVFALLBACK-0005 a fallback input',at:a+4});}
  writeFileSync(home+'/events.jsonl',L.join('\n')+'\n');
}
