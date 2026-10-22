'use strict';
// The desktop app loads its UI from a hub that does not serve the in-app
// browser (started by systemd, --headless or an older install): the browser
// reads as unavailable, nothing else stops. main.cjs runs against a stubbed
// Electron, a stubbed service and a browser controller that cannot connect.
const {test}=require('node:test'),assert=require('node:assert/strict'),Module=require('node:module'),path=require('node:path'),os=require('node:os'),fs=require('node:fs');

function harness(){
  const calls={loadURL:[],sent:[],errors:[],handlers:new Map(),controllers:0};
  let ready;const whenReady=new Promise(r=>ready=r);
  const events=()=>{const on=new Map();return{on(e,f){on.set(e,f);return this;},once(e,f){on.set(e,f);return this;},emit(e,...a){on.get(e)?.(...a);}};};
  class BrowserWindow{
    constructor(){Object.assign(this,events());const w=this;
      this.webContents={...events(),send:(ch,v)=>calls.sent.push([ch,v]),setWindowOpenHandler(){},getZoomFactor:()=>1,isDestroyed:()=>false,focus(){}};
      BrowserWindow.all.push(this);setImmediate(()=>w.emit('ready-to-show'));}
    show(){}isDestroyed(){return false;}isMinimized(){return false;}getContentBounds(){return{width:800,height:600};}
    loadFile(){return Promise.resolve();}
    loadURL(url){calls.loadURL.push(url);return Promise.resolve();}
    static getAllWindows(){return BrowserWindow.all;}
  }
  BrowserWindow.all=[];
  const ses={setPermissionRequestHandler(){},setPermissionCheckHandler(){},webRequest:{onHeadersReceived(){}},
    fetch:async url=>({ok:url.endsWith('/auth/pair')})};
  const electron={app:{isPackaged:false,setPath(){},setName(){},requestSingleInstanceLock:()=>true,on(){},quit(){},whenReady:()=>whenReady},
    BrowserWindow,Menu:{setApplicationMenu(){},buildFromTemplate:x=>x},dialog:{showErrorBox:(a,b)=>calls.errors.push(b)},
    ipcMain:{handle:(ch,f)=>calls.handlers.set(ch,f)},session:{fromPartition:()=>ses},shell:{},webContents:{getFocusedWebContents:()=>null}};
  class Service{async start(){return{origin:'http://127.0.0.1:1',version:'fixture'};}async token(){return'a'.repeat(32);}}
  // the hub has no /desktop/browser/v1: the controller's start fails and it closes
  const controller={createDesktopBrowserController:({onState})=>{calls.controllers++;let closed=false,state={phase:'connecting'};
    return{async start(){closed=true;state={phase:'error',message:'Browser connection unavailable'};onState(state);},
      state:()=>state,isClosed:()=>closed,close(){closed=true;},hide(){},present:async()=>({shown:false}),edit(){}};}};
  return{calls,ready,stubs:{electron,'./service.cjs':{Service},'./browser-controller.cjs':controller},ses};
}

test('the app loads its UI from a hub without the in-app browser, and Reconnect tries again',async()=>{
  const h=harness(),home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-nobrowser-'));
  const saved={home:process.env.BACKPLANE_HOME,db:process.env.BACKPLANE_DESKTOP_BROWSER},load=Module._load;
  process.env.BACKPLANE_HOME=home;process.env.BACKPLANE_DESKTOP_BROWSER='1';const resources=process.resourcesPath;process.resourcesPath=home;
  Module._load=function(request,parent,...rest){if(Object.hasOwn(h.stubs,request))return h.stubs[request];return load.call(this,request,parent,...rest);};
  try{
    const main=require.resolve('../main.cjs');delete require.cache[main];require(main);
    h.ready();for(let i=0;i<50&&!h.calls.loadURL.length;i++)await new Promise(r=>setTimeout(r,10));
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
