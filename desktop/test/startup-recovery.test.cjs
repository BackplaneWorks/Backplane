'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=name=>fs.readFileSync(path.join(__dirname,'..',name),'utf8');
function fixture(){
  const s=source('main.cjs'),events=[],handlers={};
  let reject,resolve;
  const ctx={connectionStatus:{phase:'connecting',message:'Connecting to your service…'},connecting:null,quitting:false,directBrowser:false,
    service:{start:()=>new Promise((yes,no)=>{resolve=yes;reject=no;}),token:async()=> 'private'},
    window:{isDestroyed:()=>false,webContents:{send:(channel,value)=>events.push(value)},loadURL:async()=>{}},
    senderIsWindow:e=>e.main===true,startupURL:'file:///app/startup.html',ipcMain:{on:(n,f)=>handlers[n]=f},Date};
  vm.createContext(ctx);vm.runInContext(s.slice(s.indexOf('function status('),s.indexOf('// one attempt at a time:')),ctx);
  const ses={fetch:async()=>({ok:true})};
  return {ctx,events,handlers,start:()=>ctx.connect(ses),fail:message=>reject(Error(message)),ready:()=>resolve({origin:'http://127.0.0.1:4000'})};
}
function page(main){
  const nodes={status:{textContent:'Connecting to your service…'},retry:{hidden:true,addEventListener:(name,fn)=>nodes.retry.click=fn}};
  let listener;
  vm.runInNewContext(source('startup.js'),{document:{getElementById:id=>nodes[id]},window:{backplaneDesktop:{onConnection:f=>listener=f,connectionState:()=>main.ctx.connectionStatus,retry:()=>main.start()}}});
  return{nodes,receive:()=>listener(main.ctx.connectionStatus)};
}
test('reloading a failed startup replays its error and keeps Retry usable',async()=>{
  const f=fixture(),pending=f.start();f.fail('Could not read credentials');await pending;
  const first=page(f),reloaded=page(f);
  for(const p of [first,reloaded]){assert.equal(p.nodes.status.textContent,'Could not read credentials');assert.equal(p.nodes.retry.hidden,false);}
  const retry=reloaded.nodes.retry.click();assert.equal(f.ctx.connectionStatus.phase,'connecting');
  const during=page(f);assert.equal(during.nodes.retry.hidden,true);f.ready();await retry;await new Promise(r=>setImmediate(r));
  assert.equal(f.ctx.connecting,null);
});
test('a reload during an attempt never starts a second service attempt',async()=>{
  const f=fixture(),pending=f.start();page(f);page(f);assert.equal(f.events.length,1);f.fail('Unavailable');await pending;
  assert.equal(page(f).nodes.retry.hidden,false);
});
test('connection state is answered only to this window main frame on the local startup document',()=>{
  const f=fixture(),read=(main,url)=>{const e={main,senderFrame:{url}};f.handlers['desktop:connection-now'](e);return e.returnValue;};
  assert.equal(read(true,'file:///app/startup.html'),f.ctx.connectionStatus);
  assert.equal(read(false,'file:///app/startup.html'),null);assert.equal(read(true,'https://other'),null);assert.equal(read(true,'http://127.0.0.1:4000'),null);
});
