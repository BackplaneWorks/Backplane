// bun lib/frames.test.mjs  (run on Boat; nothing here needs a display)
import assert from 'node:assert/strict';
import {acks,parse,toEpochMs} from './video-frames.mjs';
import {clickStats,frameStats,layoutShift,deadControls,rssSlope} from './frames.mjs';
// 60 Hz budget 16.67 ms
let c=clickStats([{id:'a',t_inject:0,t_ack:10,t_result:80},{id:'b',t_inject:100,t_ack:112,t_result:150}],60);
assert.equal(c.pass,true);assert.equal(c.overBudget,0);assert.ok(Math.abs(c.ackMs.p50-12)<=2||c.ackMs.p50===12||c.ackMs.p50===10);
c=clickStats([{id:'a',t_inject:0,t_ack:40,t_result:80}],60);assert.equal(c.pass,false);assert.equal(c.overBudget,1);
c=clickStats([{id:'a',t_inject:0,t_result:80}],60);assert.equal(c.pass,false);assert.equal(c.noAck,1);
// evidence coarser than the budget -> unknown (null), never pass
c=clickStats([{id:'a',t_inject:0,t_ack:10,t_result:80}],60,0,33.3);assert.equal(c.pass,null);
// smooth 60 fps vs one 100 ms hitch
const smooth=Array.from({length:120},(_,i)=>i*16.667);let f=frameStats(smooth,60);assert.equal(f.jank,0);assert.equal(f.pass,true);assert.ok(Math.abs(f.fps-60)<0.5);
const hitch=[...smooth.slice(0,60),...smooth.slice(60).map(t=>t+100)];f=frameStats(hitch,60);assert.equal(f.jank,1);assert.ok(f.droppedEstimate>=5);assert.equal(f.longOver50,1);
assert.equal(frameStats([0,10],60).pass,false); // too few frames never passes
// layout shift: nothing moves
const vp={w:1000,h:800};const base={x:0,y:0,w:1000,h:100};
let s=layoutShift([{t:0,inputT:null,rects:{h:base,b:{x:0,y:100,w:1000,h:700}}},{t:100,inputT:null,rects:{h:base,b:{x:0,y:100,w:1000,h:700}}}],vp);assert.equal(s.score,0);assert.equal(s.pass,true);
// body jumps down 200px with no input: impact 0.875 * dist 0.25
s=layoutShift([{t:0,inputT:null,rects:{b:{x:0,y:100,w:1000,h:700}}},{t:100,inputT:null,rects:{b:{x:0,y:300,w:1000,h:700}}}],vp);assert.ok(s.score>0.1&&s.pass===false,JSON.stringify(s));
// same jump right after a click is not counted
s=layoutShift([{t:0,inputT:null,rects:{b:{x:0,y:100,w:1000,h:700}}},{t:100,inputT:60,rects:{b:{x:0,y:300,w:1000,h:700}}}],vp);assert.equal(s.score,0);
// dead controls
let d=deadControls([{id:'a',label:'Save'},{id:'b',label:'Nothing'},{id:'c',label:'Off',enabled:false},{id:'d',label:'Hidden',visible:false}],{a:{changed:true},b:{changed:false,action:false}});
assert.equal(d.deadCount,1);assert.equal(d.dead[0].id,'b');assert.equal(d.clickable,2);assert.equal(d.pass,false);
d=deadControls([{id:'a',label:'x'},{id:'z',label:'never clicked'}],{a:{action:true}});assert.equal(d.deadCount,1);assert.equal(d.notClicked,1); // an unclicked control is not assumed alive
assert.equal(deadControls([],{}).pass,false); // no controls found never passes
// rss slope: flat passes, +100 MB/h fails
const flat=Array.from({length:60},(_,i)=>({t_s:i*60+300,rssMB:300+(i%3)}));assert.equal(rssSlope(flat).pass,true);
const grow=Array.from({length:60},(_,i)=>({t_s:i*60+300,rssMB:300+100*(i*60/3600)}));const g=rssSlope(grow);assert.ok(Math.abs(g.slopeMBh-100)<1);assert.equal(g.pass,false);
assert.equal(rssSlope([{t_s:400,rssMB:1}]).pass,false);
// video helpers
assert.deepEqual(parse('x pts_time:1.5 y\n n:1 pts_time:2.25'),[1.5,2.25]);
assert.deepEqual(acks([1000,1040,2000],[990,1500,2100]),[10,500,NaN]);
// real ffmpeg round trip when available: a moving test pattern must give changed frames, a static one none
import {execFileSync} from 'node:child_process';import {changedFrames} from './video-frames.mjs';
try{execFileSync('ffmpeg',['-y','-loglevel','error','-f','lavfi','-i','testsrc=duration=2:rate=10:size=160x120',(process.env.TMPDIR||'/tmp')+'/e2e-moving.mp4']);execFileSync('ffmpeg',['-y','-loglevel','error','-f','lavfi','-i','color=c=blue:duration=2:rate=10:size=160x120',(process.env.TMPDIR||'/tmp')+'/e2e-static.mp4']);
 const mv=changedFrames((process.env.TMPDIR||'/tmp')+'/e2e-moving.mp4',0.00005),st=changedFrames((process.env.TMPDIR||'/tmp')+'/e2e-static.mp4',0.00005);assert.ok(mv.length>=15,'moving frames: '+mv.length);assert.equal(st.length,0,'static frames: '+st.length);console.log('ffmpeg changed-frame check ok:',mv.length,'moving,',st.length,'static')}
catch(e){if(e.code==='ENOENT')console.log('no ffmpeg: video check skipped');else throw e}
console.log('frames.test.mjs: all assertions passed');
