import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {makeReplayTrace,validateReplayTrace,TRACE_LIMITS} from '../../src/server/ui-replay-trace.js';
import {replay} from '../../test/tools/ahead_validation.mjs';

export function expand(trace){
  assert(validateReplayTrace(trace),'invalid or private trace schema');
  // Content-free traces reconstruct structure with PUBLIC synthetic strings.
  // They cannot reproduce failures requiring the original text or CSS geometry.
  const ids=[...new Set(trace.events.flatMap(e=>e.ids).filter(Boolean))];
  const threads=ids.map(id=>({$:'ThreadCreated',id:'trace'+id,project:'p',title:'Synthetic trace thread',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:2}));
  return [{kind:'recv',frame:{t:'changes',items:threads}},...trace.events.flatMap((r,i)=>{
    const target=r.ids[0]?'trace'+r.ids[0]:'A';
    switch(r.kind){
      case 'select':return [{kind:'act',action:'select',value:target}];
      case 'bot':return [{kind:'act',action:'bot',value:'bot'}];
      case 'draft':return [{kind:'act',action:'draft',value:'Synthetic draft '+i}];
      case 'send':return [{kind:'act',action:'send',value:''}];
      case 'stream':return [{kind:'recv',frame:{t:'delta',thread:target,text:'Synthetic stream'}}];
      case 'reply':return [{kind:'recv',frame:{t:'changes',items:[{$:'MessagePosted',thread:target,msg:'trace'+i,role:'assistant',text:'Synthetic reply',at:i+10}]}}];
      case 'reconnect':return [{kind:'recv',frame:{t:'log',origin:'synthetic',since:1,items:[]}}];
      case 'newest':return [{kind:'act',action:'hold-end',value:target}];
      case 'view-error':return [{kind:'recv',frame:{t:'reply',id:99999,ok:false,error:'synthetic'}}];
      // Page and scroll are transport/geometry events: counted but never
      // mislabeled as Ui.act coverage. Transport replay owns them separately.
      default:return [];
    }
  })];
}
const start=performance.now(),F=(await import(pathToFileURL(resolve(process.argv[2])))).default;
let at=0;const recorder=makeReplayTrace({enabled:true,now:()=>at++});
for(const [action,value] of [['select','private-thread'],['draft','PRIVATE_MESSAGE'],['send','PRIVATE_MESSAGE']])
  recorder.rpc({m:'view.action',p:{action,value,session:{sel:'private-thread'},private:'PRIVATE_SECRET'}});
recorder.record('reply',['private-thread']);recorder.record('reconnect');recorder.record('stream',['private-thread']);
const synthetic=recorder.export();assert(validateReplayTrace(synthetic));
assert(!JSON.stringify(synthetic).includes('PRIVATE'));assert(!JSON.stringify(synthetic).includes('private-thread'));
const disabled=makeReplayTrace();disabled.record('send',['private']);assert.equal(disabled.export().events.length,0);
for(let i=0;i<10000;i++)recorder.record('stream',['id'+i]);
assert.equal(recorder.export().events.length,TRACE_LIMITS.events);assert(recorder.export().dropped>0);
for(const invalid of [{...synthetic,text:'private'}, {...synthetic,events:[{...synthetic.events[0],text:'private'}]},
  {...synthetic,events:[{kind:'stream',ids:[129],dt:0}]}])assert.equal(validateReplayTrace(invalid),false);
const trace=process.argv[3]?JSON.parse(readFileSync(process.argv[3],'utf8')):synthetic;
const sequence=expand(trace),run=replay(F,sequence);
const result={pass:true,approach:'trace',events:trace.events.length,replayed:run.steps,negativeControls:4,
  seconds:(performance.now()-start)/1000,scope:'new schema owned by ahead-validation; opt-in hub-boundary hook, synthetic content; not yet installed in live dispatch',
  unsupported:trace.events.filter(e=>['page','scroll'].includes(e.kind)).length};
if(process.env.TRACE_FIXTURE_OUT)writeFileSync(process.env.TRACE_FIXTURE_OUT,JSON.stringify(synthetic,null,2)+'\n');
console.log(JSON.stringify(result));
