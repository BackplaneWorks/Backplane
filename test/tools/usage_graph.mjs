import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
const lib = (await import(pathToFileURL(process.argv[2]).href)).default;
const list = xs => { const a=[]; for (; xs?.$ === 'Con'; xs=xs.tail) a.push(xs.head); return a; };
let checks=0;
const check=(label, actual, expected)=>{ assert.deepEqual(actual, expected, label); checks++; console.log('ok '+label); };
const u=lib.ui();
check('only advertised providers have tabs',list(lib.providers(u)),['claude','codex','grok']);
check('first available provider is selected',lib.selected(u),'claude');
check('local provider selection emits no request',lib.no_requests(u),0n);
const cx=lib.act(u,'usage-provider','codex');
check('provider selection is retained',lib.selected(cx),'codex');
check('disabled provider cannot be selected',lib.selected(lib.act(cx,'usage-provider','muse')),'claude');
check('selection follows changed capabilities',lib.selected(lib.caps(cx,'{"grok":["m"]}')),'grok');
check('no capabilities invents no provider',lib.selected(lib.caps(u,'{}')),'');
const cldays=list(lib.days(u)), cxdays=list(lib.days(cx));
const day=date=>cldays.find(d=>d.date===date);
const parts=v=>v.split('/').map(BigInt);
check('daily provider aggregation includes input cache and output once',parts(day('2026-10-15').total).slice(0,6),[1n,10n,20n,30n,40n,5n]);
check('another model belongs to the same provider daily series',parts(day('2026-10-16').total).slice(1,6),[100n,0n,0n,200n,150n]);
check('missing days remain missing',day('2026-10-01').total,'');
check('provider tabs have distinct real readings',parts(cxdays.find(d=>d.date==='2026-10-15').total)[1],100n);
check('explicit token zero is a reading',parts(cxdays.find(d=>d.date==='2026-10-18').total)[8],1n);
check('unknown zero is never labelled a recorded zero',lib.reading('1/0/0/0/0/0/0/0/0').startsWith('Token readings unknown'),true);
check('old aggregate is not extrapolated into provider days',list(lib.legacy_days())[0].total,'');
check('epoch reset excludes previous daily readings',list(lib.days(lib.epoch(u))).every(d=>d.total===''),true);
const ps=list(lib.projects(u));
check('projects ranked across models/providers, not largest model',ps.map(p=>[p.id,p.title,p.tokens]),[['p','Board',510n],['p2','Board',350n],['gone','Old project',30n],['','Unknown project',10n]]);
check('top five stays bounded',list(lib.top_many()).length,5);
check('missing catalog retains project identity',list(lib.top_many()).slice(0,2).map(p=>[p.id,p.title]),[['p','Unavailable project'],['p2','Unavailable project']]);
const today=lib.act(cx,'usage-period','today');
check('today uses one UTC day',list(lib.days(today)).map(d=>d.date),['2026-10-18']);
check('today never substitutes monthly projects',list(lib.projects(today)),[]);
for (const iso of ['2026-10-01T00:00:00Z','2026-10-09T23:59:59Z','2026-10-10T00:00:00Z','2026-11-01T00:00:00Z','2028-02-29T23:59:59Z','2026-12-31T23:59:59Z']) {
 const now=BigInt(Date.parse(iso)/1000), date=new Date(Number(now)*1000);
 const first=Date.UTC(date.getUTCFullYear(),date.getUTCMonth(),1)/1000;
 const monthDays=list(lib.days_at(now,'month')), todayDays=list(lib.days_at(now,'today'));
 check('zero-seeded UTC day offset '+iso,lib.month_offset(now),BigInt(date.getUTCDate()-1));
 check('UTC month boundary '+iso,lib.count(now,'month'),BigInt(date.getUTCDate()));
 check('UTC start '+iso,lib.start(now,'month'),BigInt(first));
 check('monthly graph dates stay inside UTC month '+iso,monthDays.map(d=>d.date),Array.from({length:date.getUTCDate()},(_,i)=>new Date((first+i*86400)*1000).toISOString().slice(0,10)));
 check('Today graph is exactly the UTC date '+iso,todayDays.map(d=>d.date),[iso.slice(0,10)]);
}
// Rolling windows include the current UTC day and never reset on a Monday,
// month boundary, year boundary, leap day or local daylight-saving transition.
for (const iso of ['2026-11-02T12:30:00Z','2027-01-01T00:00:00Z','2028-03-01T23:59:59Z','2026-03-08T07:01:00Z']) {
 const now=BigInt(Date.parse(iso)/1000), midnight=now/86400n*86400n;
 for (const [period,n] of [['week',7],['2weeks',14],['4weeks',28],['30days',30]]) {
  const dates=list(lib.days_at(now,period)).map(d=>d.date);
  check('rolling count '+period+' '+iso,lib.count(now,period),BigInt(n));
  check('rolling start '+period+' '+iso,lib.start(now,period),midnight-BigInt(n-1)*86400n);
  check('rolling days '+period+' '+iso,dates,Array.from({length:n},(_,i)=>new Date(Number(midnight-BigInt(n-1-i)*86400n)*1000).toISOString().slice(0,10)));
 }
}
check('rolling token/cost counters include endpoints and exclude outside days',parts(lib.rolling_total('week')),[3n,80n,0n,0n,20n,10n,100000000n,1n,2n]);
check('rolling ranking combines providers and months once',list(lib.rolling_projects('week')).map(p=>[p.id,p.tokens]),[['steady',100n]]);
check('top five is selected after summing project days',list(lib.rolling_ranked()).map(p=>p.tokens),[100n,60n,60n,60n,60n]);
check('reset epoch excludes all rolling counters',lib.rolling_reset(),'');
check('empty rolling range stays unknown',lib.rolling_empty(),'');
const flatten=xs=>list(xs).flatMap(n=>[n,...flatten(n.kids)]);
const text=nodes=>nodes.filter(n=>n.$?.endsWith('.Txt')).map(n=>n.text).join(' ');
const nodes=flatten(lib.page(u));
const tabs=nodes.filter(n=>n.tag==='button'&&list(n.attrs).some(a=>a.name==='role'&&a.value==='tab'));
check('production page has enabled tabs only',tabs.length,3);
const controls=nodes.filter(n=>n.tag==='button');
const action=n=>list(n.attrs).find(a=>a.action)?.action;
check('period controls grouped independently of metrics',controls.filter(n=>action(n)==='usage-period').length,6);
for (const [period,n] of [['week',7],['2weeks',14],['4weeks',28],['30days',30]]) {
 const changed=lib.act(u,'usage-period',period);
 check('period action selects rolling graph '+period,list(lib.days(changed)).length,n);
 check('rolling project totals follow selection '+period,list(lib.projects(changed)).map(p=>p.tokens),[510n,350n,30n,10n]);
 check('rolling caption uses actual dates '+period,text(flatten(lib.page(changed))).includes(list(lib.days(changed))[0].date+' – 2026-10-18 (UTC)'),true);
}
check('default graph is rolling month',cldays.length,30);
check('top projects is below daily plot',text(nodes).indexOf('Top 5 projects')>text(nodes).indexOf('Daily recorded tokens'),true);
check('cost view never relabels token graph as money',text(flatten(lib.page(lib.act(u,'usage-metric','cost')))).includes('Daily recorded tokens'),false);
console.log(JSON.stringify({pass:true,checks,scope:'production aggregation, UI actions, web nodes; deterministic TurnUsage validation fixture'}));
