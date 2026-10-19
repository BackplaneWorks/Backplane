// Compare actual X focus and DnD routing against the active CEF child.
import {join} from 'node:path';
import {check,jsonProcess,privateDisplay,until} from '../../cef/test/harness.ts';
const d=await privateDisplay();
const h=jsonProcess(['bun',new URL('../src/main.ts',import.meta.url).pathname,'--parent',String(d.parent),'--cache',join(d.root,'cef')],d.env);
try {
  const html='data:text/html,'+encodeURIComponent('<input id=q><title>Native routing</title>');
  await h.call('open',{page:'a',url:html});
  await h.call('open',{page:'b',url:html});
  for(const page of ['a','b','a']){
    await h.call('show',{page,x:20,y:20,w:600,h:400});
    await h.call('focus',{page});
    const outer=(await d.x('children')).children.find((w:any)=>w.mapped&&!w.inputOnly);
    const child=(await d.x(`children ${outer.xid}`)).children.find((w:any)=>!w.inputOnly);
    const actual={page,outer:outer.xid,child:child?.xid,focus:(await d.x('focus')).focus,proxy:(await d.x('proxy')).proxy};
    console.log(JSON.stringify(actual));
    check('focus targets the native Chromium widget',actual.focus===actual.child,actual);
    check('app remains stable desktop drag recipient',actual.proxy===d.parent,actual);
    const routing=await d.x('native-target');
    check('drop target follows active owner with live ownership',routing.target===actual.child&&routing.owner===actual.child,routing);
    await d.x('click 45 45 1');await d.x('key a');
    await until(()=>h.call('evaluate',{page,expression:"q.value==='a'"}),'native key');
  }
  await h.call('hide');
  check('hiding restores parent drop routing',(await d.x('proxy')).proxy===0);
}finally{h.proc.stdin.end();await h.proc.exited;await d.cleanup();}
