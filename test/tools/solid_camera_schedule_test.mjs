// Execute the production camera scheduling with GPU drawing replaced at its boundary.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {runInNewContext} from 'node:vm';
const frames=new Map();let serial=0;
const source=readFileSync('src/web/solid-pathtrace-core.js','utf8').replace(/^import .*;$/gm,'').replace(/^export /gm,'');
const View=runInNewContext(source+'\nOriginalModelView;',{
  requestAnimationFrame:fn=>{frames.set(++serial,fn);return serial;},cancelAnimationFrame:id=>frames.delete(id),
  setTimeout,clearTimeout,structuredClone,performance,console,DOMException,
});
function fixture(){
  frames.clear();const draws=[],reports=[],v=Object.create(View.prototype);
  Object.assign(v,{ready:true,mode:'raster',state:{visible:true,phase:'raster'},
    applyCamera(){},raster(){draws.push(this.cameraState);},report(...args){reports.push(args);},
    releaseMaskRelief(){},releaseRasterBatches(){},releaseTrace(){},
  });
  return {v,draws,reports};
}
function flush(){const batch=[...frames.values()];frames.clear();for(const fn of batch)fn();}
{
  const {v,draws}=fixture();
  for(let i=0;i<100;i++){v.setCamera({cam:{step:i}});v.setInteractive(true);}
  assert.equal(draws.length,0,'a burst of camera messages must not synchronously queue GPU draws');
  assert.equal(frames.size,1,'the newest pose needs only one scheduled frame');flush();
  assert.equal(draws.length,1);assert.equal(draws[0].cam.step,99,'the visible frame uses the newest camera');
  assert.equal(frames.size,0,'a settled Studio view does not repaint continuously');
  v.setInteractive(false);assert.equal(frames.size,1);flush();assert.equal(draws.length,2);assert.equal(frames.size,0);
}
{
  const {v,draws}=fixture();v.setCamera({cam:{step:1}});v.stop();
  assert.equal(frames.size,0,'stop cancels an obsolete camera frame');flush();assert.equal(draws.length,0);
  v.setCamera({cam:{step:2}});v.hidden=true;v.stop();assert.equal(frames.size,0);
  v.setCamera({cam:{step:3}});assert.equal(frames.size,0,'a hidden view does not queue a camera redraw');
  v.hidden=false;v.disposed=true;v.setCamera({cam:{step:4}});assert.equal(frames.size,0,'a disposed view cannot schedule new drawing');
}
{
  const {v,draws}=fixture();v.ready=false;v.setCamera({cam:{step:1}});v.setInteractive(true);
  assert.equal(v.cameraState.cam.step,1,'a pose heard during loading is retained');assert.equal(frames.size,0);assert.equal(draws.length,0);
}
{
  const {v}=fixture();let staleRay=false;
  v.raf=++serial;frames.set(v.raf,()=>{staleRay=true;});
  v.setCamera({cam:{step:1}});v.setInteractive(true);flush();assert.equal(staleRay,false,'camera input cancels pending ray work immediately');
}
console.log('ok camera bursts, newest pose, settled idle, stop/hide/dispose/loading and pending ray cancellation');

// Exercise the actual controller/worker message boundary as well: repeated
// key notifications must leave camera updates batched on the main thread.
{
 const sent=[],controllerFrames=new Map();let sequence=0;
 const controllerSource=readFileSync('src/web/solid-pathtrace.js','utf8').replace(/^export /gm,'');
 class Worker {
  postMessage(data){sent.push(data);if(data.id)queueMicrotask(()=>this.onmessage({data:{id:data.id,result:{ready:true}}}));}
  terminate(){}
 }
 const create=runInNewContext(controllerSource+'\ncreatePathTraceView;',{
  Worker,ResizeObserver:class{observe(){}disconnect(){}},URL,location:{href:'http://127.0.0.1:47649/',origin:'http://127.0.0.1:47649'},
  document:{addEventListener(){},removeEventListener(){}},devicePixelRatio:1,setTimeout,clearTimeout,
  requestAnimationFrame:fn=>{controllerFrames.set(++sequence,fn);return sequence;},
 });
 const canvas={clientWidth:400,clientHeight:300,transferControlToOffscreen:()=>({}),addEventListener(){},removeEventListener(){}};
 const controller=await create({canvas});sent.length=0;
 controller.setCamera({cam:{step:0}});controller.setInteractive(true);
 for(let i=1;i<=100;i++){controller.setCamera({cam:{step:i}});controller.setInteractive(true);}
 assert.equal(sent.filter(m=>m.method==='camera').length,1,'repeated moving notifications keep intermediate poses on the main thread');
 assert.equal(sent.filter(m=>m.method==='interactive').length,1);
 for(const fn of controllerFrames.values())fn();controllerFrames.clear();
 assert.equal(sent.filter(m=>m.method==='camera').length,2);assert.equal(sent.filter(m=>m.method==='camera').at(-1).args.cam.step,100);
 controller.setInteractive(false);controller.setInteractive(false);
 assert.equal(sent.filter(m=>m.method==='interactive').length,2,'settling is sent once');controller.dispose();
 console.log('ok controller camera batching and moving/settled notification deduplication');
}
