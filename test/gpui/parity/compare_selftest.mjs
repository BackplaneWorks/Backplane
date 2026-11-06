// Fixtures that compare.mjs MUST report as differences (and ones it must not). usage: bun test/gpui/parity/compare_selftest.mjs
import {normalise,diff} from './compare.mjs';
let bad=0;const t=(name,ok)=>{console.log((ok?'ok   ':'FAIL ')+name);if(!ok)bad++};
const frame=(json,extra={})=>({c:'c1',dir:'in',kind:'bin',json,...extra});
const changes=items=>frame({t:'changes',items});
const set=(key,value='x')=>({$:'SettingSet',key,value,at:1793000000});
const dtn=(fa,fb)=>{const a=normalise(fa),b=normalise(fb),d=[];for(const c of new Set([...Object.keys(a),...Object.keys(b)]))for(const s of ['out','events','hk','deltas','replies','views','other'])diff(a[c]?.[s],b[c]?.[s],`${c}.${s}`,d);return d};
const view=drafts=>frame({t:'reply',id:1,ok:true,result:{viewRevision:'1',session:{drafts}}});

// N20: a housekeeping event written twice
t('doubled hk event is a difference',dtn([changes([set('alert.t1')])],[changes([set('alert.t1'),set('alert.t1')])]).length>0);
t('same hk events in another order are not',dtn([changes([set('alert.t1'),set('told.t1')])],[changes([set('told.t1'),set('alert.t1')])]).length===0);
// N22: liveness by presence
t('liveness missing on one side is a difference',dtn([changes([set('liveness.t1','a')])],[changes([])]).length>0);
t('liveness ticks 1 vs 3 are not',dtn([changes([set('liveness.t1','a')])],[changes([set('liveness.t1','a'),set('liveness.t1','a'),set('liveness.t1','a')])]).length===0);
t('liveness with another value is not (the reason text varies)',dtn([changes([set('liveness.t1','a')])],[changes([set('liveness.t1','b')])]).length===0);
t('liveness for another thread is a difference',dtn([changes([set('liveness.t1','a')])],[changes([set('liveness.t2','a')])]).length>0);
t('autosettle.quiet 6 vs 7 writes is not',dtn([changes(Array.from({length:6},()=>set('autosettle.quiet.t1','<q>')))],[changes(Array.from({length:7},()=>set('autosettle.quiet.t1','<q>')))]).length===0);
t('autosettle.quiet written 3 times vs never is a difference',dtn([changes(Array.from({length:3},()=>set('autosettle.quiet.t1','<q>')))],[changes([])]).length>0);
// N24: the shape of a stream
const dl=(n,txt='ab')=>[...Array.from({length:n},()=>frame({t:'delta',thread:'t1',text:txt})),frame({t:'delta_end',thread:'t1'})];
t('streamed in 3 frames vs sent as one frame is a difference',dtn(dl(3),dl(1,'ababab')).length>0);
t('3 frames vs 2 frames, same joined text, is not',dtn(dl(3,'ab'),[...dl(2,'abab').slice(0,1),frame({t:'delta',thread:'t1',text:'ab'}),frame({t:'delta_end',thread:'t1'})]).length===0);
t('same frame count, other text is a difference',dtn(dl(3,'ab'),dl(3,'ac')).length>0);
// N27 (temporary): the person's room post, `cat:""` in B only
import {n27} from './compare.mjs';
const post=(extra)=>({shown:{bot:{posts:[{id:'p1',from:'you',text:'hi',ago:'now',mine:true,...extra}]}}});
const dd=(a,b)=>diff(a,b,'x.views[0].result');
t('N27: cat "" in B against none in A is equal',dd(post({}),post({cat:''})).length===0);
t('N27: cat "fl" in B against none in A DIFFERS',dd(post({}),post({cat:'fl'})).length>0);
t('N27: cat "" in A against none in B DIFFERS',dd(post({cat:''}),post({})).length>0);
t('N27: cat "" outside bot.posts DIFFERS',diff({a:{from:'you'}},{a:{from:'you',cat:''}},'x.views[0].result').length>0);
t('N27: a bot author with cat "" in B DIFFERS',dd({shown:{bot:{posts:[{id:'p',from:'bub',text:'t'}]}}},{shown:{bot:{posts:[{id:'p',from:'bub',text:'t',cat:''}]}}}).length>0);
// N26: two keys normalising to the same string
t('two colliding keys vs one is a difference',dtn([view({'@x 1793000001':'a','@x 1793000002':'b'})],[view({'@x 1793000001':'a'})]).length>0);
t('two colliding keys vs two is not',dtn([view({'@x 1793000001':'a','@x 1793000002':'b'})],[view({'@x 1793000003':'a','@x 1793000004':'b'})]).length===0);
// N21: arrays
const A=(...ids)=>ids.map(id=>({id}));
t('moved element, equal length, is a difference',diff(A('a','b'),A('b','a')).length>0);
t('moved element, different length, is a difference',(()=>{const d=diff(A('a','b','c'),A('b','a'));return d.length>0&&d.some(x=>x.includes('only in A'))})());
t('same order, one removed: exactly the removed one',diff(A('a','b','c'),A('a','c')).length===1);
t('same id, different content, is a difference',(()=>{const d=diff([{id:'x',v:1},{id:'y'}],[{id:'x',v:2}]);return d.some(x=>x.includes('.v')||x.includes('[0]'))})());
process.exit(bad?1:0);
