'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const load=()=>import('../../src/web/phone-shell.js');
test('?phone=1/0 overrides and is kept; ?phone=auto forgets it',async()=>{
 const {phoneOverride}=await load();
 assert.deepEqual(phoneOverride('?phone=1',null),{value:'1',keep:'1'});
 assert.deepEqual(phoneOverride('?a=b&phone=0','1'),{value:'0',keep:'0'});
 assert.deepEqual(phoneOverride('?phone=auto','1'),{value:null,keep:''});
 assert.deepEqual(phoneOverride('','0'),{value:'0',keep:null});
 assert.deepEqual(phoneOverride('','x'),{value:null,keep:null});
});
test('a phone is a coarse pointer on a short side of 600 px or less, unless overridden',async()=>{
 const {phoneDecide}=await load();
 assert.equal(phoneDecide({override:null,coarse:true,shortSide:393}),true);
 assert.equal(phoneDecide({override:null,coarse:true,shortSide:820}),false);
 assert.equal(phoneDecide({override:null,coarse:false,shortSide:393}),false);
 assert.equal(phoneDecide({override:'1',coarse:false,shortSide:1200}),true);
 assert.equal(phoneDecide({override:'0',coarse:true,shortSide:393}),false);
});
test('only a phone adds anything to a view request',async()=>{
 const {phoneRequest}=await load();
 assert.deepEqual(phoneRequest(true),{form:'phone'});
 assert.deepEqual(phoneRequest(false),{});
});
test('the shell does nothing on a desktop page',async()=>{
 const {installPhoneShell}=await load();
 const s=installPhoneShell({on:false,doc:null,win:null,dispatch:()=>{throw Error('no');}});
 assert.equal(s.on,false);s.rendered();
});
test('a sidebar thread row swipes as the phone apps: settle; snooze an hour or delete through its menu',async()=>{
 const {sidebarSwipes}=await load();
 const row={matches:s=>s.includes('.thread'),getAttribute:n=>n==='data-value'?'t1':null};
 const s=sidebarSwipes(row);
 assert.deepEqual(s.lead.map(x=>[x.action,x.value]),[['row-settle','t1']]);
 assert.deepEqual(s.trail.map(x=>[x.action,x.value,!!x.menu]),[['row-snooze','t1|1h',false],['row-delete','t1',true]]);
 assert.equal(sidebarSwipes({matches:()=>false,getAttribute:()=>'x'}),null);
});
