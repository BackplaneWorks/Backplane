// Focused script-popup acceptance; reuse the accepted helper/host on private X.
import {join} from 'node:path';
import {check,jsonProcess,privateDisplay,until} from '../../cef/test/harness.ts';
const d=await privateDisplay();
const h=jsonProcess([process.env.BACKPLANE_BROWSER!, '--parent',String(d.parent),'--cache',join(d.root,'cef')],d.env);
const site=Bun.serve({hostname:'127.0.0.1',port:0,fetch(req){
  const path=new URL(req.url).pathname;
  return new Response(path==='/popup'
    ? '<!doctype html><title>Script popup ready</title><input id=q><script>window.ready=true;window.recorderAtFirstScript=!!window.__bpRecInstalled;window.opener?.postMessage("popup-ready","*")</script>'
    : '<!doctype html><title>Opener</title><input id=draft><a id=link target=_blank href=/popup>Link</a><button id=sync onclick="window.open(\'/popup\')">Script</button><button id=async onclick="setTimeout(()=>window.open(\'/popup\'),20)">Async</button><script>window.messages=[];onmessage=e=>messages.push(e.data)</script>',
    {headers:{'content-type':'text/html'}});
}});
try{
  for(const kind of (process.env.POPUP_CASES??'link,sync,async,evaluate,blank-navigate').split(',')){
    const page='popup-'+kind;
    await h.call('open',{page,url:`http://127.0.0.1:${site.port}/opener`});
    await h.call('show',{page,x:30,y:30,w:600,h:400});
    await h.call('type',{page,selector:'#draft',text:'retained opener '+kind});
    try{
      if(['link','sync','async'].includes(kind))await h.call('click',{page,selector:'#'+kind});
      else await h.call('evaluate',{page,expression:kind==='evaluate'?"void window.open('/popup')":"void (()=>{const p=window.open('about:blank');p.location.href='/popup'})()"});
      await until(async()=>{const t=await h.call('tabs',{page});return t.tabs.length===2&&t.active===1&&t.tabs[1].title==='Script popup ready';},kind+' popup navigated',10000);
      check(kind+': popup adopted and script executed',await h.call('evaluate',{page,expression:'window.ready===true'}));
      check(kind+': recorder installed before document scripts',await h.call('evaluate',{page,expression:'window.recorderAtFirstScript===true'}));
      await h.call('type',{page,selector:'#q',text:'native target '+kind});
      check(kind+': popup input works',await h.call('evaluate',{page,expression:`q.value===${JSON.stringify('native target '+kind)}`}));
      await h.call('tabs',{page,close:1});
      // target=_blank links intentionally have noopener in modern Chromium.
      check(kind+': opener state preserved',await h.call('evaluate',{page,expression:`draft.value===${JSON.stringify('retained opener '+kind)}`}));
      if(kind!=='link')check(kind+': opener communication preserved',await h.call('evaluate',{page,expression:"messages.includes('popup-ready')"}));
    }catch(e){const s=await h.call('status',{page});console.log(JSON.stringify({kind,status:s,targets:await(await fetch(`http://127.0.0.1:${s.port}/json/list`)).json()}));throw e;}
  }
  console.log('all script-created CEF popup checks passed');
}finally{site.stop(true);h.proc.stdin.end();await h.proc.exited;await d.cleanup();}
