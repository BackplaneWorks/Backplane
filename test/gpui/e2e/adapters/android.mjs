// Android adapter: adb only. Controls inventory, dead-control sweep, layout snapshots and frame times come from the OS, not from the app.
import {execSync} from 'node:child_process';import {mkdirSync,writeFileSync} from 'node:fs';
import {sleep} from '../../perf/lib.mjs';import {frameStats,deadControls,layoutShift} from '../lib/frames.mjs';
export const name='android';
export const PKG=process.env.E2E_ANDROID_PKG||'dev.backplane.mobile.gpui';
export const capabilities={select:null,popup:null,type:null,send:null,wait_reply:null,watch_stream:null,agents:null,memory:null,scroll:null,controls:null,frames:null,layout:null,pair:'app:invite',paste_image:'adb:clipboard',viewer_board:null,viewer_schematic:null,viewer_3d:null,files:null,download:null,settings:null,bots:null,rooms:null,network_sleep:'adb:svc'};
const adb=(a,o={})=>execSync(`adb ${a}`,{encoding:'utf8',timeout:60000,...o});
export const devices=()=>{try{return adb('devices').split('\n').slice(1).filter(l=>/\tdevice$/.test(l)).map(l=>l.split('\t')[0])}catch{return []}};
export function dump(){adb('shell uiautomator dump /sdcard/e2e.xml >/dev/null');return adb('shell cat /sdcard/e2e.xml')}
export function nodes(xml){const out=[];for(const m of xml.matchAll(/<node [^>]*>/g)){const a=k=>new RegExp(k+'="([^"]*)"').exec(m[0])?.[1]??'';const b=/\[(\d+),(\d+)\]\[(\d+),(\d+)\]/.exec(a('bounds'));if(!b)continue;
  out.push({id:a('resource-id')||a('content-desc')||a('text')||`${b[1]},${b[2]}`,label:a('text')||a('content-desc'),clickable:a('clickable')==='true',enabled:a('enabled')==='true',visible:true,rect:{x:+b[1],y:+b[2],w:+b[3]-+b[1],h:+b[4]-+b[2]}})}return out}
export const tap=(x,y)=>adb(`shell input tap ${Math.round(x)} ${Math.round(y)}`);
export function startFrames(){adb(`shell dumpsys gfxinfo ${PKG} reset`)}
export function readFrames(){ // framestats: rows of 14 timestamps in ns; column 1 = INTENDED_VSYNC, 13 = FRAME_COMPLETED
  const t=adb(`shell dumpsys gfxinfo ${PKG} framestats`);const rows=t.split('---PROFILEDATA---')[1]?.split('\n').slice(2).map(l=>l.split(',').map(Number)).filter(r=>r.length>=14&&r[0]===0)||[];
  return rows.map(r=>r[13]/1e6)}
// dead-control sweep: click every clickable, enabled node once on the current screen; changed = the UI tree differs afterwards
export async function sweep(){const before0=dump();const inv=nodes(before0).filter(n=>n.clickable);const results={};
  for(const n of inv){const b=dump();tap(n.rect.x+n.rect.w/2,n.rect.y+n.rect.h/2);await sleep(400);const a=dump();results[n.id]={changed:a!==b,action:false};if(a!==b){adb('shell input keyevent KEYCODE_BACK');await sleep(300)}}
  return deadControls(inv,results)}
export async function run(j,ctx){const d=await import('../android/driver.mjs');return d.run(j,ctx)} // step driver: ../android/driver.mjs (real taps, typing, screenrecord)
