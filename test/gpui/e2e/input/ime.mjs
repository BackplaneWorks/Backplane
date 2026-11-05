// A real input method on Xvfb: ibus with the Hangul engine (composition without any network: 'g' 'k' 's' -> preedit ㅎ, 하, 한, committed with space).
// A client sees it through XIM (XMODIFIERS=@im=ibus) or GTK/QT modules; whether GPUI takes preedit from it is what the IME journey measures.
import {spawn,execFileSync} from 'node:child_process';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
export async function startIme(display,engine='hangul'){
  const addr='unix:path=/tmp/e2e-ibus-'+display.replace(':','');
  const env={...process.env,DISPLAY:display,XMODIFIERS:'@im=ibus',GTK_IM_MODULE:'ibus',QT_IM_MODULE:'ibus',LANG:'en_US.UTF-8',IBUS_ADDRESS:addr}; // the CLI and the client must find this daemon's address
  const p=spawn('ibus-daemon',['-r','-x','--panel','disable','--address',addr],{env,stdio:'ignore'});await sleep(3000);
  let status='started',engineNow='';
  try{execFileSync('ibus',['engine',engine],{env,timeout:8000});engineNow=execFileSync('ibus',['engine'],{env,encoding:'utf8',timeout:8000}).trim()}catch(e){status='engine not selectable: '+String(e.message).slice(0,160)}
  return {env,engine:engineNow,status,stop:()=>{try{p.kill('SIGTERM')}catch{}}};
}
// keys that compose '한' on the Hangul 2-set layout, then space to commit
export const HANGUL_HAN=['g','k','s','space'];
