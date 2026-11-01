// hotboot.js's lines as clients see them: a turn's input setting and a
// thread's riding mail blank, the
// same text codec.bend's Change.wire writes (law input_wire_blank), every
// other line untouched. No hub is started.
//   bun test/tools/hotboot_wire_test.cjs
const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const root=path.resolve(__dirname,'../..'),source=fs.readFileSync(path.join(root,'src/server/effects/hotboot.js'),'utf8');
const c={require,Buffer,io_eff(){},CID:x=>x,Nil:'Nil',Con:'Con',Hot:{lines:'Hot.lines'}};vm.createContext(c);vm.runInContext(source,c);
const lines=(file,to,skips=[])=>{let xs={$:'Nil'};for(const s of skips.slice().reverse())xs={$:'Con',head:String(s),tail:xs};const out=[];for(let l=c.hot_lines(file,'0',String(to),xs);l.$==='Con';l=l.tail)out.push(l.head);return out.reverse();};
const set=(key,value,at)=>JSON.stringify({$:'SettingSet',key,value,at});
const h=fs.mkdtempSync(path.join(os.tmpdir(),'bp-hotwire-'));
try{
  const big='Original request "x" ,"at":7 日本語\n'.repeat(500);
  const log=[set('fallback.input.t1','big "x"',5),set('launch.input.run-a',big,1793502963),set('agent.model','opus',6),
    set('input.launch.t1','kept',7),JSON.stringify({$:'MessagePosted',thread:'t1',id:'m1',role:'user',text:'{"$":"SettingSet","key":"launch.input.t1"',at:8}),
    set('fallback.input.t2','',9),set('ride.t3','[{"id":"m1","text":"a waiting note, \\"quoted\\""}]',10),set('rides.t3','kept',11)];
  const file=path.join(h,'events.jsonl');fs.writeFileSync(file,log.join('\n')+'\n');
  const got=lines(file,fs.statSync(file).size);
  assert.equal(got[0],'{"$":"SettingSet","key":"fallback.input.t1","value":"","at":5}');
  assert.equal(got[1],'{"$":"SettingSet","key":"launch.input.run-a","value":"","at":1793502963}');
  for(const i of [2,3,4])assert.equal(got[i],log[i]);
  assert.equal(got[5],log[5]);
  // a thread's riding mail goes blank too (law ride_wire_blank); a key that only starts alike is kept
  assert.equal(got[6],'{"$":"SettingSet","key":"ride.t3","value":"","at":10}');
  assert.equal(got[7],log[7]);
  assert.equal(got.length,log.length);
  for(const l of got)JSON.parse(l);
  console.log('ok hotboot wire: inputs and rides blank, other lines kept');
}finally{fs.rmSync(h,{recursive:true})}
