'use strict';
// Source names/thumbnails are local picker UI only. Never log or persist them.
const escape=s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
function chooseSource(electron,parent,sources){return new Promise(resolve=>{
 const picker=new electron.BrowserWindow({parent,modal:true,width:720,height:600,minWidth:360,minHeight:300,title:'Choose a screen or window to record',webPreferences:{sandbox:true,contextIsolation:true,nodeIntegration:false,partition:'backplane-recording-picker'}});
 let settled=false;const finish=value=>{if(settled)return;settled=true;resolve(value);if(!picker.isDestroyed())picker.close();};
 picker.on('closed',()=>finish(null));picker.webContents.setWindowOpenHandler(()=>({action:'deny'}));
 picker.webContents.on('will-navigate',(event,url)=>{event.preventDefault();const m=/^backplane-capture:\/\/select\/(\d+)$/.exec(url);if(m&&Number(m[1])<sources.length)finish(sources[Number(m[1])]);});
 const html='<!doctype html><meta http-equiv="Content-Security-Policy" content="default-src &#39;none&#39;; img-src data:; style-src &#39;unsafe-inline&#39;"><meta name="viewport" content="width=device-width"><style>body{background:#111410;color:#eceee8;font:16px sans-serif;padding:16px}main{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:12px}a{display:block;color:inherit;border:1px solid #53634c;padding:10px;overflow-wrap:anywhere}img{width:100%;height:150px;object-fit:contain}a:focus{outline:3px solid #cee66b}</style><p>Choose what to record. The recording can show private information. Close this window to cancel.</p><main>'+sources.map((s,i)=>'<a href="backplane-capture://select/'+i+'"><img alt="" src="'+escape(s.thumbnail?.toDataURL()||'')+'"><span>'+escape(s.name)+'</span></a>').join('')+'</main>';
 picker.loadURL('data:text/html;charset=utf-8,'+encodeURIComponent(html)).catch(()=>finish(null));
 });}
function displayCapturePermission(wc,permission,url,details,window,origin){
 try{return permission==='display-capture'&&wc===window?.webContents&&!!origin&&new URL(url).origin===origin&&details?.isMainFrame===true&&!details.mediaTypes?.includes('audio');}catch{return false;}
}
function installDisplayCapture({electron,session,getWindow,origin,choose=chooseSource,wayland=process.platform==='linux'&&(!!process.env.WAYLAND_DISPLAY||process.env.XDG_SESSION_TYPE==='wayland')}){
 let pending=false;
 session.setDisplayMediaRequestHandler(async(request,callback)=>{
  let replied=false,claimed=false;const reply=value=>{if(replied)return;replied=true;try{callback(value?{video:value}:null);}catch{}};
  const parent=getWindow(),frame=request.frame,selectedOrigin=origin();
  const current=()=>parent&&!parent.isDestroyed()&&parent===getWindow()&&frame===parent.webContents.mainFrame&&frame&&!frame.detached&&origin()===selectedOrigin&&new URL(frame.url).origin===selectedOrigin;
  try{
   if(pending||!request.userGesture||!request.videoRequested||request.audioRequested||!selectedOrigin||new URL(request.securityOrigin).origin!==selectedOrigin||!current()){reply(null);return;}
   pending=true;claimed=true;
   const sources=await electron.desktopCapturer.getSources({types:['window','screen'],thumbnailSize:wayland?{width:0,height:0}:{width:320,height:180},fetchWindowIcons:false});
   if(!current()||!sources.length){reply(null);return;}
   // PipeWire's getSources opens the system portal picker and returns only
   // the source it authorized. X11 always gets our explicit selection UI.
   const selected=wayland?(sources.length===1?sources[0]:null):await choose(electron,parent,sources.slice(0,128));
   reply(current()?selected:null);
  }catch{reply(null);}finally{if(claimed)pending=false;}
 });
}
module.exports={installDisplayCapture,chooseSource,displayCapturePermission};
