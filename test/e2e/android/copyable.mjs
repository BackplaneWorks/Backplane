// Copyable on Android, end to end (real input on the emulator: adb taps, UI Automator dumps). A stand-in hub on a temp home, stand-in agents only.
//   bun test/e2e/android/copyable.mjs        (EMU_PORT: this worktree's emulator, scripts/android-emu.sh; HUB_BIN, HUB_PORT, OUT)
// Checks: a tap on a copy button puts the exact text on the clipboard (read back by pasting it into the chat's composer) in a project thread, a bot's
// chat, the uberbot's chat and a room; Run in terminal opens the chat's terminal with the command typed at the prompt and not run (read back from the
// hub's screen of that shell: the command once, its output never), in the uberbot's chat and in a project thread. Screenshots go to OUT.
// Safety: only this emulator (emulator-$EMU_PORT), never an attached phone; the hub runs in a temp home with its own HOME.
import {execFileSync} from 'node:child_process';
import {mkdirSync,writeFileSync,readFileSync,rmSync,chmodSync} from 'node:fs';
import {startHub,stopHub,client,sleep} from '../../perf/lib.mjs';

const ROOT=new URL('../../..',import.meta.url).pathname.replace(/\/$/,'');
const EMU=+(process.env.EMU_PORT||5578), SERIAL=`emulator-${EMU}`, PKG=process.env.E2E_ANDROID_PKG||'dev.backplane.mobile.gpui', SCHEME='bpgpui';
const HUB_PORT=+(process.env.HUB_PORT||3897), BIN=process.env.HUB_BIN||`${ROOT}/native/host/target/fast/backplane-hub`;
const APK=process.env.APK||`${ROOT}/mobile/android/app/build/outputs/apk/debug/app-debug.apk`;
const OUT=process.env.OUT||`${ROOT}/build/gpui/results/android/copyable`;
const TMP=`${process.env.BP_TMP||process.env.HOME+'/.cache/bp-tmp'}/copyable-android-${process.pid}`;
const ADB=`${process.env.ANDROID_HOME||process.env.HOME+'/Android-sdk-writable'}/platform-tools/adb`;
const adb=(...a)=>execFileSync(ADB,['-s',SERIAL,...a],{encoding:'utf8',timeout:120000,maxBuffer:1<<26});
const sh=c=>adb('shell',c);
const results=[];const check=(name,ok,detail='')=>{results.push({name,ok:!!ok,detail});console.log(`${ok?'PASS':'FAIL'} ${name}${detail?' — '+detail:''}`)};

// ---- the screen
const un=s=>s.replace(/&amp;/g,'&').replace(/&lt;/g,'<').replace(/&gt;/g,'>').replace(/&quot;/g,'"').replace(/&#10;/g,'\n').replace(/&apos;/g,"'");
function ui(){sh('uiautomator dump /sdcard/cp.xml >/dev/null 2>&1');const x=sh('cat /sdcard/cp.xml');const out=[];
  for(const m of x.matchAll(/<node [^>]*>/g)){const a=k=>un(new RegExp(` ${k}="([^"]*)"`).exec(m[0])?.[1]??'');const b=/\[(\d+),(\d+)\]\[(\d+),(\d+)\]/.exec(a('bounds'));if(!b)continue;
    out.push({text:a('text'),desc:a('content-desc'),cls:a('class'),x:+b[1],y:+b[2],w:+b[3]-+b[1],h:+b[4]-+b[2]})}return out}
const label=n=>n.text||n.desc;
const mid=n=>[Math.round(n.x+n.w/2),Math.round(n.y+n.h/2)];
const tap=n=>sh(`input tap ${mid(n).join(' ')}`);
async function waitFor(f,ms=20000){const end=Date.now()+ms;for(;;){const ns=ui();const n=f(ns);if(n)return n;if(Date.now()>end)return null;await sleep(500)}}
const byText=re=>ns=>ns.find(n=>re.test(label(n))&&n.w>0&&n.h>0);
async function tapText(re,ms=20000){const n=await waitFor(byText(re),ms);if(!n)throw Error('not on screen: '+re);tap(n);await sleep(700);return n}
const swipeUp=()=>{const s=/(\d+)x(\d+)/.exec(sh('wm size'));const w=+s[1],h=+s[2];sh(`input swipe ${w/2|0} ${h*0.7|0} ${w/2|0} ${h*0.3|0} 300`)};
const back=()=>sh('input keyevent KEYCODE_BACK');
const shot=name=>{mkdirSync(OUT,{recursive:true});const p=`${OUT}/${name}.png`;writeFileSync(p,execFileSync(ADB,['-s',SERIAL,'exec-out','screencap','-p'],{maxBuffer:1<<27}));console.log('shot '+p);return p};
// the copy button on the row of `near` (the nearest one at or below the text's top, within a few rows)
const nearest=(ns,near,desc)=>ns.filter(n=>n.desc===desc&&n.w>0&&n.y+n.h>=near.y-4).sort((a,b)=>Math.abs(a.y-near.y)-Math.abs(b.y-near.y))[0];
// what the clipboard holds: pasted into the chat's composer (the platform's Paste key), read from the dump, then the composer emptied
async function clipboardVia(composerRe){
  const c=(await waitFor(ns=>ns.filter(n=>/EditText/.test(n.cls)).sort((a,b)=>b.y-a.y)[0],8000));if(!c)throw Error('no composer');
  // its right end: Android's clipboard preview sits over the bottom left corner after a copy
  sh(`input tap ${c.x+c.w-30} ${Math.round(c.y+c.h/2)}`);await sleep(500);sh('input keyevent KEYCODE_PASTE');await sleep(900);
  const now=ui().filter(n=>/EditText/.test(n.cls)).sort((a,b)=>b.y-a.y)[0];const got=now?.text??'';
  sh('input keycombination KEYCODE_CTRL_LEFT KEYCODE_A');sh('input keyevent KEYCODE_DEL');sh('input keyevent KEYCODE_BACK');await sleep(400);
  return got}

/** Opens the chat row `re` on the home list, through its project row `group` when the row is not shown yet (scrolling the list a few times). */
async function openChat(re,group){
  for(let i=0;i<6;i++){const ns=ui();const n=ns.find(n=>re.test(label(n))&&n.w>0);if(n){tap(n);await sleep(900);return}
    const g=group&&ns.find(n=>group.test(label(n))&&n.w>0);if(g&&i===0){tap(g);await sleep(900);continue}
    // a finished thread sits in the project's Done shelf, folded
    const done=ns.find(n=>/^DONE \(\d+\)$/.test(label(n)));if(done&&i<3){tap(done);await sleep(900);continue}
    swipeUp();await sleep(600)}
  throw Error('not on the home list: '+re)}
/** A code block's bar button (`desc`) for the block whose text is `blk`: just above it. */
const blockBar=(ns,blk,desc)=>ns.filter(n=>n.desc===desc&&n.y<blk.y&&n.y>blk.y-220).sort((a,b)=>b.y-a.y||b.w-a.w)[0];
const blockRun=(ns,blk)=>blk&&blockBar(ns,blk,'Run in terminal');
/** Back to the home list (Back until it shows; the app again if Back left it). */
async function goHome(){for(let i=0;i<5;i++){const ns=ui();if(ns.some(n=>label(n)==='Add project'))return;if(!ns.some(n=>/^Back$/.test(n.desc))&&!ns.some(n=>label(n)==='Settings'&&n.y<400)){sh(`monkey -p ${PKG} -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1`);await sleep(1500);continue}back();await sleep(900)}}
async function main(){
  rmSync(TMP,{recursive:true,force:true});for(const d of ['bin','proj','userhome'])mkdirSync(`${TMP}/${d}`,{recursive:true});
  const bun=execFileSync('sh',['-c','command -v bun'],{encoding:'utf8'}).trim();
  writeFileSync(`${TMP}/bin/claude`,`#!/bin/sh\nexec ${bun} ${ROOT}/test/tools/claude_standin.ts "$@"\n`);chmodSync(`${TMP}/bin/claude`,0o755);
  for(const n of ['codex','grok','muse']){writeFileSync(`${TMP}/bin/${n}`,'#!/bin/sh\nexit 1\n');chmodSync(`${TMP}/bin/${n}`,0o755)}
  writeFileSync(`${TMP}/proj/README.md`,'copyable fixture\n');execFileSync('sh',['-c','git init -q -b main && git add -A && git -c user.email=r@x -c user.name=rig commit -qm init'],{cwd:`${TMP}/proj`});
  const frames=new Map();
  const h=await startHub({bin:BIN,home:`${TMP}/home`,port:HUB_PORT,standins:`${TMP}/bin`,log:`${TMP}/hub.log`,extraEnv:{HOME:`${TMP}/userhome`,SHELL:'/bin/bash',BP_STANDIN_MODE:'plain'}});
  const c=await client(h,{onMsg:o=>{if(o.t==='term'&&Array.isArray(o.b)){if(o.reset)frames.set(o.thread,[]);(frames.get(o.thread)??frames.set(o.thread,[]).get(o.thread)).push(...o.b)}}});
  try{
    const call=async(m,p)=>{const r=await c.rpc(m,p,15000);if(!r?.o?.ok)throw Error(`${m}: ${r?.o?.error||'no reply'}`);return r.o};
    const ev=k=>readFileSync(`${TMP}/home/events.jsonl`,'utf8').split('\n').filter(l=>l.includes(`"$":"${k}"`)).map(l=>JSON.parse(l));
    await call('project.add',{path:`${TMP}/proj`});await sleep(800);
    const project=ev('ProjectCreated').pop().id;
    const thread=(await call('thread.create',{project,title:'copy thread',env:'local'})).thread;
    const bub=(await call('bots.create',{name:'Bub',uber:true})).bot, mochi=(await call('bots.create',{name:'Mochi'})).bot;
    await call('bots.room',{room:'',name:'den',members:mochi});await sleep(800);
    const room=ev('RoomSet').filter(e=>e.name==='den').pop().id;
    const chatOf=b=>ev('BotSet').filter(e=>e.id===b&&e.thread).pop()?.thread;
    await call('setting.set',{key:'onboard.done',value:'1'});
    // the replies, Markdown as an agent writes it
    const T={thread:'echo copyable-thread',uber:'echo copyable-uber',bot:'ls -la /tmp',room:'echo copyable-room'};
    c.send('turn.start',{thread,text:`echo:Check it with \`git status --short\`, then:\\n\`\`\`bash\\n${T.thread}\\n\`\`\`:echo`});
    await call('bots.say',{bot:bub,text:`echo:Then type:\\n\`\`\`bash\\n${T.uber}\\n\`\`\`:echo`});
    await call('bots.say',{bot:mochi,text:`echo:Try \`${T.bot}\` first.:echo`});
    const said=t=>ev('MessagePosted').some(e=>e.role==='assistant'&&(e.text||'').includes(t));
    const t0=Date.now();while(!(said(T.thread)&&said(T.uber)&&said(T.bot))&&Date.now()-t0<60000)await sleep(500);
    check('the stand-in replies are in',said(T.thread)&&said(T.uber)&&said(T.bot));
    // the room's post after Mochi's reply (a room post reaches Mochi as a message that would cut its turn short)
    await call('bots.post',{room,text:`please run \`${T.room}\` here`});await sleep(3000);
    const uberChat=chatOf(bub), mochiChat=chatOf(mochi);

    // ---- the phone: installed, paired with this hub (the emulator reaches the host's loopback at 10.0.2.2)
    adb('install','-r',APK);sh(`pm clear ${PKG}`);try{sh(`pm grant ${PKG} android.permission.POST_NOTIFICATIONS`)}catch{}
    const inv=await (await fetch(`${h.url}/auth/invitations`,{method:'POST',headers:{authorization:'Bearer '+h.token}})).json();
    const link=inv.link.replace(new URL(inv.link).host,`10.0.2.2:${HUB_PORT}`);
    sh(`am start -W -a android.intent.action.VIEW -d '${SCHEME}://pair?url=${encodeURIComponent(link)}' ${PKG}`);
    // the app asks before it pairs with a hub a link named
    const ask=await waitFor(byText(/Pair with this hub\?/),20000);if(ask){const ps=ui().filter(n=>/^Pair$/.test(label(n))).sort((a,b)=>b.y-a.y);if(ps[0])tap(ps[0]);await sleep(1500)}
    await waitFor(byText(/Bub|copy thread|Projects|Skip/),40000);const sk=ui().find(n=>/^Skip$/.test(label(n)));if(sk){tap(sk);await sleep(800)}

    // a project thread: Copy on the inline command, then the block's Run in terminal
    // a fresh pairing opens the uberbot's chat: back to the list
    await sleep(2000);if(ui().some(n=>n.desc==='Back'&&n.y<300)){back();await sleep(1200)}
    console.log('home: '+ui().map(label).filter(Boolean).join(' | '));
    await openChat(/^copy thread$/,/^proj$/);
    const span=await waitFor(byText(/git status --short/),20000);check('thread: the reply is shown',span);
    let ns=ui();const inl=span&&nearest(ns,span,'Copy');check('thread: the inline command has a copy button',inl);
    if(inl){tap(inl);shot('thread-copied');const d0=Date.now();const flash=ui().some(n=>n.desc==='Copied');const took=Date.now()-d0;
      if(flash||took<1300)check('thread: the button says Copied',flash,`dump ${took} ms`);else console.log(`SKIP thread: the button says Copied — the UI dump took ${took} ms, longer than the 1.5 s it shows (see thread-copied.png)`);
      check('thread: Copy puts the exact command on the clipboard',(await clipboardVia())==='git status --short');}
    const blk=await waitFor(byText(new RegExp('^'+T.thread+'$')),5000);ns=ui();
    const run=blockRun(ns,blk);check('thread: the command block has Run in terminal',run);
    const shotReply=shot('thread-reply');
    if(run){tap(run);await sleep(4000);shot('thread-terminal');
      c.send('term.snap',{key:thread});await sleep(1500);
      const scr=String.fromCharCode(...(frames.get(thread)??[])).replace(/\x1b\[[0-9;?]*[ -\/]*[@-~]|\x1b[()][0-9A-B]|\x1b[=>]/g,'');
      check('thread: the terminal has the command typed in',scr.includes(T.thread),JSON.stringify(scr.slice(-160)));
      check('thread: and not run (no output)',scr.split('copyable-thread').length===2);
      back();await sleep(800)}
    await goHome();

    // the uberbot's chat: Run in terminal on its block (its shell works in the home folder)
    await openChat(/^Bub$/);
    const ub=await waitFor(byText(new RegExp('^'+T.uber+'$')),20000);check('uberbot: the reply is shown',ub);
    ns=ui();const ucopy=ub&&blockBar(ns,ub,'Copy');
    if(ucopy){tap(ucopy);await sleep(300);check('uberbot: Copy puts the whole block on the clipboard',(await clipboardVia())===T.uber)}
    const urun=ub&&blockRun(ui(),ub);check('uberbot: the block has Run in terminal',urun);
    if(urun){shot('uberbot-reply');tap(urun);await sleep(4000);shot('uberbot-terminal');
      c.send('term.snap',{key:uberChat});await sleep(1500);
      const scr=String.fromCharCode(...(frames.get(uberChat)??[])).replace(/\x1b\[[0-9;?]*[ -\/]*[@-~]|\x1b[()][0-9A-B]|\x1b[=>]/g,'');
      check('uberbot: the terminal is its chat\'s, with the command typed in',scr.includes(T.uber),JSON.stringify(scr.slice(-160)));
      check('uberbot: and not run',scr.split('copyable-uber').length===2);
      back();await sleep(800)}
    await goHome();

    // a bot's chat: Copy on an inline command
    await openChat(/^Mochi$/);
    const bs=await waitFor(byText(/ls -la \/tmp/),20000);check('bot chat: the reply is shown',bs);
    const bcopy=bs&&nearest(ui(),bs,'Copy');check('bot chat: the inline command has a copy button',bcopy);
    if(bcopy){tap(bcopy);await sleep(300);check('bot chat: Copy puts the exact command on the clipboard',(await clipboardVia())===T.bot)}
    await goHome();

    // a room: the post's piece
    await openChat(/^#?den$/);
    const rp=await waitFor(byText(new RegExp(T.room+'$')),20000);check('room: the post\'s command is shown to copy',rp);
    const rcopy=rp&&nearest(ui(),rp,'Copy');check('room: it has a copy button',rcopy);
    check('room: and Run in terminal',rp&&nearest(ui(),rp,'Run in terminal'));
    if(rcopy){shot('room');tap(rcopy);await sleep(300);const got=await clipboardVia();check('room: Copy puts the exact command on the clipboard',got===T.room,JSON.stringify(got))}
    back();
    console.log('reply screenshot: '+shotReply);
  }finally{try{c.ws.close()}catch{}stopHub(h);await sleep(1000);if(!process.env.KEEP)rmSync(TMP,{recursive:true,force:true})}
  mkdirSync(OUT,{recursive:true});writeFileSync(`${OUT}/results.json`,JSON.stringify(results,null,1));
  const bad=results.filter(r=>!r.ok);console.log(`${results.length-bad.length}/${results.length} passed`);process.exit(bad.length?1:0);
}
main().catch(e=>{console.error('FAILED',e.message);try{shot('failed');console.error(ui().map(n=>`${label(n)}@${n.x},${n.y}`).filter(x=>!x.startsWith('@')).join(' | '))}catch{}process.exit(2)});
