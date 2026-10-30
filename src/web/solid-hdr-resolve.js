// Resolve traced radiance over each native pixel's actual source footprint.
// No display transfer function belongs in either intermediate pass.
const vertexShader='void main(){gl_Position=vec4(position.xy,0.0,1.0);}';
const footprint=`
 uniform vec2 inputSize;uniform vec2 outputSize;uniform vec2 pixelScale;
 void bounds(out ivec2 first,out ivec2 last,out vec2 lo,out vec2 hi){
  vec2 pixel=floor(gl_FragCoord.xy);lo=pixel*pixelScale;hi=(pixel+1.0)*pixelScale;
  first=ivec2(floor(lo));last=min(ivec2(ceil(hi))-1,ivec2(inputSize)-1);
 }
 float overlap(ivec2 p,vec2 lo,vec2 hi){vec2 d=max(vec2(0.0),min(hi,vec2(p)+1.0)-max(lo,vec2(p)));return d.x*d.y;}
`;
export const hdrAreaResolveShader=`
 uniform sampler2D map;${footprint}
 void main(){
  ivec2 first,last;vec2 lo,hi;bounds(first,last,lo,hi);vec4 total=vec4(0.0);float weight=0.0;
  // The CPU gate guarantees the entire footprint fits within 3x3 texels.
  for(int y=0;y<3;y++)for(int x=0;x<3;x++){
   ivec2 p=first+ivec2(x,y);if(any(greaterThan(p,last)))continue;float w=overlap(p,lo,hi);vec4 c=texelFetch(map,p,0);
   total+=vec4(c.rgb*c.a,c.a)*w;weight+=w;
  }
  gl_FragColor=vec4(total.a>0.0?total.rgb/total.a:vec3(0.0),total.a/weight);
 }
`;
const mixedGuideShader=`
 uniform sampler2D normalMap;uniform sampler2D depthMap;uniform sampler2D albedoMap;uniform mat4 inverseProjection;${footprint}
 vec3 viewAt(ivec2 p,float depth){vec2 uv=(vec2(p)+0.5)/inputSize;vec4 v=inverseProjection*vec4(uv*2.0-1.0,depth*2.0-1.0,1.0);return v.xyz/v.w;}
 void main(){
  ivec2 first,last;vec2 lo,hi;bounds(first,last,lo,hi);ivec2 center=min(ivec2(floor((lo+hi)*0.5)),ivec2(inputSize)-1);
  float depth=texelFetch(depthMap,center,0).r;vec3 normal=normalize(texelFetch(normalMap,center,0).xyz*2.0-1.0),point=viewAt(center,depth),albedo=texelFetch(albedoMap,center,0).rgb;float mixed=0.0;
  for(int y=0;y<3;y++)for(int x=0;x<3;x++){
   ivec2 p=first+ivec2(x,y);if(any(greaterThan(p,last))||overlap(p,lo,hi)<=0.0)continue;
   float d=texelFetch(depthMap,p,0).r;vec3 n=normalize(texelFetch(normalMap,p,0).xyz*2.0-1.0),da=texelFetch(albedoMap,p,0).rgb-albedo;
   bool background=depth>=1.0,otherBackground=d>=1.0;
   if(background!=otherBackground||(!background&&(dot(n,normal)<0.995||abs(dot(viewAt(p,d)-point,normal))>max(length(point)*0.00001,0.000002)||dot(da,da)>0.003)))mixed=1.0;
  }
  gl_FragColor=vec4(mixed,0.0,0.0,1.0);
 }
`;

export class HDRResolve {
 constructor(T,FullScreenQuad){
  this.T=T;
  const uniforms=()=>({inputSize:{value:new T.Vector2()},outputSize:{value:new T.Vector2()},pixelScale:{value:new T.Vector2()}});
  this.colorQuad=new FullScreenQuad(new T.ShaderMaterial({depthTest:false,depthWrite:false,toneMapped:false,uniforms:{...uniforms(),map:{value:null}},vertexShader,fragmentShader:hdrAreaResolveShader}));
  this.guideQuad=new FullScreenQuad(new T.ShaderMaterial({depthTest:false,depthWrite:false,toneMapped:false,uniforms:{...uniforms(),normalMap:{value:null},depthMap:{value:null},albedoMap:{value:null},inverseProjection:{value:new T.Matrix4()}},vertexShader,fragmentShader:mixedGuideShader}));
 }
 resize(width,height){
  if(width===this.width&&height===this.height)return;
  if(!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1)throw Error('Invalid HDR resolve output dimensions');
  this.target?.dispose();this.mixed?.dispose();const T=this.T;
  this.target=new T.WebGLRenderTarget(width,height,{type:T.FloatType,minFilter:T.NearestFilter,magFilter:T.NearestFilter,depthBuffer:false});
  this.mixed=new T.WebGLRenderTarget(width,height,{minFilter:T.NearestFilter,magFilter:T.NearestFilter,depthBuffer:false});
  this.target.texture.colorSpace=T.NoColorSpace;this.mixed.texture.colorSpace=T.NoColorSpace;
  this.width=width;this.height=height;this.mixedReady=false;
 }
 render(renderer,source,width,height){
  if(!Number.isInteger(source.width)||!Number.isInteger(source.height)||source.width<1||source.height<1)throw Error('Invalid HDR resolve input dimensions');
  if(!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1)throw Error('Invalid HDR resolve output dimensions');
  if(source.width/width>2||source.height/height>2){const error=Error('Waiting for a trace target at the current canvas size');error.code='HDR_RESOLVE_PENDING';throw error;}
  this.resize(width,height);this.inputWidth=source.width;this.inputHeight=source.height;this.mixedReady=false;
  const u=this.colorQuad.material.uniforms;u.map.value=source.texture;u.inputSize.value.set(source.width,source.height);u.outputSize.value.set(width,height);u.pixelScale.value.set(source.width/width,source.height/height);
  const previous=renderer.getRenderTarget();try{renderer.setRenderTarget(this.target);this.colorQuad.render(renderer);}finally{renderer.setRenderTarget(previous);}
  return this.target;
 }
 protectMixed(renderer,normal,depth,albedo,inverseProjection){
  const u=this.guideQuad.material.uniforms;u.inputSize.value.set(this.inputWidth,this.inputHeight);u.outputSize.value.set(this.width,this.height);u.pixelScale.value.set(this.inputWidth/this.width,this.inputHeight/this.height);u.normalMap.value=normal;u.depthMap.value=depth;u.albedoMap.value=albedo;u.inverseProjection.value.copy(inverseProjection);
  const previous=renderer.getRenderTarget();try{renderer.setRenderTarget(this.mixed);this.guideQuad.render(renderer);this.mixedReady=true;}finally{renderer.setRenderTarget(previous);}
  return this.mixed;
 }
 get bytes(){return this.target?this.width*this.height*20:0;}
 get maxTexelsPerPixel(){return 9;}
 dispose(){
  this.target?.dispose();this.mixed?.dispose();this.target=null;this.mixed=null;this.width=0;this.height=0;this.mixedReady=false;
  for(const quad of [this.colorQuad,this.guideQuad]){quad?.material.dispose();quad?.dispose();}this.colorQuad=null;this.guideQuad=null;
 }
}
