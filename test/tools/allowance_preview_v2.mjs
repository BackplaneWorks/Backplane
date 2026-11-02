// Additive validation adapter: serialize textarea value as HTML text, like the production host sets its value property.
// Production Bend page/composer and actions, with explicit allowance fixtures.
// This is a private review server, not a live hub or a provider usage claim.
import {pathToFileURL} from 'node:url';
import {resolve} from 'node:path';
import {writeFileSync} from 'node:fs';
const lib=(await import(pathToFileURL(resolve(process.argv[2])).href)).default;
const port=Number(process.env.ALLOWANCE_PREVIEW_PORT||4041);
let ui=lib.ui();
const e=x=>String(x).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');
function html(n){if(n.type==='text')return e(n.text);const as=n.attrs.map(x=>x.event?`data-on-${e(x.event)}="${e(x.action)}"`:`${e(x.name)}="${e(x.value)}"`);return `<${n.tag} ${as.join(' ')}>${n.tag==='textarea'?e(n.attrs.find(a=>a.name==='value')?.value||''):n.kids.map(html).join('')}</${n.tag}>`;}
const node=u=>JSON.parse(lib['../src/core/json.Json.show'](lib['../src/web/presentation.Node.write'](lib['../src/web/view.View.page'](u))));
const content=()=>html(node(ui));
const shell=()=>`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Backplane allowance · isolated validation fixture</title><link rel="stylesheet" href="/style.css"><body><div id="root">${content()}</div><script type="module">
import {installWorkspaceKeys} from '/workspace-keys.js';
async function dispatch(action,value=''){const focus=document.activeElement?.id;const raw=await fetch('/act',{method:'POST',body:JSON.stringify({action,value})}).then(r=>r.json());document.querySelector('#root').innerHTML=raw.html;if(action==='allowance-open')document.querySelector('#allowance-close')?.focus();else if(action==='allowance-close')document.querySelector('#allowance-chip')?.focus();else if(focus)document.getElementById(focus)?.focus();}
installWorkspaceKeys({dispatch});
document.addEventListener('click',e=>{const b=e.target.closest('[data-on-click]');if(b){e.preventDefault();dispatch(b.dataset.onClick,b.dataset.value||'');}});
document.addEventListener('input',e=>{if(e.target.id==='composer')fetch('/act',{method:'POST',body:JSON.stringify({action:'draft',value:e.target.value})});});
window.allowanceFixture=async scenario=>{const r=await fetch('/scenario',{method:'POST',body:scenario}).then(r=>r.text());document.querySelector('#root').innerHTML=r;};
</script></body></html>`;
const server=Bun.serve({hostname:'127.0.0.1',port,async fetch(req){const path=new URL(req.url).pathname;
 if(path==='/act'){const {action,value}=await req.json();ui=lib.act(ui,action,value);return Response.json({html:content()});}
 if(path==='/scenario'){const scenario=await req.text();ui=lib.ui();if(scenario==='stale')ui=lib.tick(ui,1793168000n);else if(scenario==='unknown')ui=lib.model(ui,'grok');else if(scenario==='reset')ui=lib.tick(ui,1793176001n);return new Response(content());}
 if(path==='/style.css'||path==='/workspace-keys.js')return new Response(Bun.file(resolve('src/web'+path)),{headers:{'content-type':path.endsWith('.css')?'text/css':'text/javascript'}});
 return new Response(shell(),{headers:{'content-type':'text/html'}});
}});

console.log(`Isolated production-page allowance fixture listening on 127.0.0.1:${port}`);
