#!/usr/bin/env bun
// A stand-in for the GPUI binary that speaks the metrics contract (G1-G5), to test the adapter without a display. Not a measurement of anything.
import {readFileSync} from 'node:fs';
const a=process.argv.slice(2);const sf=a[a.indexOf('--script')+1];const steps=JSON.parse(readFileSync(sf,'utf8'));
const t0=performance.now();const t=()=>Math.round((performance.now()-t0)*1000)/1000;const m=(event,o={})=>console.error('GPUI_METRIC '+JSON.stringify({...o,event,t_ms:t(),epoch_ms:Date.now()}));
const dead=process.env.FAKE_DEAD||'dead-btn';let rec=null;
m('process_start');m('gpu',{device:process.env.FAKE_SOFTWARE==='1'?'llvmpipe':'NVIDIA GeForce RTX 5080',software:process.env.FAKE_SOFTWARE==='1'});m('first_frame');m('usable',{threads:20});
m('controls',{controls:[{id:'save',label:'Save',enabled:true,visible:true,wired:true},{id:'open',label:'Open',enabled:true,visible:true,wired:true},{id:dead,label:'Nothing',enabled:true,visible:true,wired:false}]});
let lay=0;const layout=shift=>m('layout',{rects:{sidebar:{x:0,y:0,w:300,h:900},main:{x:300,y:shift?200:0,w:1140,h:700}}});
layout(false);
for(const s of steps){
 if(s.record==='start'){rec=[];m('record',{state:'start',name:s.name});for(let i=0;i<120;i++)rec.push(Math.round(i*16.667*1000)/1000);continue}
 if(s.record==='stop'){m('frames',{name:'x',n:rec.length,frames_ms:rec});rec=null;continue}
 if(s.wait){await new Promise(r=>setTimeout(r,Math.min(s.wait,20)));continue}
 const label=s.select!==undefined?`select:${s.select}`:s.popup!==undefined?`popup:${s.popup}`:s.click?`click:${s.click}`:s.scroll!==undefined?`scroll:${s.scroll}`:s.net_sleep?`net_sleep:${s.net_sleep}`:s.home!==undefined?`home:${s.home}`:'x';
 if(s.click){const id=s.click;m('click',{id,found:true,changed:id!==dead,action:id!==dead?'did':null});continue}
 if(['@','>','%'].includes(s.popup)&&process.env.FAKE_NOPOPUP==='1'){m('input',{step:label,ok:false,error:'timeout',latency_ms:15000});continue}
 m('ack',{step:label,ack_ms:9.5});m('input',{step:label,ok:true,latency_ms:55});
 if(process.env.FAKE_SHIFT==='1'&&s.select===1){await new Promise(r=>setTimeout(r,650));layout(true)} // a jump 650 ms after the input: outside the 500 ms quiet window
}
m('script_done',{steps:steps.length});
