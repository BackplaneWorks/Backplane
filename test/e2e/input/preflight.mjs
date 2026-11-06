// Harness self-check: proves the pointer, keyboard, clipboard, PRIMARY, XDND and input method work against PLAIN X clients before GPUI starts.
// If any check fails the run is a HARNESS failure, never a verdict about the app. Boat or an isolated display only.
import {spawn,execFileSync,spawnSync} from 'node:child_process';import {writeFileSync,readFileSync,existsSync,mkdirSync} from 'node:fs';import {resolve} from 'node:path';
import {shot,diffPixels} from './driver.mjs';import {xinput} from './xinput.mjs';import {clip} from './clip.mjs';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
// ime: a running startIme() instance (its env is what an XIM client needs); out: folder for scratch files
export async function preflight({display,env,ime,out,portalLog=null}){
  mkdirSync(out,{recursive:true});const xi=xinput(display),cb=clip(display);const checks={};
  const ok=(k,v,detail)=>{checks[k]={ok:!!v,detail:detail??null}};
  const guard=async(k,f)=>{try{await f()}catch(e){ok(k,false,'check crashed: '+String(e.message||e).slice(0,200))}};
  // 1. pointer, keyboard, wheel reach a plain X window (xev logs them)
  await guard('pointer-keys',async()=>{
    const xev=spawn('xev',['-geometry','300x300+40+40','-name','pf-xev','-event','mouse','-event','keyboard'],{env,stdio:['ignore','pipe','ignore']});let log='';xev.stdout.on('data',d=>log+=d);await sleep(1500);
    const g=xi.geometry('pf-xev');xi.focus(g.id);await sleep(300);xi.click(g.x+60,g.y+60);await xi.drag(g.x+20,g.y+20,g.x+200,g.y+120,10,20);xi.key('ctrl+c');xi.type('é');xi.wheel(g.x+100,g.y+100,3);await sleep(600);xev.kill();
    const m=(log.match(/MotionNotify/g)||[]).length;
    ok('pointer-click',/ButtonPress[\s\S]*button 1/.test(log),null);ok('pointer-drag',m>=8&&/ButtonRelease/.test(log),{motion:m});
    ok('key-ctrl-c',/keysym 0x63, c/.test(log)&&/Control_L/.test(log),null);ok('key-unicode',/eacute/.test(log),null);ok('wheel',/button (4|5)/.test(log),null)});
  // 1b. screenshots and the pixel diff the client-pointer check relies on: identical shots differ by 0, a new window changes the picture
  await guard('screenshot-diff',async()=>{const a=shot(display,out+'/pf-a.png'),a2=shot(display,out+'/pf-a2.png');const same=diffPixels(a,a2);
    const w=spawn('xev',['-geometry','400x300+700+400','-name','pf-shot'],{env,stdio:'ignore'});await sleep(1200);const b=shot(display,out+'/pf-b.png');w.kill();const moved=diffPixels(a,b);
    ok('screenshot-diff',same===0&&moved>1000,{identicalDiff:same,newWindowDiff:moved})});
  // 2. clipboard + PRIMARY, read by a different process than the owner (what a client does)
  await guard('clipboard',async()=>{
    cb.setText('pf clip');await sleep(300);ok('clipboard-text',cb.getText()==='pf clip',{got:cb.getText()});
    spawnSync('convert',['-size','8x8','xc:red',out+'/pf.png'],{env});cb.setImage(out+'/pf.png');await sleep(300);const img=cb.getImage();
    ok('clipboard-image',img&&img.length>50&&img.slice(1,4).toString()==='PNG',{bytes:img?.length});ok('clipboard-targets',cb.targets().includes('image/png'),{targets:cb.targets()});
    cb.setText('pf primary','primary');await sleep(300);ok('primary',cb.primary()==='pf primary',{got:cb.primary()});
    cb.setFiles([resolve(out+'/pf.png')]);await sleep(300);ok('clipboard-files',cb.targets().includes('text/uri-list'),{targets:cb.targets()});cb.clear();await sleep(300);
    const left=[cb.getText('clipboard'),cb.primary()].map(x=>x==null?'':String(x).trim());ok('selections-cleared-after-preflight',left[0]===''&&left[1]==='',{clipboard:left[0].slice(0,40),primary:left[1].slice(0,40)})});
  // 3. XDND: a drop onto an XDND-aware window is accepted and the target reads the path
  await guard('xdnd',async()=>{
    writeFileSync(out+'/pf-drop.txt','x');const done=out+'/pf-dropped.txt';try{execFileSync('rm',['-f',done,done+'.ready'])}catch{}
    const t=spawn('python3',[resolve('test/e2e/input/dnd_target.py'),display,'pf-dnd',done],{env,stdio:'ignore'});
    for(let i=0;i<50&&!existsSync(done+'.ready');i++)await sleep(100);await sleep(400);const gd=xi.geometry('pf-dnd');
    const r=spawnSync('python3',[resolve('test/e2e/input/dnd.py'),display,gd.id,String(gd.x+100),String(gd.y+100),resolve(out+'/pf-drop.txt')],{env,encoding:'utf8'});await sleep(600);
    const got=existsSync(done)?readFileSync(done,'utf8'):'';ok('xdnd-drop',r.status===0&&got.includes('pf-drop.txt'),{exit:r.status,out:(r.stdout||'').trim(),err:(r.stderr||'').slice(0,160),got:got.trim()});try{t.kill()}catch{}});
  // 4. input method: the daemon answers, the engine switches both ways, an XIM client sees the server, and a plain XIM client (xev) gets 한 from g k s space
  await guard('ime',async()=>{
    if(!ime||ime.status!=='started'){ok('ime-daemon',false,ime?.status||'no ime instance');return}
    ok('ime-daemon',true,{engine:ime.engine});
    let sw='';try{sw=ime.setEngine('hangul')}catch(e){ok('ime-switch',false,String(e.stderr||e.message).slice(0,200));return}
    ok('ime-switch',/hangul/.test(sw),{engine:sw});try{ime.setEngine('xkb:us::eng')}catch{}
    let xs='';try{xs=execFileSync('xprop',['-root','XIM_SERVERS'],{env,encoding:'utf8'})}catch{}ok('xim-server-registered',/ibus|fcitx/.test(xs),{xprop:xs.trim().slice(0,120)});
    const xev=spawn('xev',['-geometry','300x300+380+40','-name','pf-xim','-event','keyboard'],{env:{...env,...ime.env},stdio:['ignore','pipe','ignore']});let log='';xev.stdout.on('data',d=>log+=d);await sleep(1800);
    const g=xi.geometry('pf-xim');xi.focus(g.id);await sleep(500);xi.click(g.x+50,g.y+50);await sleep(400);
    // ibus keeps an engine per input context: switch AFTER the client's context has focus (a new context starts on the default engine)
    try{ime.setEngine('hangul')}catch{}await sleep(500);for(const k of['g','k','s','space']){xi.key(k);await sleep(250)}await sleep(800);xev.kill();writeFileSync(out+'/xim-xev.log',log);
    let diag='';try{diag=(ime.backend==='fcitx5'?['backend fcitx5','engine now: '+spawnSync('fcitx5-remote',['-n'],{env:ime.env,encoding:'utf8'}).stdout.trim(),'procs: '+spawnSync('sh',['-c','pgrep -a fcitx | cut -c1-100'],{encoding:'utf8'}).stdout.trim().split('\n').join(' ; '),'hangul addon: '+spawnSync('sh',['-c','ls /usr/share/fcitx5/addon /usr/share/fcitx5/inputmethod 2>/dev/null | grep -i -c hangul'],{encoding:'utf8'}).stdout.trim()]:['backend ibus','engine now: '+execFileSync('ibus',['engine'],{env:ime.env,encoding:'utf8'}).trim()]).join(' || ')}catch(e){diag='diag failed '+e.message}
    writeFileSync(out+'/ime-diag.txt',diag+'\n--- daemon log tail ---\n'+ime.logTail(3000));
    const filtered=(log.match(/XFilterEvent returns: True/g)||[]).length;const han=/ed 95 9c|한/.test(log);
    ok('ime-composes-in-plain-xim-client',han,{diag:diag.slice(0,400),locale:ime.locale,locales:ime.locales,filteredKeys:filtered,sawHan:han,tail:log.split('\n').filter(l=>/Lookup|keysym|XFilter/.test(l)).slice(-14).join(' | ').slice(0,700)});
    try{ime.setEngine('xkb:us::eng')}catch{}});
  // 5. the file-chooser portal stand-in is up (paperclip)
  if(portalLog)await guard('portal',async()=>{ok('portal-standin',existsSync(portalLog+'.ready'),{log:portalLog})});
  const failed=Object.entries(checks).filter(([,v])=>!v.ok).map(([k])=>k);
  return {pass:failed.length===0,failed,checks};
}
