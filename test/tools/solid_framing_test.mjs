import assert from 'node:assert/strict';
import {fittedDistance} from '../../src/web/solid-framing.js';

const flat = [-4500,-2500,0,4500,2500,80];
const dot = (a,b) => a.reduce((sum,v,i)=>sum+v*b[i],0);
let cases = 0;
for (const box of [flat,[-100,-100,-100,100,100,100],[-2,-1,-.2,2,1,.2]]) {
  for (const aspect of [390/844,1,1200/900,3]) for (const orthographic of [false,true]) for (const [yaw,pitch] of [[-.6,.5],[0,Math.PI/2],[0,0],[Math.PI,-Math.PI/2],[2.7,-.7]]) {
    const f=[-Math.sin(yaw)*Math.cos(pitch),Math.cos(yaw)*Math.cos(pitch),-Math.sin(pitch)],r=[Math.cos(yaw),Math.sin(yaw),0],u=[r[1]*f[2],-r[0]*f[2],r[0]*f[1]-r[1]*f[0]];
    const pivot=[0,1,2].map(i=>(box[i]+box[i+3])/2),cam={f,r,u,pivot,fov:.7};
    const distance=fittedDistance(box,cam,aspect,orthographic);
    assert(Number.isFinite(distance)&&distance>0);
    let occupied=0;
    for(const x of [box[0],box[3]])for(const y of [box[1],box[4]])for(const z of [box[2],box[5]]) {
      const delta=[x-pivot[0],y-pivot[1],z-pivot[2]],depth=distance+dot(delta,f);
      assert(depth>0,'geometry remains in front of the camera');
      const scale=(orthographic?distance:depth)*Math.tan(cam.fov/2);
      const nx=dot(delta,r)/(scale*aspect),ny=dot(delta,u)/scale;
      assert(Math.abs(nx)<=1&&Math.abs(ny)<=1,'every saved bound stays inside the viewport');
      occupied=Math.max(occupied,Math.abs(nx),Math.abs(ny));
    }
    assert(occupied>.75,'fit uses available width or height rather than an empty bounding sphere');
    cases++;
  }
}
const camera={pivot:[0,0,0],r:[1,0,0],u:[0,1,0],f:[0,0,-1],fov:Math.PI/2};
assert(Math.abs(fittedDistance([-2,-1,-.2,2,1,.2],camera,2,false,1)-1.2)<1e-12);
assert(Math.abs(fittedDistance([-2,-1,-.2,2,1,.2],camera,2,true,1)-1)<1e-12);
console.log(`ok projected camera framing: ${cases} PCB/cube/aspect/orientation/projection cases, all bounds visible and useful viewport occupancy`);
