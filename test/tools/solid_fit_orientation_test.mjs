import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {runInNewContext} from 'node:vm';
import {fittedDistance} from '../../src/web/solid-framing.js';

// Execute the actual viewer's Orbit and View.fit without opening a canvas.
const source=readFileSync('src/web/solid.js','utf8').replace(/^import .*;\n/gm,'').replace(/^export /gm,'');
const {View,Orbit}=runInNewContext(source+'\n({View,Orbit});',{SolidAssets:class{},fittedDistance});
const box=[-4500,-2500,0,4500,2500,80],dot=(a,b)=>a.reduce((n,v,i)=>n+v*b[i],0);
let cases=0;
for(const [yaw,pitch] of [[0,Math.PI/2],[Math.PI,-Math.PI/2],[-.6,.5],[0,0],[Math.PI/2,0],[2.7,-.7]]){
  for(const roll of [0,.43])for(const projection of ['perspective','orthographic'])for(const aspect of [390/844,1,3]){
    const cam=new Orbit().aim(yaw,pitch).roll(roll);cam.fov=.9;cam.sx=113;cam.sy=-71;cam.dist=999;cam.home=111;
    const oldAxes=Object.fromEntries(['r','u','f'].map(k=>[k,Array.from(cam[k])]));
    const view=Object.assign(Object.create(View.prototype),{cam,box,projection,canvas:{clientWidth:aspect*844,clientHeight:844}});
    view.fit();
    for(const axis of ['r','u','f']){
      assert.deepEqual(Array.from(view.cam[axis]),oldAxes[axis],`Fit must preserve ${axis}, including selected face and roll`);
      assert.notEqual(view.cam[axis],cam[axis],'fitted camera owns its axis arrays');
    }
    assert.equal(view.cam.fov,.9);assert.equal(view.cam.sx,0);assert.equal(view.cam.sy,0);
    assert.deepEqual(Array.from(view.cam.pivot),[0,0,40]);assert.equal(view.cam.home,view.cam.dist);assert(view.cam.dist>0);
    for(const x of [box[0],box[3]])for(const y of [box[1],box[4]])for(const z of [box[2],box[5]]){
      const delta=[x,y,z-40],depth=view.cam.dist+dot(delta,view.cam.f);
      const scale=(projection==='orthographic'?view.cam.dist:depth)*Math.tan(view.cam.fov/2);
      assert(depth>0);assert(Math.abs(dot(delta,view.cam.r)/(scale*aspect))<=1);assert(Math.abs(dot(delta,view.cam.u)/scale)<=1);
    }
    const fitted=view.cam;view.fit();assert.equal(view.cam.dist,fitted.dist,'repeated Fit remains stable');cases++;
  }
}
console.log(`ok actual View.fit preserves face/roll/FOV and fits all corners in ${cases} cases`);
