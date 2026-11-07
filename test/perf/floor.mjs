const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
import {mkdirSync} from 'node:fs';import {makeStandins,startHub,stopHub,client,sleep,pct} from './lib.mjs';
const R=BP_ROOT;const root=R+'/build/gpui/perf-run/floor';mkdirSync(root,{recursive:true});makeStandins(root+'/bin');
const h=await startHub({bin:R+'/build/gpui/backplane-serve',home:root+'/home',port:3809,standins:root+'/bin'});
const t=[];for(let i=0;i<100;i++){const a=performance.now();await fetch(h.url+'/hello');t.push(performance.now()-a);await sleep(30)}
console.log('http /hello p50',pct(t,50).toFixed(2),'p95',pct(t,95).toFixed(2));
const c=await client(h);for(const gap of [0,3,30,200]){const r=[];for(let i=0;i<60;i++){r.push((await c.rpc('load.ping',{})).ms);await sleep(gap)}console.log('ws ping gap',gap,'p50',pct(r,50).toFixed(2),'min',Math.min(...r).toFixed(2),'p95',pct(r,95).toFixed(2))}
stopHub(h);process.exit(0)
