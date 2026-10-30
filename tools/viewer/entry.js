// Exact, local vendor boundary. The application never imports from a CDN.
export * as THREE from 'three';
export {GLTFLoader} from 'three/addons/loaders/GLTFLoader.js';
export {RoomEnvironment} from 'three/addons/environments/RoomEnvironment.js';
export {RectAreaLightUniformsLib} from 'three/addons/lights/RectAreaLightUniformsLib.js';
export {FullScreenQuad} from 'three/addons/postprocessing/Pass.js';
export {WebGLPathTracer,PhysicalCamera,GradientEquirectTexture,DenoiseMaterial} from 'three-gpu-pathtracer';
import {GenerateMeshBVHWorker} from 'three-mesh-bvh/worker';
import {WorkerBase} from 'three-mesh-bvh/src/workers/utils/WorkerBase.js';
export class SceneBVHWorker extends WorkerBase {
  constructor(url){super(new Worker(url,{type:'module',name:'backplane-viewer-bvh'}));this.name='BackplaneSceneBVHWorker';}
  runTask(...args){return GenerateMeshBVHWorker.prototype.runTask.apply(this,args);}
}
export const versions={three:'0.181.1',pathtracer:'0.0.24',bvh:'0.9.5'};
