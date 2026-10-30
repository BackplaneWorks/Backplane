'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=fs.readFileSync(path.resolve(__dirname,'../../src/web/host.js'),'utf8');
function fixture(accepted=true){const calls=[];let restored=true;const ctx={viewMode:'view-v1',thin:{ready:true,request:(action,value)=>{calls.push({action,value});if(action==='bfield'&&accepted)restored=false;return accepted;}},thinMailDraft:{needsSubmit:()=>restored},document:{getElementById:id=>id==='mail-reply'?{value:'restored 日本語 👩‍💻 é\n'}:null,querySelector:()=>null}};
 const a=source.indexOf('function dispatch('),b=source.indexOf('// a right click on the plot',a);vm.createContext(ctx);vm.runInContext(source.slice(a,b),ctx);return{ctx,calls};}
test('actual human Send captures the displayed restored reply before its original empty-valued mail-send',()=>{
 const f=fixture();assert.equal(f.calls.length,0);f.ctx.dispatch('mail-send','');assert.deepEqual(f.calls,[{action:'bfield',value:'mrreply\x1frestored 日本語 👩‍💻 é\n'},{action:'mail-send',value:''}]);
});
test('actual rejected restored-field admission blocks Send rather than sending stale actor text',()=>{
 const f=fixture(false);f.ctx.dispatch('mail-send','');assert.equal(f.calls.length,1);assert.equal(f.calls[0].action,'bfield');
});
test('an already typed or accepted field does not manufacture another field action on Send',()=>{
 const f=fixture();f.ctx.thinMailDraft.needsSubmit=()=>false;f.ctx.dispatch('mail-send','');assert.deepEqual(f.calls,[{action:'mail-send',value:''}]);
});
test('cached readonly presentation cannot submit a stored draft or captured action',()=>{
 const f=fixture();f.ctx.viewMode='read-only';f.ctx.thin.ready=false;f.ctx.showViewStatus=()=>{};f.ctx.dispatch('mail-send','');assert.equal(f.calls.length,0);
});
