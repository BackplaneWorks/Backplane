// Execute the production compact view and original-model visibility handoff.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {runInNewContext} from 'node:vm';
import {ViewerQuality} from '../../src/web/viewer-quality.js';
import {fittedDistance} from '../../src/web/solid-framing.js';
const frames=new Map(),events=[],glCalls=[];let frameId=0;
const source=readFileSync('src/web/solid.js','utf8').replace(/^import .*;\n/gm,'').replace(/^export /gm,'');
const api=runInNewContext(source+'\n({View,Orbit,useView:v=>view=v,...(typeof setPreviewVisible === "function"?{setPreviewVisible}:{})});',{
 SolidAssets:class{get(){return null;}},fittedDistance,window:{devicePixelRatio:1},
 CustomEvent:class{constructor(type){this.type=type;}},
 requestAnimationFrame:fn=>{frames.set(++frameId,fn);return frameId;},cancelAnimationFrame:id=>frames.delete(id),clearInterval(){},
});
const gl=new Proxy({getUniformLocation(){return 1;}},{get(o,k){return o[k]??((...args)=>{glCalls.push(k);});}});
const canvas={isConnected:true,clientWidth:320,clientHeight:200,width:320,height:200,classList:{remove(){}},dispatchEvent:e=>events.push(e.type)};
const v=Object.assign(Object.create(api.View.prototype),{canvas,gl,cam:new api.Orbit(),probe:'',r:1,count:3,bufs:[],projection:'perspective',aperture:'off',exposure:0,thermal:{enabled:false,temperature:0,liquid:0},previewVisible:true});
api.useView(v);
const flush=()=>{const due=[...frames.values()];frames.clear();for(const fn of due)fn();};
const draws=()=>glCalls.filter(k=>k==='drawArrays').length;
v.later();flush();assert.equal(draws(),1,'Fast preview draws normally');
const quality=Object.assign(Object.create(ViewerQuality.prototype),{alive:true,canvas,solid:api,qualityCanvas:{style:{}},root:{classList:{toggle(){}},dispatchEvent:e=>events.push(e.type)}});
v.later();const queued=[...frames.values()][0];
quality.show(true);v.later();flush();
assert.equal(draws(),1,'the original Studio canvas stops covered compact draws');
assert(events.filter(x=>x==='solid-camera-change').length>=3,'camera changes still reach the original renderer while the preview is covered');
queued();assert.equal(draws(),1,'a canceled compact callback cannot draw behind Studio');
api.View.prototype.draw.call(v);assert.equal(draws(),1,'direct stale draws are also inert while covered');
api.setPreviewVisible({},true);assert.equal(frames.size,0,'another canvas cannot resume this preview');
quality.show(false);assert.equal(frames.size,1,'Fast/failure/thermal fallback schedules the current compact camera');flush();assert.equal(draws(),2);
quality.show(false);assert.equal(frames.size,0,'unchanged fallback visibility does not repaint');
v.later();const canceled=[...frames.values()][0];v.dispose();v.later();canceled();assert.equal(frames.size,0);assert.equal(draws(),2,'disposed resize/frame callbacks cannot draw');
const detached=Object.assign(Object.create(api.View.prototype),{...v,disposed:false,previewVisible:true,canvas:{...canvas,isConnected:false},gl});
api.View.prototype.draw.call(detached);detached.later();assert.equal(draws(),2);assert.equal(frames.size,0,'detached compact views schedule no work');
console.log('ok production compact preview: Studio cover stops GPU draws while camera changes continue; fallback resumes once; canvas ownership and disposed/detached callbacks remain inert');
