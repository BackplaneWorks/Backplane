// The compact Solid wire retains base colour, but no material IDs, textures,
// roughness, opacity or provenance. These are presentation estimates, never
// measured material data. Keep authored colours and all geometry untouched.
// glTF baseColorFactor is linear RGB; glb.bend quantizes it without a transfer:
// https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#materials
export function materialEstimate(r, g, b) {
  const hi = Math.max(r, g, b), lo = Math.min(r, g, b);
  let kind = "dielectric", roughness = 0.55, metalness = 0, coat = 0;
  if (hi < 0.12 && hi - lo < Math.max(0.015, hi * 0.45)) { kind = "polymer"; roughness = 0.63; coat = 0.12; }
  else if (lo > 0.9) { kind = "silk-or-white"; roughness = 0.82; }
  else if (r > 0.18 && g > r * 0.55 && g < r * 1.02 && b < g * 0.45) {
    kind = "copper-or-gold"; roughness = 0.3; metalness = 0.86;
  } else if (hi - lo < hi * 0.16 && lo > 0.18 && hi < 0.9) {
    kind = "metal-or-gray"; roughness = 0.28; metalness = 0.82;
  } else if (r > b * 1.25 && g > b * 1.12 && r > 0.06) {
    kind = "ceramic-or-substrate"; roughness = 0.72;
  } else if (hi - lo > Math.max(0.015, hi * 0.28)) { kind = "mask-or-color"; roughness = 0.36; coat = 0.35; }
  return { kind, roughness, metalness, coat, source: "color-estimate" };
}

// Estimate once per vertex on the GPU. Progressive prefixes share their source
// arrays, so material presentation adds no fourth CPU/GPU array or cache.
export const vertexShader = `attribute vec3 p; attribute vec3 n; attribute vec3 c;
uniform mat4 m; varying vec3 vc; varying vec3 vn; varying vec3 vp; varying vec3 vm;
vec3 estimate(vec3 c) {
  float hi=max(c.r,max(c.g,c.b)), lo=min(c.r,min(c.g,c.b));
  if(hi<0.12 && hi-lo<max(0.015,hi*0.45)) return vec3(0.63,0.0,0.12);
  if(lo>0.9) return vec3(0.82,0.0,0.0);
  if(c.r>0.18 && c.g>c.r*0.55 && c.g<c.r*1.02 && c.b<c.g*0.45) return vec3(0.3,0.86,0.0);
  if(hi-lo<hi*0.16 && lo>0.18 && hi<0.9) return vec3(0.28,0.82,0.0);
  if(c.r>c.b*1.25 && c.g>c.b*1.12 && c.r>0.06) return vec3(0.72,0.0,0.0);
  if(hi-lo>max(0.015,hi*0.28)) return vec3(0.36,0.0,0.35);
  return vec3(0.55,0.0,0.0);
}
void main() { gl_Position=m*vec4(p,1.0); vc=c; vn=n; vp=p; vm=estimate(c); }`;

export const fragmentShader = `
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
varying vec3 vc; varying vec3 vn; varying vec3 vp; varying vec3 vm;
uniform vec3 eye; uniform vec3 right; uniform vec3 up; uniform vec3 forward;
uniform float exposure; uniform float orthographic; uniform vec3 thermal;
// a probed part (hon) lights the model within its box on the board (hl:
// x0 y0 x1 y1, hz: its side's heights) and dims the rest (cross-probe)
uniform vec4 hl; uniform vec2 hz; uniform float hon;
// a board's own faces (bz: on, bottom, top in model units, 10 um) carry
// its soldermask: the person chose a glossy mask (2026-10-31), as the
// original-model Studio draws it (solid-material-policy.js). The compact
// wire has no material roles, so a mask is a non-metal, non-silk face
// lying flat within 120 um of either board face (KiCad puts the mask
// 40-50 um outside the body, the silk under 100 um); parts stand above it.
uniform vec3 bz;
const float PI = 3.14159265;
vec3 fresnel(vec3 f0, float h) { return f0 + (1.0 - f0) * pow(1.0 - h, 5.0); }
vec3 light(vec3 N, vec3 V, vec3 L, vec3 base, vec3 f0, float rough, float metal) {
  vec3 H = normalize(V + L);
  float nl = max(dot(N,L), 0.0), nv = max(dot(N,V), 0.001), nh = max(dot(N,H), 0.0);
  float a = rough * rough, a2 = a * a, d = nh * nh * (a2 - 1.0) + 1.0;
  float D = a2 / max(PI * d * d, 0.0001), k = (rough + 1.0) * (rough + 1.0) / 8.0;
  float G = (nl / (nl * (1.0-k) + k)) * (nv / (nv * (1.0-k) + k));
  vec3 F = fresnel(f0, max(dot(V,H), 0.0));
  return (base * (1.0-metal) * (1.0-F) / PI + F * D * G / max(4.0*nl*nv, 0.001)) * nl;
}
// An analytic studio environment: broad neutral panels, darker between them.
// Rough surfaces widen the panels; metals reflect them instead of turning gray.
vec3 studio(vec3 R, float rough, float metal) {
  vec3 q = vec3(dot(R,right), dot(R,up), dot(R,-forward));
  float power = mix(110.0, 3.0, rough * rough);
  float key = pow(max(dot(q, normalize(vec3(-0.55,0.75,0.65))),0.0),power);
  float fill = pow(max(dot(q, normalize(vec3(0.85,0.25,0.35))),0.0),power*0.65);
  float sky=0.08+0.2*smoothstep(-0.4,0.25,q.y)+0.7*smoothstep(0.45,0.85,q.y);
  return vec3(sky*mix(0.15,1.0,metal)) + vec3(1.5,1.48,1.42)*key + vec3(0.65,0.72,0.85)*fill;
}
vec3 srgb(vec3 x) {
  return mix(12.92*x, 1.055*pow(max(x,vec3(0.0)),vec3(1.0/2.4))-0.055, step(vec3(0.0031308),x));
}
void main() {
  vec3 V = orthographic > 0.5 ? normalize(-forward) : normalize(eye-vp), N = normalize(vn);
  // The wire has double-sided CAD faces. Light whichever side is visible;
  // this does not change winding, handedness, geometry or silk orientation.
  if(dot(N,V)<0.0) N=-N;
  float rough=vm.x, metal=vm.y, coat=vm.z, nv=max(dot(N,V),0.0);
  if(bz.x>0.5 && metal<0.5 && rough<0.8 && abs(normalize(vn).z)>0.9 && min(abs(vp.z-bz.y),abs(vp.z-bz.z))<12.0){ rough=0.24; coat=1.0; }
  vec3 base=max(vc,vec3(0.0)), f0=mix(vec3(0.04),base,metal);
  vec3 key=normalize(-right*0.55+up*0.75-forward*0.65);
  vec3 fill=normalize(right*0.85+up*0.25-forward*0.35);
  vec3 rgb=base*(1.0-metal)*(mix(0.27,0.74,step(0.8,rough))+0.08*max(dot(N,up),0.0));
  rgb+=light(N,V,key,base,f0,rough,metal)*1.5;
  rgb+=light(N,V,fill,base,f0,rough,metal)*0.65;
  rgb+=studio(reflect(-V,N),rough,metal)*fresnel(f0,nv)*(1.0-rough*0.35);
  rgb+=studio(reflect(-V,N),0.3,0.0)*fresnel(vec3(0.04),nv)*coat;
  // A display-only thermal overlay. No claim of per-part temperature/flow.
  float heat=clamp((thermal.y-25.0)/235.0,0.0,1.0);
  vec3 tint=mix(vec3(0.12,0.32,0.85),vec3(1.0,0.15,0.02),heat);
  rgb=mix(rgb,mix(rgb,tint,0.44)+vec3(0.1)*thermal.z*metal,thermal.x);
  rgb*=exp2(exposure);
  // Soft highlight shoulder; retain diffuse whites and dark material detail.
  rgb=rgb/(vec3(1.0)+max(rgb-vec3(0.8),vec3(0.0))*0.28);
  vec3 col=clamp(srgb(rgb),0.0,1.0);
  if (hon > 0.5) {
    bool inb = vp.x >= hl.x && vp.x <= hl.z && vp.y >= hl.y && vp.y <= hl.w && vp.z >= hz.x && vp.z <= hz.y;
    col = inb ? min(mix(col, vec3(1.0, 0.85, 0.4), 0.3) * 1.25, 1.0) : col * 0.4;
  }
  gl_FragColor=vec4(col,1.0);
}`;
