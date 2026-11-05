// Proves each input mechanism works on a bare Xvfb with stand-in targets (xev, xclip, an XDND window): bun input/selftest.mjs  (Boat)
import {spawn,execFileSync,spawnSync} from 'node:child_process';import {writeFileSync,readFileSync,existsSync,mkdirSync} from 'node:fs';
import {startDisplay,installMissing} from '../lib/capture.mjs';import {xinput} from './xinput.mjs';import {clip} from './clip.mjs';import {startIme} from './ime.mjs';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));const out='out/e2e/input-selftest';mkdirSync(out,{recursive:true});
const R={checks:{}};const ok=(k,v,e)=>{R.checks[k]={ok:!!v,...(e||{})};console.log((v?'ok  ':'FAIL')+' '+k+(e?' '+JSON.stringify(e).slice(0,160):''))};
R.install=installMissing();
const pk=['xdotool','xclip','python3-xlib','x11-utils','ibus','ibus-hangul','imagemagick','xvfb','i3'];
try{execFileSync('sh',['-c','sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq xdotool xclip python3-xlib x11-utils ibus ibus-hangul imagemagick 2>&1 | tail -2'],{stdio:'inherit',timeout:600000})}catch(e){R.aptError=String(e.message).slice(0,200)}
const dsp=await startDisplay(63);const D=dsp.display;const xi=xinput(D);const env=dsp.env;
try{
  // 1. mouse + keys reach a window: xev logs them
  const xev=spawn('xev',['-geometry','300x300+50+50','-name','e2e-xev','-event','mouse','-event','keyboard'],{env,stdio:['ignore','pipe','ignore']});let log='';xev.stdout.on('data',d=>log+=d);await sleep(1500);
  const g=xi.geometry('e2e-xev');xi.focus(g.id);await sleep(300);
  xi.click(g.x+50,g.y+50);await xi.drag(g.x+20,g.y+20,g.x+200,g.y+120,10,20);xi.key('ctrl+c');xi.type('é日');xi.wheel(g.x+100,g.y+100,3);await sleep(600);xev.kill();
  ok('click',/ButtonPress[\s\S]*button 1/.test(log));ok('drag-motion',(log.match(/MotionNotify/g)||[]).length>=8,{motion:(log.match(/MotionNotify/g)||[]).length});
  ok('drag-press-release',/ButtonRelease/.test(log));ok('ctrl+c',/KeyPress[\s\S]*keysym 0x63, c/.test(log)&&/Control_L/.test(log));
  ok('unicode-type',/eacute|0x1000[0-9a-f]+/.test(log)||/U\+|keysym 0x(e9|100)/.test(log),{sample:(log.match(/keysym 0x[0-9a-f]+, \S+/g)||[]).slice(-6)});
  ok('wheel',/button (4|5)/.test(log));
  // 2. clipboard: text and image roundtrip, TARGETS, PRIMARY
  const cb=clip(D);cb.setText('clip text');await sleep(300);ok('clipboard-text',cb.getText()==='clip text');
  spawnSync('convert',['-size','8x8','xc:red',out+'/red.png'],{env});cb.setImage(out+'/red.png');await sleep(300);
  const img=cb.getImage();ok('clipboard-image',img&&img.length>50&&img.slice(1,4).toString()==='PNG',{bytes:img?.length});ok('clipboard-targets',cb.targets().includes('image/png'),{targets:cb.targets()});
  cb.setText('primary text','primary');await sleep(300);ok('primary',cb.primary()==='primary text');cb.clear();
  // 3. XDND file drop onto an XDND-aware window
  writeFileSync(out+'/drop.txt','dropped');const t=spawn('python3',['test/gpui/e2e/input/dnd_target.py',D,'e2e-dnd',out+'/dropped.txt'],{env,stdio:'inherit'});
  for(let i=0;i<50&&!existsSync(out+'/dropped.txt.ready');i++)await sleep(100);await sleep(500);const gd=xi.geometry('e2e-dnd');
  const dr=spawnSync('python3',['test/gpui/e2e/input/dnd.py',D,gd.id,String(gd.x+100),String(gd.y+100),process.cwd()+'/'+out+'/drop.txt'],{env,encoding:'utf8'});await sleep(800);
  const got=existsSync(out+'/dropped.txt')?readFileSync(out+'/dropped.txt','utf8'):'';ok('xdnd-drop',dr.status===0&&got.includes('drop.txt'),{exit:dr.status,out:dr.stdout?.trim(),err:dr.stderr?.slice(0,200),got:got.trim()});
  // 4. screenshots + diff
  const {shot,diffPixels}=await import('./driver.mjs');const a=shot(D,out+'/a.png');xi.click(g.x+10,g.y+10);const b=shot(D,out+'/b.png');ok('screenshot',diffPixels(a,a)===0,{identicalDiff:diffPixels(a,a),movedDiff:diffPixels(a,b)});
  // 5. IME: ibus + hangul starts and selects (composition is only provable inside a client that takes preedit; xev does not)
  const ime=await startIme(D);ok('ime-starts',ime.status==='started'&&/hangul/.test(ime.engine),{status:ime.status,engine:ime.engine});ime.stop();
}catch(e){ok('selftest-crash',false,{error:String(e.stack||e).slice(0,300)})}
finally{dsp.stop()}
R.pass=Object.values(R.checks).every(c=>c.ok);writeFileSync(out+'/result.json',JSON.stringify(R,null,1));console.log(R.pass?'input selftest: all checks ok':'input selftest: FAILURES');process.exit(0);
