// Engine handoffs must retain every owner's tabs, authentication and input.
import { join } from 'node:path';
import { check, jsonProcess, privateDisplay } from '../../cef/test/harness.ts';
const d = await privateDisplay();
const server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(req) {
  const path = new URL(req.url).pathname;
  if(path==='/cookie')return new Response('',{headers:{'x-cookie':req.headers.get('cookie')??''}});
  return new Response(`<!doctype html><title>${path}</title><input id=q><select id=s><option>a</option><option>b</option></select><div style="height:3000px"></div><script>window.initial=[localStorage.getItem('local'),sessionStorage.getItem('session')];</script>`, { headers: { 'content-type': 'text/html', ...(path === '/login' ? {'set-cookie':'auth=retained; HttpOnly; Path=/; Max-Age=3600'} : {}) } });
} });
const url = `http://127.0.0.1:${server.port}`;
const flags = ['--profile',join(d.root,'chrome'), '--cache',join(d.root,'cef')];
const command = (extra: string[]) => process.env.BACKPLANE_BROWSER ? [process.env.BACKPLANE_BROWSER,...flags,...extra] : ['bun',new URL('../src/main.ts',import.meta.url).pathname,...flags,...extra];
let h = jsonProcess(command(['--engine','chrome']),d.env);
try {
  await h.call('open',{page:'a',url:url+'/login'});
  await h.call('evaluate',{page:'a',expression:"localStorage.local='first';sessionStorage.session='a';q.value='日本語 draft';s.selectedIndex=1;scrollTo(0,400)"});
  await h.call('tabs',{page:'a',new:url+'/sibling'});
  await h.call('evaluate',{page:'a',expression:"sessionStorage.session='sibling';q.value='second draft'"});
  await h.call('tabs',{page:'a',select:0});
  await h.call('open',{page:'b',url:url+'/other'});
  await h.call('evaluate',{page:'b',expression:"sessionStorage.session='b';q.value='owner b'"});
  for (const engine of ['cef','chrome']) {
    const state = await h.call('engine',{engine,parent:d.parent,display:d.env.DISPLAY});
    check(`${engine} selection is reported`,state.engine===engine && state.requestedEngine===engine,state);
    const tabs = await h.call('tabs',{page:'a'});
    check(`${engine} preserves both tabs and selection`,tabs.tabs.length===2 && tabs.active===0,tabs);
    const a = await h.call('evaluate',{page:'a',expression:"({initial,value:q.value,selected:s.selectedIndex,y:scrollY})"});
    check(`${engine} preserves pre-script storage and draft`,a.initial[0]==='first' && a.initial[1]==='a' && a.value==='日本語 draft' && a.selected===1 && a.y===400,a);
    await h.call('tabs',{page:'a',select:1});
    check(`${engine} preserves sibling state`,await h.call('evaluate',{page:'a',expression:"q.value==='second draft' && sessionStorage.session==='sibling'"}));
    await h.call('tabs',{page:'a',select:0});
    check(`${engine} preserves another owner's session`,await h.call('evaluate',{page:'b',expression:"q.value==='owner b' && sessionStorage.session==='b'"}));
    // HttpOnly cookie is observable only through the browser's network request.
    check(`${engine} preserves authentication`,await h.call('evaluate',{page:'a',expression:"fetch('/cookie').then(r=>r.headers.get('x-cookie'))"}) === 'auth=retained');
  }
  await h.call('evaluate',{page:'a',expression:"localStorage.local='updated';sessionStorage.session='updated'"});
  await h.call('open',{page:'a',url:url+'/after'});
  check('restoration hook does not overwrite later navigation',await h.call('evaluate',{page:'a',expression:"initial[0]==='updated' && initial[1]==='updated'"}));
  await h.call('engine',{engine:'cef',parent:d.parent,display:d.env.DISPLAY});
  await h.call('close'); await h.proc.exited;
  // A fresh native-default helper uses the same persistent native cookie store.
  h=jsonProcess(command(['--parent',String(d.parent),'--display',d.env.DISPLAY]),d.env);
  await h.call('open',{page:'a',url:url+'/restart'});
  check('native helper defaults to CEF after restart',(await h.call('status')).engine==='cef');
  check('native authentication persists across restart',await h.call('evaluate',{page:'a',expression:"fetch('/cookie').then(r=>r.headers.get('x-cookie'))"}) === 'auth=retained');
  await h.call('close'); await h.proc.exited;
  // Startup runtime failure is reported, and remains usable through Chrome.
  h=jsonProcess(command(['--parent',String(d.parent),'--display',d.env.DISPLAY,'--cef','/usr/bin/false']),{...d.env,BACKPLANE_CEF:''});
  // A failed native host must not strand the default client.
  await h.call('open',{page:'fallback',url:url+'/fallback'});
  const fallback=await h.call('status');
  check('unavailable native runtime reports Chrome fallback',fallback.engine==='chrome' && fallback.requestedEngine==='cef' && fallback.fallback?.includes('unavailable'),fallback);
  console.log('all default/session handoff checks passed');
} finally {
  if(h.proc.exitCode===null){h.proc.stdin.end();await h.proc.exited;}
  server.stop(true);await d.cleanup();
}
