import {createPathTraceView} from '../../src/web/solid-pathtrace-core.js';
let view,loadAbort,pendingCamera,cameraTimer,hidden=false,activeGeneration=0;
const camera=()=>{if(pendingCamera&&view){const value=pendingCamera;pendingCamera=null;view.setCamera(value);}cameraTimer=0;};
self.onmessage=async({data:{id,method,args}})=>{
 try{
  let result;
  if(method==='init'){
   args.canvas.clientWidth=args.width;args.canvas.clientHeight=args.height;
   view=await createPathTraceView({...args,onStatus:status=>self.postMessage({status})});view.pixelRatio=args.pixelRatio;result={ready:true};
  }else if(!view){if(method==='resize'||method==='visibility')return;throw Error('Original-model renderer is not ready');}
  else if(method==='load'){loadAbort?.abort();activeGeneration=args.generation;loadAbort=new AbortController();result=await view.load({...args,signal:loadAbort.signal});}
  else if(method==='cancel'){if(args.generation===activeGeneration){loadAbort?.abort();view.stop();}}
  else if(method==='resize')view.resizeCanvas(args.width,args.height,args.pixelRatio);
  else if(method==='camera'){pendingCamera=args;if(!cameraTimer)cameraTimer=setTimeout(camera,0);}
  else if(method==='interactive'){camera();view.setInteractive(args.on);}
  else if(method==='mode'){camera();view.setMode(args.mode,args.options);}
  else if(method==='components')result=view.setComponents(args);
  else if(method==='material-policy')result=view.setMaterialPolicy(args.policy);
  else if(method==='solder-preview'){camera();result=view.setSolderPreview(args.enabled);}
  else if(method==='present')result=view.present(args.denoised);
  else if(method==='selection')view.selectComponent(args.id);
  else if(method==='bounds')result=view.componentBounds(args.id);
  else if(method==='pick'){camera();result=view.pick(args.x,args.y);}
  else if(method==='visibility'){hidden=args.hidden;view.hidden=hidden;if(hidden)view.stop();else view.refresh();}
  else if(method==='status')result=view.diagnostics();
  else if(method==='dispose'){loadAbort?.abort();view.dispose();self.close();}
  if(id)self.postMessage({id,result});
 }catch(error){if(id)self.postMessage({id,error:String(error.message||error)});else self.postMessage({status:{phase:'error',reason:String(error.message||error),samples:0}});}
};
