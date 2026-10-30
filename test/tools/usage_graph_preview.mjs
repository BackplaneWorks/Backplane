// Local review host: renders production Bend Usage nodes with explicit fixtures.
import { pathToFileURL } from 'node:url';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
const lib=(await import(pathToFileURL(resolve(process.argv[2])).href)).default;
const review=resolve('build/usage-review');
const each=xs=>{const a=[];for(;xs?.$==='Con';xs=xs.tail)a.push(xs.head);return a;};
const escape=x=>String(x).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');
function html(n){if(n.$?.endsWith('.Txt'))return escape(n.text);let attrs=each(n.attrs).map(a=>a.$?.endsWith('.At')?`${escape(a.name)}="${escape(a.value)}"`:`data-on-${escape(a.event)}="${escape(a.action)}"`).join(' ');return `<${n.tag} ${attrs}>${each(n.kids).map(html).join('')}</${n.tag}>`;}
let ui=lib.ui();
const content=()=>each(lib.page(ui)).map(html).join('');
const shell=()=>`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Backplane Usage preview</title><link rel="stylesheet" href="/style.css"><body><main class="settings-view" style="height:auto;min-height:100vh"><header class="settings-path"><h2>Settings / Usage</h2><span>Validation fixture · not live account usage</span></header><div class="settings" id="preview">${content()}</div></main><script type="module">
import {installUsageTabs} from '/usage-tabs.js';installUsageTabs();
document.addEventListener('click',async e=>{const b=e.target.closest('[data-on-click]');if(!b)return;const action=b.getAttribute('data-on-click'), value=b.getAttribute('data-value')||'';const focused=document.activeElement?.id;const r=await fetch('/act',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action,value})});document.querySelector('#preview').innerHTML=await r.text();if(focused)document.getElementById(focused)?.focus();});
</script></body></html>`;
const server=Bun.serve({hostname:'127.0.0.1',port:4039,async fetch(req){const url=new URL(req.url);if(url.pathname==='/act'&&req.method==='POST'){const {action,value}=await req.json();ui=lib.act(ui,action,value);return new Response(content(),{headers:{'Content-Type':'text/html'}});}if(['/style.css','/usage-tabs.js'].includes(url.pathname))return new Response(Bun.file(resolve('src/web'+url.pathname)),{headers:{'Content-Type':url.pathname.endsWith('.css')?'text/css':'text/javascript'}});return new Response(shell(),{headers:{'Content-Type':'text/html'}});}});
if (process.env.USAGE_CAPTURE) {
  try { await import('./usage_graph_capture.mjs'); }
  finally { await server.stop(true); }
} else {
  writeFileSync(resolve(review,'preview-ready'),'http://127.0.0.1:4039\n');
  console.log('Usage preview ready http://127.0.0.1:4039 — production Usage renderer, deterministic TurnUsage fixtures');
}
