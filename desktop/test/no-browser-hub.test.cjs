'use strict';
// The desktop app loads its UI from a hub that does not serve the in-app
// browser (started by systemd, --headless or an older install): the browser
// reads as unavailable, nothing else stops. main.cjs runs against a stubbed
// Electron, a stubbed service and a browser controller that cannot connect.
const {test}=require('node:test'),assert=require('node:assert/strict'),Module=require('node:module'),path=require('node:path'),os=require('node:os'),fs=require('node:fs');

function harness(hang=false,{discover,keepOpen=false,token}={}){
  const calls={loadURL:[],sent:[],errors:[],handlers:new Map(),controllers:0,controllerOptions:[],discover:[],tokenReads:0,token:'a'.repeat(32)};
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
  const ses={setPermissionRequestHandler(){},setPermissionCheckHandler(){},webRequest:{onHeadersReceived(){}},
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
  return{calls,ready,stubs:{electron,'./service.cjs':{Service},'./browser-controller.cjs':controller},ses};
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

test('the app loads its UI from a hub without the in-app browser, and Reconnect tries again',async()=>{
  const h=harness(),home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-nobrowser-'));
  const saved={home:process.env.BACKPLANE_HOME,db:process.env.BACKPLANE_DESKTOP_BROWSER},load=Module._load;
  process.env.BACKPLANE_HOME=home;process.env.BACKPLANE_DESKTOP_BROWSER='1';const resources=process.resourcesPath;process.resourcesPath=home;
  Module._load=function(request,parent,...rest){if(Object.hasOwn(h.stubs,request))return h.stubs[request];return load.call(this,request,parent,...rest);};
  try{
    const main=require.resolve('../main.cjs');delete require.cache[main];require(main);
    h.ready();for(let i=0;i<50&&!h.calls.loadURL.length&&!h.calls.errors.length;i++)await new Promise(r=>setTimeout(r,10));
    assert.deepEqual(h.calls.errors,[],'Electron fixture must reach service connection without a shell startup error');
    assert.deepEqual(h.calls.loadURL,['http://127.0.0.1:1'],'the UI loads although the browser is unavailable');
    assert(!h.calls.sent.some(([ch,v])=>ch==='desktop:connection'&&v.phase==='error'),'no startup error screen');
    assert.deepEqual(h.calls.errors,[]);
    assert.equal(h.calls.controllers,1);
    // Reconnect makes a new controller once the first one closed
    const trusted={sender:null,senderFrame:{url:'http://127.0.0.1:1/'}};
    const w=h.stubs.electron.BrowserWindow.all[0];trusted.sender=w.webContents;trusted.senderFrame=w.webContents.mainFrame={url:'http://127.0.0.1:1/'};
    const state=await h.calls.handlers.get('desktop:browser-reconnect')(trusted);
    assert.equal(h.calls.controllers,2);assert.equal(state.phase,'error');
    // the panel asks to be shown: it hears the browser is unavailable, as an error the panel offers Reconnect for
    const shown=await h.calls.handlers.get('desktop:browser-present')(trusted,{owner:'t1',bounds:{x:0,y:0,width:100,height:100}});
    assert.deepEqual(shown,{shown:false,phase:'error',message:'Browser unavailable'});
  }finally{
    Module._load=load;process.resourcesPath=resources;
    for(const [k,v] of [['BACKPLANE_HOME',saved.home],['BACKPLANE_DESKTOP_BROWSER',saved.db]])if(v===undefined)delete process.env[k];else process.env[k]=v;
    fs.rmSync(home,{recursive:true,force:true});
  }
});

function reconnect(h){
  const w=h.stubs.electron.BrowserWindow.all[0];
  w.webContents.mainFrame={url:'http://127.0.0.1:1/'};
  return h.calls.handlers.get('desktop:browser-reconnect')({sender:w.webContents,senderFrame:w.webContents.mainFrame});
}

test('Browser explicit reconnect binds a fresh version and token without replay or UI navigation',async()=>{
  const h=harness(false,{discover:async()=>({origin:'http://127.0.0.1:1',version:'replacement'})});
  await boot(h,async()=>{
    assert.equal(h.calls.controllers,1);assert.deepEqual(h.calls.discover,[],'automatic startup uses its original strict service binding');
    const first=h.calls.controllerOptions[0];assert.equal(first.version,'fixture');assert.equal(first.token,'a'.repeat(32));h.calls.token='b'.repeat(32);
    first.unknownState.latch('review');
    await reconnect(h);
    assert.deepEqual(h.calls.discover,[{browserReconnectOrigin:'http://127.0.0.1:1'}]);
    assert.equal(h.calls.controllers,2);const fresh=h.calls.controllerOptions[1];
    assert.equal(fresh.version,'replacement');assert.equal(fresh.origin,first.origin);
    assert.equal(fresh.token,'b'.repeat(32));assert.equal(h.calls.tokenReads,3);
    assert.equal(fresh.unknownState,first.unknownState);assert.equal(fresh.unknownState.has('review'),true);
    assert.deepEqual(h.calls.loadURL,['http://127.0.0.1:1']);assert.equal(h.stubs.electron.BrowserWindow.all.length,1);
  });
});

test('Browser reconnect failure creates no controller and does not retry discovery',async()=>{
  const h=harness(false,{discover:async()=>{throw new Error('Selected service authentication failed');}});
  await boot(h,async()=>{
    assert.equal((await reconnect(h)).phase,'error');
    assert.equal(h.calls.discover.length,1);assert.equal(h.calls.controllers,1);assert.equal(h.calls.tokenReads,2);
    assert.deepEqual(h.calls.loadURL,['http://127.0.0.1:1']);
    const shown=await h.calls.handlers.get('desktop:browser-state')({sender:h.stubs.electron.BrowserWindow.all[0].webContents,senderFrame:h.stubs.electron.BrowserWindow.all[0].webContents.mainFrame});
    assert.equal(shown.phase,'error');
  });
});

test('failure before controller creation is queryable and one explicit reconnect recovers',async()=>{
  let failed=false;
  const h=harness(false,{keepOpen:true,token:calls=>{
    if(calls.tokenReads===2){failed=true;throw Error('Synthetic credential read failure');}
    return calls.token;
  }});
  await boot(h,async()=>{
    assert(failed);assert.equal(h.calls.controllers,0);
    const w=h.stubs.electron.BrowserWindow.all[0];
    w.webContents.mainFrame={url:'http://127.0.0.1:1/'};
    const state=await h.calls.handlers.get('desktop:browser-state')({sender:w.webContents,senderFrame:w.webContents.mainFrame});
    assert.equal(state.phase,'error');assert.match(state.message,/Reconnect/);
    assert(h.calls.sent.some(([ch,value])=>ch==='desktop:browser-state'&&value.phase==='error'));
    assert.equal((await reconnect(h)).phase,'ready');assert.equal(h.calls.controllers,1);
    assert.deepEqual(h.calls.loadURL,['http://127.0.0.1:1']);
  });
});

test('Browser concurrent explicit reconnects share one discovery and one fresh controller',async()=>{
  let release;const binding=new Promise(resolve=>{release=resolve;});
  const h=harness(false,{discover:()=>binding});
  await boot(h,async()=>{
    const first=reconnect(h),second=reconnect(h);
    assert.equal(h.calls.discover.length,1);assert.equal(h.calls.controllers,1);
    release({origin:'http://127.0.0.1:1',version:'replacement'});
    await Promise.all([first,second]);assert.equal(h.calls.controllers,2);assert.equal(h.calls.tokenReads,3);
  });
});

test('Browser reconnect refuses untrusted IPC and leaves a live controller alone',async()=>{
  const h=harness(false,{keepOpen:true});
  await boot(h,async()=>{
    await assert.rejects(()=>h.calls.handlers.get('desktop:browser-reconnect')({sender:{},senderFrame:{url:'http://127.0.0.1:1/'}}),/Invalid Browser sender/);
    const state=await reconnect(h);assert.equal(state.phase,'ready');
    assert.deepEqual(h.calls.discover,[]);assert.equal(h.calls.controllers,1);assert.equal(h.calls.tokenReads,2);
  });
});
