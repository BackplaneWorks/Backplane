// A real input method on Xvfb: ibus with the Hangul engine (composition without any network: 'g' 'k' 's' -> preedit ㅎ, 하, 한, committed with space).
// A client sees it through XIM (XMODIFIERS=@im=ibus) or GTK/QT modules; whether GPUI takes preedit from it is what the IME journey measures.
import {spawn,execFileSync} from 'node:child_process';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
export async function startIme(display,engine='hangul'){
  // default ibus address handling (the selftest's way, which works): the daemon writes its address under ~/.config/ibus/bus and the CLI reads it
  const env={...process.env,DISPLAY:display,XMODIFIERS:'@im=ibus',GTK_IM_MODULE:'ibus',QT_IM_MODULE:'ibus',LANG:'en_US.UTF-8'};
  let log='';const p=spawn('ibus-daemon',['-r','-x','--panel','disable','-v'],{env,stdio:['ignore','pipe','pipe']});p.stdout.on('data',d=>log+=d);p.stderr.on('data',d=>log+=d);
  let up=false;for(let k=0;k<30&&!up;k++){await sleep(500);try{execFileSync('ibus',['address'],{env,timeout:3000,stdio:'pipe'});up=true}catch{}}
  let status='started',engineNow='';
  try{execFileSync('ibus',['engine',engine],{env,timeout:8000,stdio:'pipe'});engineNow=execFileSync('ibus',['engine'],{env,encoding:'utf8',timeout:8000}).trim()}catch(e){status='engine not selectable (daemon answered: '+up+'): '+String(e.stderr||e.message).slice(0,160)+' | daemon log: '+log.slice(-300)}
  return {env,engine:engineNow,status,stop:()=>{try{p.kill('SIGTERM')}catch{}}};
}
// keys that compose '한' on the Hangul 2-set layout, then space to commit
export const HANGUL_HAN=['g','k','s','space'];
