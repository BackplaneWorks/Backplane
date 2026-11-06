// GPUI input driver: the OS-level actions the must-pass list needs (drag-select, Ctrl-C, paste an image, drop a file, IME, paperclip, link click).
// Each action returns {ok, evidence} from things outside the client (clipboard, screenshots, hub events); it never asks the client whether it worked.
import {execFileSync,spawn} from 'node:child_process';import {mkdirSync,existsSync,readFileSync,writeFileSync} from 'node:fs';import {dirname,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {xinput} from './xinput.mjs';import {clip} from './clip.mjs';import {startIme,HANGUL_HAN} from './ime.mjs';
const here=dirname(fileURLToPath(import.meta.url));const sleep=ms=>new Promise(r=>setTimeout(r,ms));
export function shot(display,file){mkdirSync(dirname(file),{recursive:true});execFileSync('import',['-window','root',file],{env:{...process.env,DISPLAY:display}});return file}
// pixels that differ between two screenshots (ImageMagick compare, absolute error count); small regions: pass a crop like '300x200+100+100'
export function diffPixels(a,b,crop){const args=crop?[a+'['+crop+']',b+'['+crop+']']:[a,b];
  const r=execFileSync('sh',['-c',`compare -metric AE ${args.map(x=>`'${x}'`).join(' ')} null: 2>&1 || true`],{encoding:'utf8'});return Number(r.trim().split(/\s+/)[0])||0}
export class Input{
  constructor(display,out){this.display=display;this.out=out;this.xi=xinput(display);this.cb=clip(display);mkdirSync(out,{recursive:true});this.n=0}
  s(name){return shot(this.display,`${this.out}/${String(++this.n).padStart(2,'0')}-${name}.png`)}
  win(name='backplane'){return this.xi.geometry(name)}
  winClass(cls='backplane'){return this.xi.geometryByClass(cls)}
  // text drag-select between two window-relative points; the X11 convention puts the selection in PRIMARY
  async dragSelect(g,from,to){const before=this.s('before-drag');await this.xi.drag(g.x+from[0],g.y+from[1],g.x+to[0],g.y+to[1]);await sleep(300);const after=this.s('after-drag');
    const primary=this.cb.primary();return {ok:!!primary&&primary.length>0,evidence:{primarySelection:primary?.slice(0,200)??null,changedPixels:diffPixels(before,after)}}}
  async copy(){this.cb.clear();this.xi.key('ctrl+c');await sleep(400);const t=this.cb.getText();return {ok:!!t&&t.length>0,evidence:{clipboard:t?.slice(0,200)??null,targets:this.cb.targets()}}}
  async pasteImage(png,g,composerAt){this.cb.setImage(png);await sleep(300);if(composerAt)this.xi.click(g.x+composerAt[0],g.y+composerAt[1]);await sleep(200);const before=this.s('before-paste');this.xi.key('ctrl+v');await sleep(1200);const after=this.s('after-paste');
    return {ok:true,evidence:{changedPixels:diffPixels(before,after),targetsOffered:this.cb.targets()}}}
  dropFile(g,file,at){let out='',code=0;try{out=execFileSync('python3',[resolve(here,'dnd.py'),this.display,g.id,String(g.x+at[0]),String(g.y+at[1]),file],{encoding:'utf8'})}catch(e){code=e.status;out=String(e.stdout||e.message)}
    return {ok:code===0,evidence:{dnd:out.trim(),exit:code}}}
  // `ime` is a running startIme() instance whose env the client was launched with (XIM: the client must start after the daemon and with XMODIFIERS=@im=ibus)
  async imeCompose(ime,composerAt,g,engine='hangul'){this.xi.click(g.x+composerAt[0],g.y+composerAt[1]);await sleep(300);let now='';try{now=ime.setEngine(engine)}catch(e){return {ok:false,evidence:{error:'engine switch failed: '+String(e.stderr||e.message).slice(0,160)}}}
    const before=this.s('before-ime');for(const k of HANGUL_HAN){this.xi.key(k);await sleep(150)}await sleep(500);const after=this.s('after-ime');try{ime.setEngine('xkb:us::eng')}catch{}
    return {ok:true,evidence:{imeStatus:ime.status,engine:now,changedPixels:diffPixels(before,after)}}}
  windows(){try{const ids=execFileSync('xdotool',['search','--onlyvisible','--name','.'],{env:{...process.env,DISPLAY:this.display},encoding:'utf8'}).trim().split('\n').filter(Boolean);return ids.map(id=>{let n='';try{n=execFileSync('xdotool',['getwindowname',id],{env:{...process.env,DISPLAY:this.display},encoding:'utf8'}).trim()}catch{}return {id,name:n}})}catch{return []}}
  clickAt(g,at,button=1){this.xi.click(g.x+at[0],g.y+at[1],button)}
}
