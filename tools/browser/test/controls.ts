// Changed metadata/tab identity behavior, same accepted native host.
import { join } from 'node:path';
import { check, until, jsonProcess, privateDisplay } from '../../cef/test/harness.ts';
if (!process.env.BACKPLANE_CEF) throw new Error('set BACKPLANE_CEF to accepted native host');
const d = await privateDisplay();
const helper = jsonProcess(process.env.BACKPLANE_BROWSER ? [process.env.BACKPLANE_BROWSER,
  '--profile', join(d.root, 'profile'), '--jpeg-dir', join(d.root, 'shots')] :
  ['bun', new URL('../src/main.ts', import.meta.url).pathname, '--profile', join(d.root, 'profile'), '--jpeg-dir', join(d.root, 'shots')], d.env);
const { call, events } = helper;
const site = Bun.serve({hostname:'127.0.0.1', port:0, fetch(req) {
  const path = new URL(req.url).pathname;
  return new Response(`<!doctype html><title>Site ${path}</title><input id=q><a target=_blank href=/popup>Link popup</a>`, {headers:{'content-type':'text/html'}});
}});
const url = (path: string) => `http://127.0.0.1:${site.port}/${path}`;
try {
  for (const engine of ['cef', 'chrome']) {
    await call('engine', {engine, parent:d.parent, display:d.env.DISPLAY});
    await call('open', {page:'a', url:url('first')});
    if (engine === 'cef') await call('show', {page:'a', x:10,y:10,w:600,h:400});
    await call('type', {page:'a', selector:'#q', text:'preserved', clear:true});
    const first = (await call('tabs',{page:'a'})).tabs[0];
    check(`${engine} exposes stable tab identity`, typeof first.id === 'string');
    await call('tabs', {page:'a', new:url('second')});
    const list = await call('tabs', {page:'a'});
    const second = list.tabs.find((t:any) => t.active);
    await call('tabs', {page:'a', selectId:first.id});
    check(`${engine} stable selection retains document`, (await call('evaluate', {page:'a', expression:'q.value'})) === 'preserved');
    await call('tabs', {page:'a', closeId:second.id});
    let stale = false;
    try { await call('tabs', {page:'a', closeId:second.id}); } catch { stale = true; }
    check(`${engine} stale identity cannot close another tab`, stale && (await call('tabs',{page:'a'})).tabs[0].id === first.id);
    await call('open', {page:'b', url:url('other-owner')});
    let cross = false;
    try { await call('tabs', {page:'b', closeId:first.id}); } catch { cross = true; }
    check(`${engine} cross-owner identity refused`, cross);
    await until(() => events.some(e => e.event === 'tabs' && e.page === 'a' && e.tabs.some((t:any) => t.id === first.id && t.active && t.title === 'Site /first')), 'metadata event');
    const n = events.length;
    await call('cast', {page:'a', ms:0});
    check(`${engine} new viewer gets cached metadata without a title query`, events.slice(n).some(e => e.event === 'tabs' && e.page === 'a' && e.url === url('first')));
    await call('close', {page:'b'});
    check(`${engine} closed owner clears metadata`, events.some(e => e.event === 'tabs' && e.page === 'b' && e.tabs.length === 0));
    await call('close', {page:'a'});
  }
  console.log('all browser control metadata checks passed');
} finally {
  await call('close').catch(() => {}); helper.proc.stdin.end(); await helper.proc.exited;
  site.stop(true); await d.cleanup();
}
