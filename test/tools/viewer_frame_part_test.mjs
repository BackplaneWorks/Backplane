import assert from 'node:assert/strict';
import {ViewerQuality} from '../../src/web/viewer-quality.js';
function fixture(){
 let resolve,reject;const pending=new Promise((ok,no)=>{resolve=ok;reject=no;}),messages=[],frames=[];
 const renderer={componentBounds(){return pending;}},q=Object.assign(Object.create(ViewerQuality.prototype),{
  alive:true,generation:4,renderer,components:{selected:'saved-part'},revision:()=> 'saved-revision',
  root:{querySelector:()=>({scrollTop:30})},solid:{frameBounds:(...args)=>frames.push(args)},message:text=>messages.push(text),
 });return{q,resolve,reject,messages,frames};
}
for(const move of [q=>{q.generation++;q.renderer=null;},q=>{q.renderer={};},q=>{q.components.selected='another-part';},q=>{q.alive=false;}]){
 const {q,resolve,reject,messages,frames}=fixture(),done=q.framePart();move(q);reject(Error('Renderer disposed'));await done;
 assert.deepEqual(messages,[],'superseded framing failures cannot overwrite the active mode or selection');assert.deepEqual(frames,[]);
 const b=fixture(),success=b.q.framePart();move(b.q);b.resolve({revision:'saved-revision',min:[0,0,0],max:[1,1,1]});await success;
 assert.deepEqual(b.frames,[],'superseded framing success cannot move the current camera');assert.deepEqual(b.messages,[]);
}
{
 const {q,resolve,frames,messages}=fixture(),done=q.framePart();resolve({revision:'saved-revision',min:[1,2,3],max:[4,5,6]});await done;
 assert.deepEqual(frames,[[[1,2,3],[4,5,6]]]);assert.deepEqual(messages,[]);
}
{
 const {q,reject,messages}=fixture(),done=q.framePart();reject(Error('Source model unavailable'));await done;assert.deepEqual(messages,['Source model unavailable'],'a current failure is still explained');
}
{
 const {q,resolve,frames,messages}=fixture(),done=q.framePart();resolve({revision:'older-revision',min:[0,0,0],max:[1,1,1]});await done;
 assert.deepEqual(frames,[]);assert.match(messages[0],/no uniquely identified source model/,'revision authority remains mandatory');
}
console.log('ok production Frame part: cancellation/replacement/selection/disposal cannot move the camera or overwrite status; current success/error and exact-revision guards remain');
