const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path');
const load=name=>import(require('node:url').pathToFileURL(path.resolve(__dirname,'../../src/web/'+name)).href);
function mediaFixture(){
 let recorder,resolvePicker,stops=0,time=0;const scheduled=new Map();let id=0;
 const track={addEventListener(){},stop(){stops++;}};
 class Recorder{static isTypeSupported(m){return m==='video/webm;codecs=vp8';}constructor(stream,opts){recorder=this;this.opts=opts;this.state='inactive';}start(slice){assert.equal(slice,500);this.state='recording';}stop(){if(this.state==='inactive')return;this.state='inactive';queueMicrotask(()=>this.onstop());}data(bytes){this.ondataavailable({data:new Blob([bytes])});}}
 const media={getDisplayMedia(opts){assert.equal(opts.audio,false);assert(!opts.video.deviceId);return new Promise(r=>resolvePicker=r);}};
 return{media,Recorder,now:()=>time,timers:{setTimeout(fn,ms){scheduled.set(++id,{fn,ms});return id;},setInterval(fn,ms){scheduled.set(++id,{fn,ms});return id;},clearTimeout(id){scheduled.delete(id);},clearInterval(id){scheduled.delete(id);}},select(){resolvePicker({getVideoTracks:()=>[track],getTracks:()=>[track]});},get recorder(){return recorder;},get stops(){return stops;},scheduled,advance(ms){time+=ms;for(const v of [...scheduled.values()])if(v.ms<=ms)v.fn();}};
}
const webm=Uint8Array.from([0x1a,0x45,0xdf,0xa3,1,2,3,4]);
test('screen recorder uses video-only picker, WebM and bounds explicit/track stop',async()=>{
 const {startScreenRecording}=await load('improvements-recording.js'),f=mediaFixture();const starting=startScreenRecording(f);assert(!f.recorder);f.select();const session=await starting;
 assert.equal(f.recorder.opts.mimeType,'video/webm;codecs=vp8');f.recorder.data(webm);session.stop();const result=await session.done;
 assert.equal(result.file.type,'video/webm');assert.deepEqual(new Uint8Array(await result.file.arrayBuffer()),webm);assert(f.stops>0);assert.equal(f.scheduled.size,0);
});
test('recorder expires at two minutes, cancels late picker tracks, and rejects oversized bytes',async()=>{
 const {startScreenRecording,RECORDING}=await load('improvements-recording.js');
 {const f=mediaFixture(),p=startScreenRecording(f);f.select();const r=await p;f.recorder.data(webm);f.advance(RECORDING.maxMs);assert.equal((await r.done).durationMs,RECORDING.maxMs);assert(f.stops);}
 {const f=mediaFixture(),abort=new AbortController(),p=startScreenRecording({...f,signal:abort.signal});abort.abort();f.select();await assert.rejects(p,/canceled/);assert(f.stops);}
 {const f=mediaFixture(),p=startScreenRecording({...f,limits:{maxBytes:4,maxMs:120000}});f.select();const r=await p;f.recorder.data(webm);await assert.rejects(r.done,/size limit/);assert.equal(f.scheduled.size,0);}
});
test('recording is hidden for phones, tablets and unsupported browser APIs',async()=>{
 const {recordingSupported}=await load('improvements-recording.js'),win={MediaRecorder(){},navigator:{userAgent:'Desktop',platform:'Linux',mediaDevices:{getDisplayMedia(){}}}};
 assert(recordingSupported(win));for(const userAgent of ['iPhone','Android','iPad'])assert(!recordingSupported({...win,navigator:{...win.navigator,userAgent}}));assert(!recordingSupported({...win,navigator:{...win.navigator,platform:'MacIntel',maxTouchPoints:5}}));assert(!recordingSupported({...win,MediaRecorder:null}));
});
test('desktop video storage is private, retained, capped, and rejects path/link/full-disk attacks',t=>{
 const {createRecordingStore,LIMITS}=require('../improvements-recording-store.cjs'),home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-recording-'));t.after(()=>fs.rmSync(home,{recursive:true,force:true}));const store=createRecordingStore(home),ids=['a','b','c'].map(x=>x.repeat(32));
 for(const id of ids){store.put(id,webm);assert(store.usage()<=LIMITS.diskBytes);}assert.equal(fs.readdirSync(store.dir).length,2);assert.equal(fs.statSync(store.dir).mode&0o777,0o700);assert.equal(fs.statSync(path.join(store.dir,ids[2]+'.webm')).mode&0o777,0o600);assert.deepEqual(store.read(ids[2]),Buffer.from(webm));
 assert.throws(()=>store.put('../secret',webm),/Invalid/);assert.throws(()=>store.put('d'.repeat(32),new Uint8Array(LIMITS.maxBytes+1)),/Invalid/);assert.throws(()=>store.put('d'.repeat(32),Uint8Array.from([1,2,3,4])),/WebM/);
 fs.utimesSync(path.join(store.dir,ids[2]+'.webm'),0,0);store.prune();assert(!fs.existsSync(path.join(store.dir,ids[2]+'.webm')));
 fs.symlinkSync('/dev/full',path.join(store.dir,'e'.repeat(32)+'.webm'));assert.throws(()=>store.prune(),/Unsafe/);fs.unlinkSync(path.join(store.dir,'e'.repeat(32)+'.webm'));
 store.clear();const faultFs=new Proxy(fs,{get(o,k){if(k==='writeSync')return()=>{throw Object.assign(Error('secret private path'),{code:'ENOSPC'});};return o[k];}}),bad=createRecordingStore(home,{fs:faultFs});assert.throws(()=>bad.put(ids[0],webm),/^Error: Could not keep/);assert.equal(fs.readdirSync(store.dir).length,0);
});
test('display capture chooses X11 explicitly, uses Wayland portal result, fences frames and handles cancel once',async()=>{
 const {installDisplayCapture}=require('../improvements-display-capture.cjs');
 for(const wayland of [false,true]){let handler,calls=0,choices=0;const frame={url:'https://hub/'},window={isDestroyed:()=>false,webContents:{mainFrame:frame}},sources=[{id:'selected',name:'PRIVATE_SOURCE'}];
 const electron={desktopCapturer:{getSources:async opts=>{calls++;assert.deepEqual(opts.types,['window','screen']);return sources;}}};
 installDisplayCapture({electron,session:{setDisplayMediaRequestHandler(h){handler=h;}},getWindow:()=>window,origin:()=> 'https://hub',wayland,choose:async()=>{choices++;return sources[0];}});
 const request={frame,userGesture:true,videoRequested:true,audioRequested:false,securityOrigin:'https://hub'};let result;
 await handler(request,x=>result=x);assert.equal(result.video.id,'selected');assert.equal(choices,wayland?0:1);
 for(const patch of [{frame:{}},{audioRequested:true},{userGesture:false},{securityOrigin:'https://evil'}]){let replies=0;await handler({...request,...patch},x=>{replies++;assert.equal(x,null);});assert.equal(replies,1);}assert.equal(calls,1);
 electron.desktopCapturer.getSources=async()=>{throw Error('cancel');};await handler(request,x=>assert.equal(x,null));
 }
});
test('video attachments require consent, retain stable keys and send exact file paths once on retry',async()=>{
 const {ImprovementsFlow}=await load('improvements-flow.js'),{ReportWriter}=await load('report-writer.js');
 for(const publicPath of [false,true]){const uploads=[],turns=[];let fail=true;const draft={text:'Exact 日本語',project:'p',writerProject:'p',includeRecording:true,recording:{id:'clip'},context:{}},video=new File([webm],'clip.webm',{type:'video/webm'});
 const rpc={request:async(m,p)=>{if(m==='thread.create')return{thread:'improvement-'+p.creationKey};if(m==='improvements.context')return{thread:p.sourceThread,accountBinding:'bound'};turns.push(p);if(fail){fail=false;throw Error('lost start');}return{ok:true};}};
 const common={rpc,save:async()=>{},upload:async(t,k,file)=>{uploads.push({t,k,file});return'/attachments/'+k+'-recording.webm';},select:async()=>{}};
 const flow=publicPath?new ReportWriter(common):new ImprovementsFlow(common),send=()=>publicPath?flow.start(draft,'Review only the attached video.',{video}):flow.send(draft,{video});await assert.rejects(send,/lost start/);await send();assert.equal(uploads.length,1);assert.equal(uploads[0].file,video);assert.deepEqual(turns[0],turns[1]);assert(turns[0].text.endsWith('[attachment: /attachments/'+uploads[0].k+'-recording.webm]'));assert.equal(turns[0].accountBinding,'bound');
 }
 const {ImprovementsFlow:Flow}=await load('improvements-flow.js');let called=0;const denied=new Flow({rpc:{request:async()=>called++},save:async()=>{},upload:async()=>called++,select(){}});await assert.rejects(denied.send({text:'text',project:'p',includeRecording:false},{video:new Blob([webm])}),/selected recording/);assert.equal(called,0);
});
test('browser origin-private recording quota, cleanup and exact reads never fall back to network',async()=>{
 const {RecordingStore}=await load('improvements-recording-store.js');const files=new Map();let full=false,aborts=0;
 const dir={async *entries(){for(const [name,file]of files)yield[name,{kind:'file',getFile:async()=>file}];},async removeEntry(name){files.delete(name);},async getFileHandle(name,opts){if(!files.has(name)&&!opts?.create)throw Error('missing');return{getFile:async()=>files.get(name),async createWritable(){return{async write(blob){if(full)throw Error('quota');files.set(name,new File([blob],name,{lastModified:Date.now()}));},async close(){},async abort(){aborts++;files.delete(name);}};}};}};
 const store=new RecordingStore({bridge:null,storage:{getDirectory:async()=>({getDirectoryHandle:async name=>{assert.equal(name,'diagnostics-recordings');return dir;}})}}),clip=new File([webm],'clip.webm',{type:'video/webm'});
 const refs=[];for(let i=0;i<3;i++)refs.push(await store.put(clip));assert.equal(files.size,2);assert.deepEqual(new Uint8Array(await (await store.get(refs[2])).arrayBuffer()),webm);assert.equal((await store.get(refs[2])).type,'video/webm');
 const [old]=files.keys();files.set(old,new File([webm],old,{lastModified:0}));await store.get(refs[2]);assert(!files.has(old));
 full=true;await assert.rejects(store.put(clip),/Could not save/);assert.equal(aborts,1);assert.equal(files.size,1);await store.clear();assert.equal(files.size,0);
});
test('Electron permission gate and concurrent/navigation fences never grant a foreign or extra stream',async()=>{
 const {installDisplayCapture,displayCapturePermission}=require('../improvements-display-capture.cjs');const frame={url:'https://hub/'},window={isDestroyed:()=>false,webContents:{mainFrame:frame}},origin='https://hub';
 assert(displayCapturePermission(window.webContents,'display-capture',origin,{isMainFrame:true,mediaTypes:['video']},window,origin));for(const [p,d]of [['media',{isMainFrame:true}],['display-capture',{isMainFrame:false}],['display-capture',{isMainFrame:true,mediaTypes:['audio']}]])assert(!displayCapturePermission(window.webContents,p,origin,d,window,origin));
 const legacy={isMainFrame:true,mediaTypes:[]};assert(displayCapturePermission(window.webContents,'media',origin,legacy,window,origin,'request'));
 for(const phase of ['check','request']){
  for(const mediaTypes of [undefined,null,'',{},['video'],['audio'],['audio','video'],['unknown']])assert.equal(displayCapturePermission(window.webContents,'media',origin,{isMainFrame:true,mediaTypes},window,origin,phase),false);
  for(const mediaType of ['audio','video','unknown'])assert.equal(displayCapturePermission(window.webContents,'media',origin,{isMainFrame:true,mediaType},window,origin,phase),false);
 }
 assert.equal(displayCapturePermission(window.webContents,'media',origin,legacy,window,origin),false);
 for(const [wc,url,d]of [[{},origin,legacy],[window.webContents,'https://foreign',legacy],[window.webContents,origin,{...legacy,isMainFrame:false}]])assert.equal(displayCapturePermission(wc,'media',url,d,window,origin,'request'),false);
 let handler,release,calls=0;installDisplayCapture({electron:{desktopCapturer:{getSources:()=>{calls++;return new Promise(r=>release=r);}}},session:{setDisplayMediaRequestHandler(h){handler=h;}},getWindow:()=>window,origin:()=>origin,wayland:true});const request={frame,userGesture:true,videoRequested:true,audioRequested:false,securityOrigin:origin};
 let first;const active=handler(request,x=>first=x);await handler(request,x=>assert.equal(x,null));await handler(request,x=>assert.equal(x,null));assert.equal(calls,1);window.webContents.mainFrame={url:'https://hub/'};release([{id:'never-granted'}]);await active;assert.equal(first,null);
});
