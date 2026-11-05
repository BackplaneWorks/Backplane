// The must-pass GPUI input actions with real OS input on Xvfb (Boat): drag-select, Ctrl-C, paste an image, drop a file, IME, paperclip, link click.
// bun test/gpui/e2e/input-run.mjs --hub-bin PATH --gpui-bin PATH [--out DIR]   (Boat only; see boat-run.sh input-run)
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';import {spawn,execFileSync} from 'node:child_process';import {resolve} from 'node:path';
import {startDisplay,installMissing} from './lib/capture.mjs';import {startFixtureHub} from './lib/hubfx.mjs';import {Input} from './input/driver.mjs';
const arg=(k,d)=>{const i=process.argv.indexOf('--'+k);return i>=0?process.argv[i+1]:d};const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const hubBin=resolve(arg('hub-bin')),gpuBin=resolve(arg('gpui-bin')),out=resolve(arg('out','out/e2e/input'));mkdirSync(out,{recursive:true});
const summary={when:new Date().toISOString(),actions:{},blocked:{},env:installMissing()};
const dsp=await startDisplay(64);const fx=await startFixtureHub({bin:hubBin,root:resolve('out/e2e-run/input'),port:3813,threads:6});
try{
  const sf=out+'/hold.script.json';writeFileSync(sf,JSON.stringify([{select:0},{wait:120000}]));
  const metrics=[];let buf='';const g=spawn(gpuBin,['--url',fx.state.url,'--token-file',fx.state.root+'/home/token','--script',sf],{env:{...dsp.env,HOME:fx.state.root+'/fakehome',BACKPLANE_GPUI_RECTS:'1'},stdio:['ignore','ignore','pipe']});
  g.stderr.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);const k=l.indexOf('GPUI_METRIC ');if(k>=0)try{metrics.push(JSON.parse(l.slice(k+12)))}catch{}}});
  await sleep(6000);const inp=new Input(dsp.display,out);const win=inp.win('backplane');inp.xi.focus(win.id);await sleep(500);
  summary.window=win;summary.gpu=metrics.find(m=>m.event==='gpu')||null;
  // regions the client reports (G3 layout rects, window-relative physical px); the composer is the click/paste/drop/IME target
  const lay=[...metrics].reverse().find(m=>m.event==='layout');const flat={};const walk=(p,v)=>{if(v&&typeof v==='object'&&'x' in v&&'w' in v)flat[p]=v;else if(v&&typeof v==='object')for(const[k,x]of Object.entries(v))walk(p?p+'/'+k:k,x)};walk('',lay?.rects||{});
  summary.regions=Object.fromEntries(Object.entries(flat).map(([k,v])=>[k,[v.x,v.y,v.w,v.h].map(Math.round)]));
  const mid=r=>[r.x+r.w/2,r.y+r.h/2];const comp=flat.composer?mid(flat.composer):null;
  const main=Object.entries(flat).filter(([k])=>!/sidebar|rail|composer/.test(k)).sort((a,b)=>b[1].w*b[1].h-a[1].w*a[1].h)[0];
  summary.mainRegion=main?main[0]:null;
  const run=async(name,f)=>{try{summary.actions[name]=await f()}catch(e){summary.actions[name]={ok:false,evidence:{error:String(e.message).slice(0,200)}}}};
  if(main){const r=main[1];await run('drag-select',()=>inp.dragSelect(win,[r.x+40,r.y+60],[r.x+Math.min(r.w-40,500),r.y+110]));await run('ctrl-c',()=>inp.copy())}
  else summary.blocked['drag-select/ctrl-c']='no main region in the layout event';
  execFileSync('convert',['-size','240x160','gradient:red-blue',out+'/paste.png'],{env:dsp.env});
  if(comp){await run('paste-image',()=>inp.pasteImage(out+'/paste.png',win,comp));
    await run('drop-file',()=>inp.dropFile(win,resolve(out+'/paste.png'),comp));
    await run('ime-hangul',()=>inp.ime(dsp.display,comp,win))}
  else summary.blocked['paste/drop/ime']='no composer region in the layout event';
  // the paperclip and links need their rectangles: the controls event has none yet (request G6)
  const ctl=[...metrics].reverse().find(m=>m.event==='controls');const attach=ctl?.controls?.find(c=>/attach/i.test(c.label+c.id));
  summary.blocked['paperclip']=attach&&attach.rect?null:'controls event has no rect (G6): cannot aim a click at the paperclip';
  summary.blocked['link-click']='no link rectangles or link events (G6)';
  if(attach?.rect){const r=attach.rect;await run('paperclip',async()=>{inp.s('before-paperclip');inp.clickAt(win,[r.x+r.w/2,r.y+r.h/2]);await sleep(800);return {ok:true,evidence:{shot:inp.s('after-paperclip')}}});delete summary.blocked.paperclip}
  try{g.kill('SIGTERM')}catch{}
}finally{await fx.stop();dsp.stop()}
// an action only counts when evidence says so; pass also needs the Rust hub on real hardware (CONTRACT.md), which this Boat run is not
for(const a of Object.values(summary.actions))a.status=a.ok?'partial':'fail';
writeFileSync(out+'/input-summary.json',JSON.stringify(summary,null,1));console.log(JSON.stringify({actions:Object.fromEntries(Object.entries(summary.actions).map(([k,v])=>[k,v.ok])),blocked:summary.blocked},null,1));
