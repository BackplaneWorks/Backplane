// Real X11 input on an Xvfb display, through XTEST (xdotool). Boat only; never aimed at the person's display.
import {execFileSync,spawnSync} from 'node:child_process';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
export function xinput(display){
  if(!/^:\d+$/.test(display)||display===':0')throw Error('xinput: refusing display '+display+' (an isolated Xvfb display other than :0 only)');
  const env={...process.env,DISPLAY:display};
  const xd=(...a)=>execFileSync('xdotool',a,{env,encoding:'utf8'}).trim();
  return {
    display,
    geometry(name){const id=xd('search','--onlyvisible','--name',name).split('\n')[0];const g=Object.fromEntries(xd('getwindowgeometry','--shell',id).split('\n').map(l=>l.split('=')));return {id,x:+g.X,y:+g.Y,w:+g.WIDTH,h:+g.HEIGHT}},
    geometryByClass(cls){const id=xd('search','--onlyvisible','--class',cls).split('\n')[0];const g=Object.fromEntries(xd('getwindowgeometry','--shell',id).split('\n').map(l=>l.split('=')));return {id,x:+g.X,y:+g.Y,w:+g.WIDTH,h:+g.HEIGHT}},
    focus(id){xd('windowfocus','--sync',id)},
    move(x,y){xd('mousemove',String(Math.round(x)),String(Math.round(y)))},
    click(x,y,button=1){this.move(x,y);xd('click',String(button))},
    doubleClick(x,y){this.move(x,y);xd('click','--repeat','2','--delay','60','1')},
    tripleClick(x,y){this.move(x,y);xd('click','--repeat','3','--delay','60','1')},
    // press, move in steps, release: what a person's drag-select does (GPUI sees ButtonPress, MotionNotify..., ButtonRelease)
    async drag(x1,y1,x2,y2,steps=12,stepMs=16){this.move(x1,y1);xd('mousedown','1');for(let i=1;i<=steps;i++){this.move(x1+(x2-x1)*i/steps,y1+(y2-y1)*i/steps);await sleep(stepMs)}xd('mouseup','1')},
    key(combo){xd('key','--clearmodifiers',combo)},               // 'ctrl+c', 'ctrl+v', 'Return', 'Tab'
    type(text,delayMs=12){xd('type','--clearmodifiers','--delay',String(delayMs),text)}, // unicode goes through temporary keysym remaps
    scroll(clicks,dir=5){xd('click','--repeat',String(Math.abs(clicks)),'--delay','30',String(clicks<0?4:dir))},
    wheel(x,y,clicks){this.move(x,y);this.scroll(clicks)},
  };
}
export const have=c=>spawnSync('sh',['-c',`command -v ${c}`]).status===0;
