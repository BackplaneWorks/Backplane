// Select invariants independently: one red property cannot conceal another.
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {learnJson,json,plain} from '../../src/web/wire.js';
import {replay,list} from '../../test/tools/ahead_validation.mjs';
const F=(await import(pathToFileURL(resolve(process.argv[2])))).default;
learnJson(F.json_tags());const checks={},start=performance.now();
for(const [name,test] of Object.entries({
  hiddenUnread(){let u=replay(F,[{kind:'act',action:'select',value:'B'},
    {kind:'recv',frame:{t:'changes',items:[{$:'MessagePosted',thread:'B',msg:'seen',role:'assistant',text:'visible',at:5}]}},
    {kind:'act',action:'hold-away',value:'B'}],{render:false}).u;
    u=F.ui(F.received(u,json({t:'changes',items:[{$:'MessagePosted',thread:'B',msg:'hidden',role:'assistant',text:'',at:6}]})));
    const actual=F.pill(u);return {pass:actual==='',actual};},
  pendingBudget(){const text='pasted article 日本語. '.repeat(4000),r=replay(F,[{kind:'act',action:'select',value:'A'},
    {kind:'act',action:'draft',value:text},{kind:'act',action:'send',value:''}],{render:false});
    const chars=list(F.pending(F.fitted(r.u))).join('').length;return {pass:chars<2048,chars};},
  surrogate(){const actual=plain(F.parse('"\\ud800"'));return {pass:actual==='�',actual};},
})){try{checks[name]=test();}catch(e){checks[name]={pass:false,reason:String(e)};}}
console.log(JSON.stringify({checks,seconds:(performance.now()-start)/1000}));
