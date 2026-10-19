// Run under dbus-run-session: independent bus/display/profile, real Mozc keys.
import {join} from 'node:path';
import {mkdirSync} from 'node:fs';
import {check,jsonProcess,privateDisplay,until} from '../../cef/test/harness.ts';
if(!process.env.CEF_PRIVATE_IME_BUS)throw Error('run CEF_PRIVATE_IME_BUS=1 dbus-run-session -- bun tools/browser/test/native-ime.ts');
const d=await privateDisplay();
const env={...d.env,XDG_CONFIG_HOME:join(d.root,'config'),XDG_CACHE_HOME:join(d.root,'cache'),IBUS_ADDRESS:`unix:path=${d.root}/ibus`,GTK_IM_MODULE:'ibus',QT_IM_MODULE:'ibus',XMODIFIERS:'@im=ibus',GDK_BACKEND:'x11'};
mkdirSync(env.XDG_CONFIG_HOME,{recursive:true});mkdirSync(env.XDG_CACHE_HOME,{recursive:true});
const daemon=Bun.spawn(['ibus-daemon','--address',env.IBUS_ADDRESS,'--xim','--panel','/usr/libexec/ibus-ui-gtk3','--config','disable','--emoji-extension','disable','--cache','none'],{env,stdin:'ignore',stdout:'ignore',stderr:'inherit'});
let peer:ReturnType<typeof jsonProcess>|null=null,h:ReturnType<typeof jsonProcess>|null=null;
const python=new URL('../../cef/test/ime-peer.py',import.meta.url).pathname;
const call=(op:string,args={})=>h!.call(op,args);
async function nativeText(text:string){for(const k of text)await d.x(`key ${k}`);}
try{
  await until(async()=>{const p=Bun.spawn(['ibus','list-engine'],{env,stdout:'pipe',stderr:'ignore'});const text=await new Response(p.stdout).text();return await p.exited===0&&text.includes('mozc-jp');},'private Mozc engine registry');
  peer=jsonProcess(['/usr/bin/python3',python],env);
  const gtk=await until(()=>peer!.lines.find(e=>e.window),'independent GTK Mozc peer');
  check('real Japanese engine selected',gtk.engine==='mozc-jp');
  await d.x(`place ${gtk.window} 1000 30 400 120`);
  await d.x(`focuswin ${gtk.window}`);
  await d.x('key Zenkaku_Hankaku');
  await nativeText('nihongo');
  await until(()=>peer!.lines.find(e=>e.preedit==='にほんご'),'GTK real Japanese preedit');
  check('independent GTK gets real Mozc preedit',true);
  await d.x('key space');await d.x('key Return');
  await until(()=>peer!.lines.find(e=>e.text==='日本語'),'GTK real Japanese conversion');
  check('independent GTK converts and commits 日本語',true);
  h=jsonProcess(['bun',new URL('../src/main.ts',import.meta.url).pathname,'--parent',String(d.parent),'--cache',join(d.root,'cef')],env);
  const html='data:text/html,'+encodeURIComponent(`<!doctype html><style>body{margin:20px}input,textarea,[contenteditable]{display:block;box-sizing:border-box;width:400px;height:60px;margin-bottom:20px;border:1px solid}</style><input id=q><textarea id=t></textarea><div id=r contenteditable></div><script>window.ime=[];for(const type of ['compositionstart','compositionupdate','compositionend','input'])document.addEventListener(type,e=>ime.push({type,data:e.data,target:e.target.id,composing:e.isComposing}));</script>`);
  await call('open',{page:'a',url:html});
  await call('show',{page:'a',x:30,y:30,w:600,h:500});
  await call('record',{page:'a',on:true});
  for(const [selector,y] of [['q',80],['t',160],['r',240]] as const){
    await d.x(`click 100 ${y} 1`);
    await nativeText('nihongo');
    await until(()=>call('evaluate',{page:'a',expression:`ime.some(e=>e.target==='${selector}'&&e.type==='compositionupdate'&&e.data==='にほんご')`}),`CEF ${selector} native preedit`);
    await d.x('key space');
    // Second space opens the real candidate list rather than committing text.
    await d.x('key space');
    const snap=Bun.spawn(['import','-display',env.DISPLAY,'-window','root',join(process.env.CEF_IME_SHOTS??d.root,`ime-${selector}-candidate.png`)],{env,stdout:'ignore',stderr:'inherit'});await snap.exited;
    // The second Space advances to ニホンゴ; choose the first candidate again.
    await d.x('key Up');
    await d.x('key Return');
    const expression=selector==='r'?"document.querySelector('#r').textContent":"document.querySelector('#'+"+JSON.stringify(selector)+").value";
    const actual=await until(async()=>{const value=await call('evaluate',{page:'a',expression});return value?.includes('日本語')?value:null;},`CEF ${selector} native conversion`);
    check(`${selector}: real Mozc conversion and compositionend`,actual.includes('日本語')&&await call('evaluate',{page:'a',expression:`ime.some(e=>e.target==='${selector}'&&e.type==='compositionend')`}));
  }
  await d.x('click 100 80 1');await nativeText('tesuto');
  await until(()=>call('evaluate',{page:'a',expression:"ime.some(e=>e.data==='てすと')"}),'native cancel preedit');
  await d.x('key Escape');await d.x('key Escape');
  check('escape cancels unfinished native composition',await call('evaluate',{page:'a',expression:"q.value==='日本語'"}));
  await call('hide');await call('show',{page:'a',x:30,y:30,w:600,h:500});
  check('hide/show preserves committed Japanese',await call('evaluate',{page:'a',expression:"q.value==='日本語'"}));
  await d.x('click 100 80 1');await d.x('key End');await nativeText('a');await d.x('key Return');
  check('hide/show restores Japanese native input',await until(()=>call('evaluate',{page:'a',expression:"q.value==='日本語あ'"}),'restored Japanese input'));
  await d.x('key Tab');
  await until(()=>h!.events.some(e=>e.event==='record'&&e.page==='a'&&e.step?.do==='type'&&e.step.value.includes('日本語')),'real Japanese teach recording');
  check('teach records native Japanese commits',true);
  console.log(JSON.stringify({compositionEvents:await call('evaluate',{page:'a',expression:'ime'})}));
  console.log('all real Japanese native IME checks passed');
}catch(e){console.log(JSON.stringify({gtk:peer?.lines,cef:h?await call('evaluate',{page:'a',expression:'({ime,values:[q.value,t.value,r.textContent]})'}).catch(()=>null):null}));throw e;}
finally{
  if(h){h.proc.stdin.end();await h.proc.exited;}
  if(peer){peer.proc.stdin.end();await peer.proc.exited;}
  const stop=Bun.spawn(['/usr/bin/python3',python,'--stop-private-ibus'],{env,stdout:'inherit',stderr:'inherit'});await stop.exited;
  await daemon.exited;await d.cleanup();
}
