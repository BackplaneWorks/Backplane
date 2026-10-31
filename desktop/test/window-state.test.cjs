'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path');
const {EventEmitter}=require('node:events');
const {restoreWindowBounds,rememberWindowBounds}=require('../window-state.cjs');
test('normal bounds survive close and fit the current display after a monitor is removed',t=>{
  const dir=fs.mkdtempSync(path.join(os.tmpdir(),'bp-window-state-'));t.after(()=>fs.rmSync(dir,{recursive:true,force:true}));const file=path.join(dir,'bounds.json');
  const window=new EventEmitter();let bounds={x:75,y:120,width:1040,height:720};window.getNormalBounds=()=>bounds;
  rememberWindowBounds(window,file);window.emit('resize');window.emit('move');window.emit('close');
  const screen={getDisplayMatching:()=>({workArea:{x:0,y:0,width:1920,height:1080}})};
  assert.deepEqual(restoreWindowBounds(file,screen),bounds);
  bounds={x:2500,y:-400,width:2200,height:1300};window.emit('close');
  assert.deepEqual(restoreWindowBounds(file,screen),{x:0,y:0,width:1920,height:1080});
  screen.getDisplayMatching=()=>({workArea:{x:-1280,y:24,width:1280,height:1000}});
  assert.deepEqual(restoreWindowBounds(file,screen),{x:-1280,y:24,width:1280,height:1000});
  assert.deepEqual(fs.readdirSync(dir),['bounds.json']);
});
test('missing or corrupt saved bounds keep the normal first-launch defaults',t=>{
  const dir=fs.mkdtempSync(path.join(os.tmpdir(),'bp-window-state-'));t.after(()=>fs.rmSync(dir,{recursive:true,force:true}));const file=path.join(dir,'bounds.json');
  const screen={getDisplayMatching:()=>{throw Error('Invalid bounds must not reach screen');}};
  assert.deepEqual(restoreWindowBounds(file,screen),{});
  for(const value of ['invalid','null','{}','{"x":0,"y":0,"width":1,"height":1}','{"x":0,"y":0,"width":"1040","height":720}']){
    fs.writeFileSync(file,value);assert.deepEqual(restoreWindowBounds(file,screen),{});
  }
});
