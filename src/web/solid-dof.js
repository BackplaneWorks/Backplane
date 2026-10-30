// Optional, depth-dependent lens presentation. Off unless explicitly enabled.
// Uses the scene's real depth texture, never a uniform blur of the canvas.
const VS = `attribute vec2 p; varying vec2 uv; void main(){uv=p*0.5+0.5;gl_Position=vec4(p,0.0,1.0);}`;
const FS = `precision highp float;
varying vec2 uv; uniform sampler2D color; uniform sampler2D depth;
uniform vec2 size; uniform vec2 clip; uniform vec2 axis; uniform float focus; uniform float focal; uniform float aperture;
vec3 linear(vec3 c){return mix(c/12.92,pow((c+0.055)/1.055,vec3(2.4)),step(vec3(0.04045),c));}
vec3 srgb(vec3 c){return mix(c*12.92,1.055*pow(max(c,vec3(0.0)),vec3(1.0/2.4))-0.055,step(vec3(0.0031308),c));}
float distanceAt(vec2 p){float z=texture2D(depth,p).r*2.0-1.0;return (2.0*clip.x*clip.y)/(clip.y+clip.x-z*(clip.y-clip.x))/100.0;}
float radius(float z){return min(8.0,abs(focal*focal*(z-focus)/(max(aperture*z*abs(focus-focal),0.01)))*size.y/24.0*0.5);}
void main(){
  float z=distanceAt(uv),r=radius(z);vec4 center=texture2D(color,uv);
  if(r<0.5||center.a<0.01){gl_FragColor=center;return;}
  vec4 sum=vec4(linear(center.rgb),center.a);float weight=1.0;
  for(int i=0;i<16;i++){
    float offset=float(i)-7.5;
    vec2 p=uv+axis*offset*r/7.5/size;
    float tapZ=distanceAt(p);vec4 tap=texture2D(color,p);
    // Depth-aware separable lens approximation: linear filtering keeps fine
    // silk/lead edges smooth without noisy sparse disc samples.
    float w=exp(-offset*offset/20.0)*tap.a*step(abs(tapZ-z),max(z*0.03,0.02)+abs(tapZ-focus)*0.5);
    sum+=vec4(linear(tap.rgb),tap.a)*w;weight+=w;
  }
  gl_FragColor=vec4(srgb(sum.rgb/weight),sum.a/weight);
}`;
function program(gl,vs,fs){
  const sh=(type,text)=>{const s=gl.createShader(type);gl.shaderSource(s,text);gl.compileShader(s);if(!gl.getShaderParameter(s,gl.COMPILE_STATUS))throw Error(gl.getShaderInfoLog(s));return s;};
  const a=sh(gl.VERTEX_SHADER,vs),b=sh(gl.FRAGMENT_SHADER,fs),p=gl.createProgram();gl.attachShader(p,a);gl.attachShader(p,b);gl.linkProgram(p);gl.deleteShader(a);gl.deleteShader(b);
  if(!gl.getProgramParameter(p,gl.LINK_STATUS))throw Error(gl.getProgramInfoLog(p));return p;
}
export class DepthOfField {
  constructor(gl) {
    this.gl=gl;this.available=false;this.width=0;this.height=0;
    if(!gl.getExtension('WEBGL_depth_texture')){this.reason='depth textures unavailable';return;}
    if(!gl.getShaderPrecisionFormat(gl.FRAGMENT_SHADER,gl.HIGH_FLOAT)?.precision){this.reason='fragment high precision unavailable';return;}
    try{
      this.program=program(gl,VS,FS);this.vertex=gl.createBuffer();gl.bindBuffer(gl.ARRAY_BUFFER,this.vertex);gl.bufferData(gl.ARRAY_BUFFER,new Float32Array([-1,-1,1,-1,-1,1,1,1]),gl.STATIC_DRAW);
      this.frame=gl.createFramebuffer();this.scratchFrame=gl.createFramebuffer();this.scratch=gl.createTexture();this.color=gl.createTexture();this.depth=gl.createTexture();this.available=true;
    }catch(e){this.reason=String(e.message||e);this.dispose();}
  }
  begin(w,h){
    if(!this.available)return false;const gl=this.gl;
    // At most 1536 pixels on the long side: two RGBA8+depth16 <=24 MiB.
    const scale=Math.min(1,1536/Math.max(w,h));w=Math.max(1,Math.round(w*scale));h=Math.max(1,Math.round(h*scale));
    if(w!==this.width||h!==this.height){
      gl.bindFramebuffer(gl.FRAMEBUFFER,this.frame);
      for(const [texture,format,type,attachment]of [[this.color,gl.RGBA,gl.UNSIGNED_BYTE,gl.COLOR_ATTACHMENT0],[this.depth,gl.DEPTH_COMPONENT,gl.UNSIGNED_SHORT,gl.DEPTH_ATTACHMENT]]){
        gl.bindTexture(gl.TEXTURE_2D,texture);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,format===gl.RGBA?gl.LINEAR:gl.NEAREST);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,format===gl.RGBA?gl.LINEAR:gl.NEAREST);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);
        gl.texImage2D(gl.TEXTURE_2D,0,format,w,h,0,format,type,null);gl.framebufferTexture2D(gl.FRAMEBUFFER,attachment,gl.TEXTURE_2D,texture,0);
      }
      if(gl.checkFramebufferStatus(gl.FRAMEBUFFER)!==gl.FRAMEBUFFER_COMPLETE){this.reason='depth framebuffer incomplete';this.dispose();gl.bindFramebuffer(gl.FRAMEBUFFER,null);console.info(`3D depth of field disabled: ${this.reason}`);return false;}
      gl.bindFramebuffer(gl.FRAMEBUFFER,this.scratchFrame);gl.bindTexture(gl.TEXTURE_2D,this.scratch);
      gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);
      gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,w,h,0,gl.RGBA,gl.UNSIGNED_BYTE,null);gl.framebufferTexture2D(gl.FRAMEBUFFER,gl.COLOR_ATTACHMENT0,gl.TEXTURE_2D,this.scratch,0);
      if(gl.checkFramebufferStatus(gl.FRAMEBUFFER)!==gl.FRAMEBUFFER_COMPLETE){this.reason='lens framebuffer incomplete';this.dispose();gl.bindFramebuffer(gl.FRAMEBUFFER,null);console.info(`3D depth of field disabled: ${this.reason}`);return false;}
      this.width=w;this.height=h;
    }
    gl.bindFramebuffer(gl.FRAMEBUFFER,this.frame);gl.viewport(0,0,w,h);return true;
  }
  finish(w,h,near,far,focus,focal,aperture){
    const gl=this.gl,p=this.program;gl.bindFramebuffer(gl.FRAMEBUFFER,null);gl.viewport(0,0,w,h);gl.disable(gl.DEPTH_TEST);gl.useProgram(p);
    gl.bindBuffer(gl.ARRAY_BUFFER,this.vertex);const a=gl.getAttribLocation(p,'p');gl.enableVertexAttribArray(a);gl.vertexAttribPointer(a,2,gl.FLOAT,false,0,0);
    for(const [i,name,texture]of [[0,'color',this.color],[1,'depth',this.depth]]){gl.activeTexture(gl.TEXTURE0+i);gl.bindTexture(gl.TEXTURE_2D,texture);gl.uniform1i(gl.getUniformLocation(p,name),i);}
    gl.uniform2f(gl.getUniformLocation(p,'size'),this.width,this.height);gl.uniform2f(gl.getUniformLocation(p,'clip'),near,far);
    for(const [name,value]of [['focus',focus],['focal',focal],['aperture',aperture]])gl.uniform1f(gl.getUniformLocation(p,name),value);
    gl.bindFramebuffer(gl.FRAMEBUFFER,this.scratchFrame);gl.viewport(0,0,this.width,this.height);gl.uniform2f(gl.getUniformLocation(p,'axis'),1,0);gl.drawArrays(gl.TRIANGLE_STRIP,0,4);
    gl.bindFramebuffer(gl.FRAMEBUFFER,null);gl.viewport(0,0,w,h);gl.activeTexture(gl.TEXTURE0);gl.bindTexture(gl.TEXTURE_2D,this.scratch);gl.uniform2f(gl.getUniformLocation(p,'axis'),0,1);gl.drawArrays(gl.TRIANGLE_STRIP,0,4);gl.disableVertexAttribArray(a);gl.activeTexture(gl.TEXTURE0);
  }
  dispose(){const gl=this.gl;if(this.frame)gl.deleteFramebuffer(this.frame);if(this.scratchFrame)gl.deleteFramebuffer(this.scratchFrame);if(this.scratch)gl.deleteTexture(this.scratch);if(this.color)gl.deleteTexture(this.color);if(this.depth)gl.deleteTexture(this.depth);if(this.vertex)gl.deleteBuffer(this.vertex);if(this.program)gl.deleteProgram(this.program);this.available=false;}
}
