// Xvfb + i3 + screen capture (Boat). Software rendering there: frame data from it validates the harness, it is not GPU evidence.
import {spawn,execSync} from 'node:child_process';import {mkdirSync,existsSync,readdirSync} from 'node:fs';
import {sleep} from '../../perf/lib.mjs';
const have=c=>{try{execSync(`command -v ${c}`,{stdio:'ignore'});return true}catch{return false}};
export function tools(){return {Xvfb:have('Xvfb'),i3:have('i3'),ffmpeg:have('ffmpeg'),import:have('import'),xdotool:have('xdotool'),vulkaninfo:have('vulkaninfo')}}
export function installMissing(){
  if(!process.env.BOAT_SANDBOX)return {installed:false,tools:tools(),note:'apt only inside a Boat sandbox'}; // best effort on a disposable Boat sandbox (root there); never on the desktop
  const t=tools();if(t.Xvfb&&t.i3&&t.ffmpeg&&t.import)return {installed:false,tools:t};
  try{execSync('apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq xvfb i3 ffmpeg imagemagick xdotool mesa-vulkan-drivers libvulkan1 x11-utils',{stdio:'pipe',timeout:600000});return {installed:true,tools:tools()}}
  catch(e){return {installed:false,error:String(e.message).slice(0,300),tools:tools()}}}
export async function startDisplay(n=62,size='1440x900x24'){
  if(process.env.E2E_ALLOW_DESKTOP!=='1'&&process.env.DISPLAY&&!process.env.BOAT_SANDBOX&&!process.env.CI&&!existsSync('/home/user'))throw Error('refusing to start a display here: run on Boat (set E2E_ALLOW_DESKTOP=1 only with the person\'s word)');
  const xv=spawn('Xvfb',[`:${n}`,'-screen','0',size,'-nolisten','tcp'],{stdio:'ignore'});await sleep(1500);
  const i3=spawn('i3',[],{stdio:'ignore',env:{...process.env,DISPLAY:`:${n}`}});await sleep(1500);
  return {display:`:${n}`,env:{...process.env,DISPLAY:`:${n}`},stop:()=>{for(const p of[i3,xv])try{p.kill('SIGTERM')}catch{}}}}
export function startRecording(display,file,fps=30){mkdirSync(file.replace(/\/[^/]+$/,''),{recursive:true});
  if(!have('ffmpeg'))return {stop:async()=>({file:null,reason:'no ffmpeg'})};
  const p=spawn('ffmpeg',['-y','-loglevel','error','-f','x11grab','-framerate',String(fps),'-video_size','1440x900','-i',display,'-c:v','libx264','-preset','ultrafast','-pix_fmt','yuv420p',file],{stdio:'ignore'});
  return {stop:async()=>{try{p.kill('SIGINT')}catch{}await sleep(1500);return {file}}}}
export function screenshot(display,file){mkdirSync(file.replace(/\/[^/]+$/,''),{recursive:true});try{execSync(`import -window root ${file}`,{env:{...process.env,DISPLAY:display},stdio:'ignore'});return file}catch{return null}}
