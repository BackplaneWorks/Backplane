// Per-owner upgrade restore uses the same pre-script handoff as engine switches.
import {join} from 'node:path';
import {check,jsonProcess,privateDisplay} from '../../cef/test/harness.ts';
const d=await privateDisplay();
const site=Bun.serve({hostname:'127.0.0.1',port:0,fetch:()=>new Response('<!doctype html><input id=q><script>window.initial=[localStorage.a,sessionStorage.b]</script>',{headers:{'content-type':'text/html;charset=utf-8'}})});
const url=`http://127.0.0.1:${site.port}`;
const flags=['--parent',String(d.parent),'--cache',join(d.root,'cef')];
const h=jsonProcess(process.env.BACKPLANE_BROWSER?[process.env.BACKPLANE_BROWSER,...flags]:['bun',new URL('../src/main.ts',import.meta.url).pathname,...flags],d.env);
try{
  await h.call('restore',{page:'a',state:{url,origin:url,local:[['a','local']],session:[['b','session']],fields:[{selector:'#q',value:'draft 日本語',checked:false,selected:-1}],scroll:[0,0]}});
  check('upgrade restore is native by default',(await h.call('status',{page:'a'})).engine==='cef');
  check('upgrade restore retains pre-script storage and draft',await h.call('evaluate',{page:'a',expression:"initial[0]==='local'&&initial[1]==='session'&&q.value==='draft 日本語'"}));
  check('restore is scoped to its owner',(await h.call('pages')).pages.length===1);
  console.log('all upgrade restore checks passed');
}finally{h.proc.stdin.end();await h.proc.exited;site.stop(true);await d.cleanup();}
