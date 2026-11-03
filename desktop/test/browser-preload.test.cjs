'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
function preload(enabled){
 const calls=[],listeners=new Map();let api;
 const ipcRenderer={sendSync:()=>'dark',invoke:(...args)=>{calls.push(args);return Promise.resolve({phase:'ready'});},on:(channel,fn)=>listeners.set(channel,fn),removeListener:(channel,fn)=>{if(listeners.get(channel)===fn)listeners.delete(channel);}};
 vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../preload.cjs'),'utf8'),{
  process:{argv:enabled?['--backplane-direct-browser=1']:[]},
  document:{documentElement:{classList:{toggle(){}}}},
  require:name=>{assert.equal(name,'electron');return{contextBridge:{exposeInMainWorld:(name,value)=>{assert.equal(name,'backplaneDesktop');api=value;}},ipcRenderer};},
 });
 return{api,calls,listeners};
}
test('only configured Electron shells advertise the geometry-only Browser bridge',async()=>{
 const ordinary=preload(false);assert.equal(ordinary.api.browser,undefined);
 const {api,calls}=preload(true);assert(api.browser);
 await api.browser.state();await api.browser.present({owner:'fixture',bounds:{x:0,y:0,width:100,height:100}});await api.browser.hide();
 assert.deepEqual(calls.map(x=>x[0]),['desktop:browser-state','desktop:browser-present','desktop:browser-hide']);
 for(const key of ['reconnect','token','command','evaluate','webContents','session','debugger'])assert.equal(api.browser[key],undefined);
});
test('Browser event subscriptions expose bounded UI state and retire cleanly',()=>{
 const {api,listeners}=preload(true),seen=[];
 const off=api.browser.onState(value=>seen.push(value));
 listeners.get('desktop:browser-state')({}, {phase:'ready',owner:'fixture',message:'ready',token:'private',target:{}});
 assert.deepEqual(Object.keys(seen[0]).sort(),['message','owner','phase']);
 listeners.get('desktop:browser-state')({}, {phase:'forged',message:'invalid'});assert.equal(seen.length,1);
 off();assert(!listeners.has('desktop:browser-state'));
 const taken=[];api.browser.onTakeover(value=>taken.push(value));
 listeners.get('desktop:browser-takeover')({}, {owner:'bad\nowner'});assert.equal(taken.length,0);
 listeners.get('desktop:browser-takeover')({}, {owner:'fixture',target:'private'});assert.deepEqual(Object.keys(taken[0]),['owner']);
});
