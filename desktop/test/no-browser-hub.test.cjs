'use strict';
// The desktop app loads its UI from a hub that does not serve the in-app
// browser (started by systemd, --headless or an older install): the browser
// reads as unavailable, nothing else stops. main.cjs runs against a stubbed
// Electron, a stubbed service and a browser controller that cannot connect.
const {test}=require('node:test'),assert=require('node:assert/strict'),Module=require('node:module'),path=require('node:path'),os=require('node:os'),fs=require('node:fs');

function harness(hang=false,{discover,keepOpen=false,token}={}){
  const calls={timers:[],loadURL:[],sent:[],errors:[],handlers:new Map(),controllers:0,controllerOptions:[],discover:[],tokenReads:0,token:'a'.repeat(32)};
  let ready;const whenReady=new Promise(r=>ready=r);
  const events=()=>{const on=new Map();return{on(e,f){on.set(e,f);return this;},once(e,f){on.set(e,f);return this;},emit(e,...a){on.get(e)?.(...a);}};};
  class BrowserWindow{
    constructor(){Object.assign(this,events());const w=this;
      this.webContents={...events(),send:(ch,v)=>calls.sent.push([ch,v]),setWindowOpenHandler(){},getZoomFactor:()=>1,isDestroyed:()=>false,focus(){}};
      BrowserWindow.all.push(this);setImmediate(()=>w.emit('ready-to-show'));}
    show(){}setMenuBarVisibility(){}isDestroyed(){return false;}isMinimized(){return false;}getContentBounds(){return{width:800,height:600};}
    loadFile(){return Promise.resolve();}
    loadURL(url){calls.loadURL.push(url);return Promise.resolve();}
    static getAllWindows(){return BrowserWindow.all;}
  }
  BrowserWindow.all=[];
  const ses={on(){},setDisplayMediaRequestHandler(){},setPermissionRequestHandler(){},setPermissionCheckHandler(){},webRequest:{onHeadersReceived(){}},
    fetch:async url=>({ok:url.endsWith('/auth/pair')})};
  const paths=new Map();
  const electron={app:{isPackaged:false,setPath:(key,value)=>paths.set(key,value),getPath:key=>paths.get(key),setName(){},requestSingleInstanceLock:()=>true,on(){},quit(){},whenReady:()=>whenReady},
    BrowserWindow,Menu:{setApplicationMenu(){},buildFromTemplate:x=>x},dialog:{showErrorBox:(a,b)=>calls.errors.push(b)},
    ipcMain:{handle:(ch,f)=>calls.handlers.set(ch,f),on(){}},session:{fromPartition:()=>ses},shell:{},webContents:{getFocusedWebContents:()=>null}};
  class Service{async discover(options){calls.discover.push(options);return discover?discover(options):{origin:'http://127.0.0.1:1',version:'fixture'};}async start(){return{origin:'http://127.0.0.1:1',version:'fixture'};}async token(){calls.tokenReads++;return token?token(calls):calls.token;}}
  // the hub has no /desktop/browser/v1: the controller's start fails and it closes
  const controller={createDesktopBrowserController:options=>{const{onState}=options;calls.controllers++;calls.controllerOptions.push(options);let closed=false,state={phase:'connecting'};
    return{async start(){if(hang)return new Promise(()=>{});closed=!keepOpen;state=keepOpen?{phase:'ready'}:{phase:'error',message:'Browser connection unavailable'};onState(state);},
      state:()=>state,isClosed:()=>closed,close(){closed=true;},hide(){},present:async()=>({shown:false}),edit(){}};}};
  return{calls,ready,stubs:{electron,'./browser-recovery.cjs':{createBrowserRecovery:options=>require('../browser-recovery.cjs').createBrowserRecovery({...options,schedule:(fn,ms)=>{const timer={fn,ms};calls.timers.push(timer);return timer;},cancel:timer=>{timer.cancelled=true;}})},'./service.cjs':{Service},'./browser-controller.cjs':controller},ses};
}

async function boot(h,check){
  const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-nobrowser-'));
  const saved={home:process.env.BACKPLANE_HOME,db:process.env.BACKPLANE_DESKTOP_BROWSER},load=Module._load;
  process.env.BACKPLANE_HOME=home;process.env.BACKPLANE_DESKTOP_BROWSER='1';const resources=process.resourcesPath;process.resourcesPath=home;
  Module._load=function(request,parent,...rest){if(Object.hasOwn(h.stubs,request))return h.stubs[request];return load.call(this,request,parent,...rest);};
  try{
    const main=require.resolve('../main.cjs');delete require.cache[main];require(main);
    h.ready();for(let i=0;i<50&&!h.calls.loadURL.length&&!h.calls.errors.length;i++)await new Promise(r=>setTimeout(r,10));
    assert.deepEqual(h.calls.errors,[],'Electron fixture must reach service connection without a shell startup error');
    await check();
  }finally{
    Module._load=load;process.resourcesPath=resources;
    for(const [k,v] of [['BACKPLANE_HOME',saved.home],['BACKPLANE_DESKTOP_BROWSER',saved.db]])if(v===undefined)delete process.env[k];else process.env[k]=v;
    fs.rmSync(home,{recursive:true,force:true});
  }
}

test('a browser start that never settles does not hold the UI',async()=>{
  const h=harness(true);
  await boot(h,async()=>{assert.deepEqual(h.calls.loadURL,['http://127.0.0.1:1'],'the UI loads first');assert.equal(h.calls.controllers,1,'the browser starts after it');});
});

test('missing browser endpoint leaves the UI loaded and retries automatically',async()=>{
 const h=harness();
 await boot(h,async()=>{
  assert.equal(h.calls.controllers,1);assert.equal(h.calls.handlers.has('desktop:browser-reconnect'),false);
  assert.deepEqual(h.calls.loadURL,['http://127.0.0.1:1']);
  const first=h.calls.timers.find(t=>!t.cancelled);assert.equal(first.ms,500);first.fn();
  await new Promise(r=>setImmediate(r));
  assert.equal(h.calls.controllers,2);assert.equal(h.calls.timers.at(-1).ms,1000);
  assert.deepEqual(h.calls.discover,[{browserReconnectOrigin:'http://127.0.0.1:1'}]);
  assert.deepEqual(h.calls.loadURL,['http://127.0.0.1:1'],'Browser retry must preserve the renderer');
 });
});

test('automatic discovery failure waits with backoff and never creates an unauthenticated controller',async()=>{
 const h=harness(false,{discover:async()=>{throw Error('Selected service authentication failed');}});
 await boot(h,async()=>{
  h.calls.timers[0].fn();await new Promise(r=>setImmediate(r));
  assert.equal(h.calls.controllers,1);assert.equal(h.calls.discover.length,1);
  assert.equal(h.calls.timers.at(-1).ms,1000);
 });
});

test('healthy controller keeps the existing guest across desktop renderer reloads',async()=>{
 const h=harness(false,{keepOpen:true});
 await boot(h,async()=>{
  assert.equal(h.calls.controllers,1);assert.equal(h.calls.timers.length,0);
  const w=h.stubs.electron.BrowserWindow.all[0];
  const event={sender:w.webContents,senderFrame:w.webContents.mainFrame={url:'http://127.0.0.1:1/'}};
  assert.equal((await h.calls.handlers.get('desktop:browser-state')(event)).phase,'ready');
  assert.equal(h.calls.controllers,1);
 });
});

test('main restores a captured URL once through its authenticated service and coalesces presentations',async()=>{
 const h=harness(false,{keepOpen:true});let opens=0,finish;
 h.ses.fetch=async(url,options)=>{
  if(url.endsWith('/auth/pair'))return{ok:true};
  assert.equal(url,'http://127.0.0.1:1/mcp/self');assert.equal(options.redirect,'error');
  assert.equal(options.headers.Authorization,'Bearer '+'a'.repeat(32));
  const request=JSON.parse(options.body);assert.equal(request.params.name,'preview_open');assert.equal(request.params.arguments.url,'https://example.test/retained');
  opens++;await new Promise(resolve=>{finish=resolve;});return{ok:true,json:async()=>({result:{isError:false}})};
 };
 await boot(h,async()=>{
  h.calls.controllerOptions[0].onRecover([{owner:'self',url:'https://example.test/retained'}]);
  const w=h.stubs.electron.BrowserWindow.all[0],event={sender:w.webContents,senderFrame:w.webContents.mainFrame={url:'http://127.0.0.1:1/'}},value={owner:'self',bounds:{x:0,y:0,width:100,height:100}};
  const present=h.calls.handlers.get('desktop:browser-present');const a=present(event,value),b=present(event,value);
  await new Promise(resolve=>setImmediate(resolve));assert.equal(opens,1);finish();await Promise.all([a,b]);await present(event,value);assert.equal(opens,1);
 });
});

test('a restore refusal is confined to the Browser presentation and leaves the private channel ready',async()=>{
 const h=harness(false,{keepOpen:true});let opens=0;
 h.ses.fetch=async url=>{if(url.endsWith('/auth/pair'))return{ok:true};opens++;return{ok:false};};
 await boot(h,async()=>{
  h.calls.controllerOptions[0].onRecover([{owner:'self',url:'https://example.test/retained'}]);
  const w=h.stubs.electron.BrowserWindow.all[0],event={sender:w.webContents,senderFrame:w.webContents.mainFrame={url:'http://127.0.0.1:1/'}},value={owner:'self',bounds:{x:0,y:0,width:100,height:100}};
  const present=h.calls.handlers.get('desktop:browser-present');const r=await present(event,value);assert.equal(r.phase,'error');assert.match(r.message,/could not be restored/);
  await present(event,value);assert.equal(opens,1);assert.equal((await h.calls.handlers.get('desktop:browser-state')(event)).phase,'ready');assert.equal(h.calls.timers.length,0);
 });
});
