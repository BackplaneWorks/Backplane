'use strict';
const {test}=require('node:test'), assert=require('node:assert/strict'), fs=require('node:fs'), vm=require('node:vm');
const source=fs.readFileSync(require.resolve('../../src/web/host.js'),'utf8');
// Execute the real delegated DOM event handler, with navigation observable.
function click(action, tag='A') {
 const listeners={},sent=[],el={tagName:tag,getAttribute:n=>n==='data-on-click'?action:n==='data-value'?'token':null,closest:()=>null};
 const code=source.slice(source.indexOf('for (const ev of EVENTS) {'),source.indexOf('// A page the person drives',source.indexOf('for (const ev of EVENTS) {')));
 // (phoneClickAction: phone-thread.js's, which changes nothing off a phone)
 vm.runInNewContext(code,{EVENTS:['click'],document:{addEventListener:(ev,fn)=>listeners[ev]=fn},dispatch:(a,v)=>sent.push([a,v]),valueOf:()=> 'token',phoneClickAction:(doc,el,action)=>action,pressedPick:null});
 let prevented=false;
 listeners.click({target:{closest:()=>el},preventDefault:()=>{prevented=true}});
 return {prevented,sent};
}
test('star link retains user navigation while saving the hub decision',()=>{
 assert.deepEqual(click('star-open'),{prevented:false,sent:[['star-open','token']]});
});
test('dismiss never navigates and still dispatches the saved choice',()=>{
 assert.deepEqual(click('star-dismiss','BUTTON'),{prevented:false,sent:[['star-dismiss','token']]});
});
test('existing file links keep their hub-owned open behavior',()=>{
 assert.deepEqual(click('file-open'),{prevented:true,sent:[['file-open','token']]});
});
