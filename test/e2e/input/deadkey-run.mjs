// The us-intl dead acute through ibus over XIM, on Xvfb (Boat only): dead_acute then e in the composer must give "é", and the client must live.
// ibus sends the accent's preedit as the COMPOUND_TEXT byte 0xB4 and the commit as 0xE9 (ISO 8859-1, CT's default GR); a client that reads them
// as UTF-8 panicked in zed-xim (client.rs:237, "Encoding Error: Utf8Error ... bytes: [180]").
// On a Boat box (never the desktop), with the hub, the client and its assets/ in test/e2e/bin: sh test/e2e/boat-inner.sh input-deadkey
//   (that is: bun test/e2e/input/deadkey-run.mjs --hub-bin PATH --gpui-bin PATH [--out DIR], under dbus-run-session, after provisioning ibus and setxkbmap)
// Exit 0 = pass, 1 = the app failed (no é, or the client died), 3 = harness (ibus or the layout could not be set up: says nothing about the app).
import {readFileSync,writeFileSync,mkdirSync,appendFileSync} from 'node:fs';import {spawn,execFileSync} from 'node:child_process';import {resolve} from 'node:path';
import {startDisplay,installMissing} from '../lib/capture.mjs';import {startFixtureHub} from '../lib/hubfx.mjs';import {Input} from './driver.mjs';import {startIme} from './ime.mjs';
const arg=(k,d)=>{const i=process.argv.indexOf('--'+k);return i>=0?process.argv[i+1]:d};const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const hubBin=resolve(arg('hub-bin')),gpuBin=resolve(arg('gpui-bin')),out=resolve(arg('out','out/e2e/deadkey'));mkdirSync(out,{recursive:true});
const summary={when:new Date().toISOString(),env:installMissing()};
const record=(verdict,reason)=>{summary.verdict=verdict;summary.reason=reason;writeFileSync(out+'/deadkey-summary.json',JSON.stringify(summary,null,1));console.log(`${verdict.toUpperCase()}: ${reason}`)};
const finish=(code,verdict,reason)=>{record(verdict,reason);process.exit(code)}; // before anything to clean up
class Done{constructor(code){this.code=code}};const done=(code,verdict,reason)=>{record(verdict,reason);throw new Done(code)}; // inside the run: cleaned up below
const dsp=await startDisplay(65);
// the layout first (us intl: the apostrophe key is dead_acute), then ibus with the matching xkb engine, before the client so it connects through XIM
try{execFileSync('setxkbmap',['-layout','us','-variant','intl'],{env:dsp.env})}catch(e){dsp.stop();finish(3,'harness','setxkbmap failed: '+String(e.stderr||e.message).slice(0,160))}
const ime=await startIme(dsp.display,'xkb:us:intl:eng');summary.ime={status:ime.status,engine:ime.engine,locale:ime.locale};
const deadKeycode=(()=>{try{return execFileSync('xmodmap',['-pke'],{env:dsp.env,encoding:'utf8'}).split('\n').find(l=>/\bdead_acute\b/.test(l))||null}catch{return null}})();summary.deadAcuteKey=deadKeycode;
if(ime.status!=='started'||!/intl/.test(ime.engine)||!deadKeycode){ime.stop();dsp.stop();finish(3,'harness','ibus or the us-intl layout is not set up: '+JSON.stringify(summary.ime)+' dead_acute key: '+deadKeycode)}
const fx=await startFixtureHub({bin:hubBin,root:resolve('out/e2e-run/deadkey'),port:3814,threads:2});
let g=null,gExit=null,code=3;const clientLog=out+'/client.stderr.log';writeFileSync(clientLog,'');
try{
  const sf=out+'/hold.script.json';writeFileSync(sf,JSON.stringify([{select:0},...Array.from({length:60},()=>[{wait:1000},{read:'composer'}]).flat()]));
  const metrics=[];let buf='';
  g=spawn(gpuBin,['--url',fx.state.url,'--token-file',fx.state.root+'/home/token','--script',sf],{env:{...dsp.env,...ime.env,LC_ALL:'C.UTF-8',HOME:fx.state.root+'/fakehome',BACKPLANE_GPUI_RECTS:'1'},stdio:['ignore','ignore','pipe']});
  g.on('exit',(c,s)=>{gExit={code:c,signal:s}});
  g.stderr.on('data',d=>{appendFileSync(clientLog,d);buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);const k=l.indexOf('GPUI_METRIC ');if(k>=0)try{metrics.push(JSON.parse(l.slice(k+12)))}catch{}}});
  const inp=new Input(dsp.display,out);let win=null;
  for(let k=0;k<40&&!win&&!gExit;k++){await sleep(1000);try{win=inp.win('backplane')||inp.winClass('backplane')}catch{}}
  if(!win)done(3,'harness','the client window never appeared ('+(gExit?'client exited '+JSON.stringify(gExit):'still running')+')');
  inp.xi.focus(win.id);await sleep(1500);
  const lay=[...metrics].reverse().find(m=>m.event==='layout');const c=lay?.rects?.composer;
  if(!c)done(3,'harness','no composer rect in the layout event');
  const at=[win.x+c.x+c.w/2,win.y+c.y+c.h/2];inp.xi.click(at[0],at[1]);await sleep(300);inp.xi.key('ctrl+a');inp.xi.key('BackSpace');await sleep(300);
  const t0=Math.floor(Date.now()/1000);inp.s('before-keys');
  inp.xi.key('dead_acute');await sleep(400);inp.xi.key('e');await sleep(2500);inp.s('after-keys');
  const read=[...metrics].reverse().find(m=>m.event==='read'&&m.what==='composer');summary.composerRead=read?{text:read.text,supported:read.supported}:null;
  if(!gExit){inp.xi.key('Return');await sleep(2000)}
  const evPath=fx.state.root+'/home/events.jsonl';let sent=null;
  try{sent=readFileSync(evPath,'utf8').split('\n').filter(l=>l.includes('"MessagePosted"')&&l.includes('"role":"user"')).map(l=>JSON.parse(l)).filter(e=>e.at>=t0).at(-1)||null}catch{}
  const err=readFileSync(clientLog,'utf8');const panic=(err.match(/panicked at [^\n]*\n?[^\n]*/)||[null])[0];
  summary.client={exit:gExit,alive:!gExit,panic,ximDropWarnings:(err.match(/xim: dropped[^\n]*/g)||[]).slice(0,3)};summary.hubMessageText=sent?.text??null;
  const composed=sent?.text==='é'||summary.composerRead?.text==='é';
  if(panic||gExit)done(1,'fail','the client died on dead_acute e: '+(panic||JSON.stringify(gExit)));
  if(!composed)done(1,'fail','dead_acute e did not give "é" (composer read '+JSON.stringify(summary.composerRead)+', hub got '+JSON.stringify(sent?.text??null)+')');
  done(0,'pass','dead_acute e gave "é" (hub got '+JSON.stringify(sent?.text??null)+', composer read '+JSON.stringify(summary.composerRead?.text??null)+'); the client is alive');
}catch(e){if(e instanceof Done)code=e.code;else{record('harness','the runner threw: '+String(e.stack||e).slice(0,300));code=3}}
finally{try{execFileSync('cp',[fx.state.root+'/hub.log',out+'/hub.log'])}catch{}try{g?.kill('SIGTERM')}catch{}try{ime.stop()}catch{}try{await fx.stop()}catch{}dsp.stop()}
process.exit(code);
