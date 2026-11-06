// Xvfb + i3 + screen capture (Boat). Software rendering there: frame data from it validates the harness, it is not GPU evidence.
import {spawn,execSync} from 'node:child_process';import {mkdirSync,existsSync,readdirSync,writeFileSync} from 'node:fs';
import {sleep} from '../../perf/lib.mjs';
const have=c=>{try{execSync(`command -v ${c}`,{stdio:'ignore'});return true}catch{return false}};
export function tools(){return {Xvfb:have('Xvfb'),i3:have('i3'),ffmpeg:have('ffmpeg'),import:have('import'),xdotool:have('xdotool'),vulkaninfo:have('vulkaninfo')}}
export function installMissing(){
  if(!process.env.BOAT_SANDBOX)return {installed:false,tools:tools(),note:'apt only inside a Boat sandbox'}; // best effort on a disposable Boat sandbox (root there); never on the desktop
  const t=tools();if(t.Xvfb&&t.i3&&t.ffmpeg&&t.import)return {installed:false,tools:t};
  try{execSync('sudo -n apt-get update -qq && sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq xvfb i3 ffmpeg imagemagick xdotool x11-xserver-utils x11-utils mesa-vulkan-drivers libvulkan1',{stdio:'pipe',timeout:600000});return {installed:true,tools:tools()}}
  catch(e){return {installed:false,error:String(e.message).slice(0,300),tools:tools()}}}
export async function startDisplay(n=62,size='1440x900x24'){
  if(process.env.E2E_ALLOW_DESKTOP!=='1'&&process.env.DISPLAY&&!process.env.BOAT_SANDBOX&&!process.env.CI&&!existsSync('/home/user'))throw Error('refusing to start a display here: run on Boat (set E2E_ALLOW_DESKTOP=1 only with the person\'s word)');
  const xv=spawn('Xvfb',[`:${n}`,'-screen','0',size,'-nolisten','tcp'],{stdio:'ignore'});await sleep(1500);
  writeFileSync('/tmp/e2e-i3.conf','font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\ndefault_floating_border none\nhide_edge_borders both\n');
  const i3=have('i3')?spawn('i3',['-c','/tmp/e2e-i3.conf'],{stdio:'ignore',env:{...process.env,DISPLAY:`:${n}`}}):null;await sleep(1500); // no i3 (h@backplane): a bare Xvfb maps the one window at the origin
  const env={...process.env,DISPLAY:`:${n}`,VK_ICD_FILENAMES:process.env.VK_ICD_FILENAMES||'/usr/share/vulkan/icd.d/lvp_icd.json'}; // llvmpipe on Boat: software rendering
  return {display:`:${n}`,env,stop:()=>{for(const p of[i3,xv].filter(Boolean))try{p.kill('SIGTERM')}catch{}}}}
export function startRecording(display,file,fps=30){mkdirSync(file.replace(/\/[^/]+$/,''),{recursive:true});
  if(!have('ffmpeg'))return {stop:async()=>({file:null,reason:'no ffmpeg'}),offsetMs:()=>NaN};
  const offs=[];const p=spawn('ffmpeg',['-y','-loglevel','error','-progress','pipe:1','-stats_period','0.5','-f','x11grab','-framerate',String(fps),'-video_size','1440x900','-i',display,'-c:v','libx264','-preset','ultrafast','-pix_fmt','yuv420p',file],{stdio:['ignore','pipe','ignore']});
  let buf='';p.stdout.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);const m=/^out_time_us=(\d+)/.exec(l);if(m)offs.push(Date.now()-Number(m[1])/1000)}});
  // epoch (ms) at which the recording's time 0 happened: median of (arrival time - out_time) over the progress lines; accurate to tens of ms
  return {offsetMs:()=>{const s=[...offs].sort((a,b)=>a-b);return s.length?s[s.length>>1]:NaN},stop:async()=>{try{p.kill('SIGINT')}catch{}await sleep(1500);return {file}}}}
export function screenshot(display,file){mkdirSync(file.replace(/\/[^/]+$/,''),{recursive:true});try{execSync(`import -window root ${file}`,{env:{...process.env,DISPLAY:display},stdio:'ignore'});return file}catch{return null}}
