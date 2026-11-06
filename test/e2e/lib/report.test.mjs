// bun test/e2e/lib/report.test.mjs (or node): per-press verdicts and the harm ranking
import assert from 'node:assert/strict';
import {kindOf,verdict,judgeSteps,stepSummary,frictions,markdown,DEFAULT_BUDGETS as B} from './report.mjs';
assert.equal(kindOf('select:3'),'navigation');assert.equal(kindOf('home:0'),'navigation');assert.equal(kindOf('click:rail:Board'),'press');assert.equal(kindOf('viewer_drawn:board'),'viewer');
assert.equal(kindOf('popup:/'),'press');assert.equal(kindOf('net_sleep:45'),'network');assert.equal(kindOf('send'),'press');
// budgets: press ok <= 300, slow <= 2000; navigation ok <= 1000, slow <= 3000
assert.equal(verdict(250,'press',B),'ok');assert.equal(verdict(301,'press',B),'slow');assert.equal(verdict(2001,'press',B),'fail');
assert.equal(verdict(2500,'navigation',B),'slow');assert.equal(verdict(10,'press',B,false),'fail');assert.equal(verdict(NaN,'press',B),'unmeasured');
// net_sleep is judged on the reconnect only (its latency includes the sleep)
let s=judgeSteps([{step:'net_sleep:45',ok:true,ms:47000},{step:'select:0',ok:true,ms:120},{step:'popup:@',ok:false,ms:15000,error:'timeout'},{step:'type:5',ok:true,ms:900}]);
assert.equal(s[0].judgedMs,2000);assert.equal(s[0].verdict,'ok');assert.equal(s[2].verdict,'fail');assert.equal(s[3].verdict,'slow');
const t=stepSummary(s);assert.equal(t.ok,2);assert.equal(t.slow,1);assert.equal(t.fail,1);assert.equal(t.slowest.step,'popup:@');
// ranking: a press that never answers outranks a layout jump, which outranks a slow press; blocked journeys are not frictions
const rs=[{journey:'a',status:'fail',steps:s,metrics:{layout:{pass:false,score:0.3,shifts:2,moved:[{ids:['main']}]}}},
  {journey:'b',status:'blocked',blocked:['keys'],steps:[],metrics:{}},
  {journey:'c',status:'partial',steps:judgeSteps([{step:'select:1',ok:true,ms:1800}]),metrics:{dead:{dead:[{id:'x',label:'X'}],deadLooksEnabled:[],disabledNoHandler:[{id:'y',label:'Y'}]}}}];
const f=frictions(rs);
assert.deepEqual(f.map(x=>x.kind),['step_failed','layout_jump','dead_looks_enabled','slow','slow','disabled_no_handler']);
assert.ok(!f.some(x=>x.journey==='b'));
const md=markdown({client:'gpui',when:'now',results:rs.map(r=>({...r,stepSummary:stepSummary(r.steps)})),frictions:f,counts:{fail:1}});
assert.ok(md.includes('## Friction, ranked by harm (6)'));assert.ok(md.includes('- b: keys'));assert.ok(md.includes('| a | popup:@ |'));
console.log('report.test.mjs: all assertions passed');
