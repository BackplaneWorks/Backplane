// Clipboard and PRIMARY selection helper (xclip). The owner process stays alive in the background so a client can request the data.
import {spawn,execFileSync} from 'node:child_process';import {readFileSync} from 'node:fs';
export function clip(display){
  const env={...process.env,DISPLAY:display};const owners=[];
  const own=(args,input)=>{const p=spawn('xclip',args,{env,stdio:['pipe','ignore','ignore']});if(input!==undefined)p.stdin.end(input);owners.push(p);return p};
  const read=(sel,target)=>{try{return execFileSync('xclip',['-selection',sel,'-o',...(target?['-t',target]:[])],{env,encoding:target&&target.startsWith('image/')?'buffer':'utf8',timeout:5000})}catch{return null}};
  return {
    setText(t,sel='clipboard'){own(['-selection',sel,'-i'],t)},
    setImage(png,sel='clipboard'){own(['-selection',sel,'-t','image/png','-i',png])},
    setFiles(paths){own(['-selection','clipboard','-t','text/uri-list','-i'],paths.map(p=>'file://'+p).join('\r\n')+'\r\n')},
    getText(sel='clipboard'){return read(sel,null)},
    getImage(sel='clipboard'){return read(sel,'image/png')},
    targets(sel='clipboard'){const t=read(sel,'TARGETS');return t?String(t).split('\n').filter(Boolean):[]},
    primary(){return read('primary',null)},          // what a drag-select leaves (the X11 convention)
    // xclip forks into the background to serve a selection, so killing the process we hold does not release it: take ownership with empty data instead
    // stdio 'ignore': xclip's forked child must not hold our pipe open
    clear(){for(const p of owners)try{p.kill()}catch{}owners.length=0;for(const sel of['clipboard','primary','secondary'])try{execFileSync('xclip',['-selection',sel,'-i','/dev/null'],{env,timeout:3000,stdio:'ignore'})}catch{}},
  };
}
