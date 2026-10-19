import {join} from 'node:path';
import {writeFileSync} from 'node:fs';
import {check,jsonProcess,privateDisplay,until} from '../../cef/test/harness.ts';
if(!process.env.CEF_NATIVE_PARENT)throw Error('set CEF_NATIVE_PARENT to built test/native/cef_parent.bend');
const d=await privateDisplay();
const file=join(d.root,'日本語 spaced file.txt');const content='Native dropped file 日本語\n';writeFileSync(file,content);
const peer=jsonProcess(['/usr/bin/python3',new URL('../../cef/test/drag-source.py',import.meta.url).pathname,file],{...d.env,GDK_BACKEND:'x11',GDK_DEBUG:process.env.CEF_DND_DEBUG??''});
const h=jsonProcess(['bun',new URL('../src/main.ts',import.meta.url).pathname,'--parent',String(d.parent),'--cache',join(d.root,'cef')],d.env);
const html='data:text/html,'+encodeURIComponent(`<!doctype html><style>body{margin:0}#target{height:300px;background:#acf}#outgoing{height:70px;background:#fca}</style><div id=target>Drop files or text here</div><div id=outgoing draggable=true>Outgoing text</div><script>window.drops=[];window.dragEvents=[];for(const name of ['dragenter','dragover','dragleave','drop'])document.addEventListener(name,e=>dragEvents.push({name,x:e.clientX,y:e.clientY,types:[...e.dataTransfer.types]}));target.ondragover=e=>e.preventDefault();target.ondrop=async e=>{e.preventDefault();drops.push({text:e.dataTransfer.getData('text/plain'),files:await Promise.all([...e.dataTransfer.files].map(async f=>({name:f.name,text:await f.text()})))})};outgoing.ondragstart=e=>e.dataTransfer.setData('text/plain','CEF outbound 日本語');</script>`);
const site=Bun.serve({port:0,hostname:'127.0.0.1',fetch:()=>new Response(decodeURIComponent(html.slice('data:text/html,'.length)),{headers:{'content-type':'text/html; charset=utf-8'}})});
try{
  const window=(await until(()=>peer.lines.find(e=>e.window),'GTK independent peer')).window;
  await d.x(`place ${window} 1000 30 320 320`);
  for(const page of ['a','b'])await h.call('open',{page,url:`http://127.0.0.1:${site.port}`});
  console.log(JSON.stringify({gtkGeometry:await d.x(`info ${window}`)}));
  for(const page of ['a','b','a']){
    await h.call('show',{page,x:30,y:30,w:600,h:400});
    await h.call('shield',{on:false});
    await d.x('drag 1100 70 200 150');
    const fileDrop=await until(async()=>{const a=await h.call('evaluate',{page,expression:'drops'});return a.find((e:any)=>e.files.length);},'real GTK file drop');
    check(`${page}: native file name and bytes`,fileDrop.files[0].name==='日本語 spaced file.txt'&&fileDrop.files[0].text===content,fileDrop);
    await d.x('drag 1100 150 200 150');
    await until(async()=>await h.call('evaluate',{page,expression:"drops.some(d=>d.text==='日本語 drag text')"}),'native text drop');
    check(`${page}: native text drag arrives`,true);
  }
  const attachmentCount=d.nativeEvents.filter(e=>e.drop).length;
  await d.x('drag 1100 70 800 500');
  await until(()=>d.nativeEvents.filter(e=>e.drop).length>attachmentCount,'outside-panel attachment drop');
  check('outside browser keeps normal app file attachments',d.nativeEvents.filter(e=>e.drop).at(-1).drop.includes(encodeURIComponent('日本語 spaced file.txt')));
  const before=await h.call('evaluate',{page:'a',expression:'drops.length'});
  await h.call('shield',{on:true});
  await d.x('drag 1100 70 200 150');
  await Bun.sleep(200);
  check('agent shield refuses desktop drop',await h.call('evaluate',{page:'a',expression:'drops.length'})===before);
  check('shield never turns browser drop into app attachment',d.nativeEvents.filter(e=>e.drop).length===attachmentCount+1);
  await h.call('shield',{on:false});
  await d.x('drag 1100 70 200 150');
  await until(async()=>await h.call('evaluate',{page:'a',expression:'drops.length'})===before+1,'unshielded desktop drop');
  check('takeover restores native drag',true);
  await d.x('drag 200 365 1100 265');
  await until(()=>peer.lines.find(e=>e.received==='CEF outbound 日本語'),'CEF text dragged to GTK');
  check('native text drags out to GTK',true);
  await h.call('hide');
  const hiddenBefore=d.nativeEvents.filter(e=>e.drop).length;
  await d.x('drag 1100 70 200 150');
  await until(()=>d.nativeEvents.filter(e=>e.drop).length===hiddenBefore+1,'hidden browser normal attachment');
  check('hide restores attachment drop path',(await d.x('native-target')).target===0&&(await d.x('proxy')).proxy===0);
  await d.x('stale-target');
  await d.x('drag 1100 70 200 150');
  await until(()=>d.nativeEvents.filter(e=>e.drop).length===hiddenBefore+2,'stale CEF target ignored');
  check('stale target cannot capture an app attachment',true);
  console.log('all native desktop drag checks passed');
}catch(e){
  console.log(JSON.stringify({peer:peer.lines,geometry:await d.x('children'),proxy:await d.x('proxy'),messages:await d.x('messages'),dom:await h.call('evaluate',{page:'a',expression:'({drops,dragEvents:dragEvents.slice(-12)})'}).catch(()=>null)}));
  throw e;
}finally{h.proc.stdin.end();await h.proc.exited;peer.proc.stdin.end();await peer.proc.exited;site.stop(true);await d.cleanup();}
