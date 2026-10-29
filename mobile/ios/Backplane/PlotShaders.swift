// The board viewer's shaders, compiled on the device the first time the
// viewer opens (so building the app needs no Metal toolchain).
//
// Each layer is drawn as coverage (R8, max blending, so overlapping copper
// never darkens), then laid over the frame in its colour and opacity.
// Tracks, outlines and dots are capsules with an analytic edge; fills go
// through the stencil (nonzero), so no polygon is triangulated beyond a
// fan. In 3D the same capsules are drawn on the board's faces: each end is
// projected, the capsule is widened in pixels by its depth, and the edge
// is measured in pixels, so layers stay sharp at any angle; the model's
// depth hides what is behind parts.
enum PlotShaders {
    static let source = #"""
#include <metal_stdlib>
using namespace metal;

struct U {
    float2 size;   // drawable, pixels
    float2 off;    // 2D: where micrometre 0,0 lands, pixels
    float scale;   // 2D: pixels per micrometre
    float fade;    // coverage multiplier (a chunk fading in)
    float z;       // 3D: the face's height, micrometres
    float focal;   // 3D: pixels per unit at depth 1
    float4 color;  // the layer's colour and opacity
    float4x4 mvp;  // 3D: micrometres to clip space
    float4 light;  // retained uniform layout
    float4 eye, right, up, forward;
    float4 thermal; // enabled, Celsius, liquid fraction, exposure
};

struct Cap { float ax, ay, bx, by, r; };

struct CapOut {
    float4 pos [[position]];
    float2 a [[flat]];
    float2 b [[flat]];
    float r [[flat]];
};

static float4 clip(float2 p, float2 size) {
    float2 c = p / size * 2.0 - 1.0;
    return float4(c.x, -c.y, 0, 1);
}

static float2 corner(uint vid, float2 A, float2 B, float R, thread float2 &dir) {
    float2 d = B - A;
    float l = length(d);
    dir = l > 0.001 ? d / l : float2(1, 0);
    float2 n = float2(-dir.y, dir.x);
    float pad = R + 1.0;
    float2 uv = float2((vid & 1) ? 1.0 : -1.0, (vid & 2) ? 1.0 : -1.0);
    return (uv.x < 0 ? A - dir * pad : B + dir * pad) + n * (uv.y * pad);
}

vertex CapOut cap_v(uint vid [[vertex_id]], uint iid [[instance_id]],
                    const device Cap *caps [[buffer(0)]], constant U &u [[buffer(1)]]) {
    Cap c = caps[iid];
    float2 A = float2(c.ax, c.ay) * u.scale + u.off;
    float2 B = float2(c.bx, c.by) * u.scale + u.off;
    // never thinner than a pixel (a zero-width KiCad line is a hairline)
    float R = max(c.r * u.scale, 0.5);
    float2 dir;
    float2 p = corner(vid, A, B, R, dir);
    CapOut o;
    o.pos = clip(p, u.size);
    o.a = A;
    o.b = B;
    o.r = R;
    return o;
}

// the same capsule on a face in 3D: its ends projected, its width by depth
vertex CapOut cap3_v(uint vid [[vertex_id]], uint iid [[instance_id]],
                     const device Cap *caps [[buffer(0)]], constant U &u [[buffer(1)]]) {
    Cap c = caps[iid];
    float4 ca = u.mvp * float4(c.ax, c.ay, u.z, 1);
    float4 cb = u.mvp * float4(c.bx, c.by, u.z, 1);
    ca.w = max(ca.w, 1e-3);
    cb.w = max(cb.w, 1e-3);
    float2 A = float2(ca.x / ca.w * 0.5 + 0.5, 0.5 - ca.y / ca.w * 0.5) * u.size;
    float2 B = float2(cb.x / cb.w * 0.5 + 0.5, 0.5 - cb.y / cb.w * 0.5) * u.size;
    float R = max(c.r * u.focal / max(min(ca.w, cb.w), 1e-3), 0.5);
    float2 dir;
    float2 p = corner(vid, A, B, R, dir);
    float4 e = (vid & 1) ? cb : ca;
    float2 n = p / u.size * 2.0 - 1.0;
    CapOut o;
    o.pos = float4(n.x * e.w, -n.y * e.w, e.z, e.w);
    o.a = A;
    o.b = B;
    o.r = R;
    return o;
}

// the edge is measured at the pixel itself (its position is in pixels)
fragment float4 cap_f(CapOut i [[stage_in]], constant U &u [[buffer(1)]]) {
    float2 pa = i.pos.xy - i.a, ba = i.b - i.a;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    float d = length(pa - ba * h);
    return float4(clamp(i.r + 0.5 - d, 0.0, 1.0) * u.fade);
}

vertex float4 fill_v(uint vid [[vertex_id]], const device packed_float2 *pts [[buffer(0)]], constant U &u [[buffer(1)]]) {
    return clip(float2(pts[vid]) * u.scale + u.off, u.size);
}

vertex float4 fill3_v(uint vid [[vertex_id]], const device packed_float2 *pts [[buffer(0)]], constant U &u [[buffer(1)]]) {
    return u.mvp * float4(float2(pts[vid]), u.z, 1);
}

fragment float4 fill_f(constant U &u [[buffer(1)]]) {
    return float4(u.fade);
}

struct QOut { float4 pos [[position]]; };

vertex QOut quad_v(uint vid [[vertex_id]]) {
    float2 p = float2((vid << 1) & 2, vid & 2);
    QOut o;
    o.pos = float4(p * 2.0 - 1.0, 0, 1);
    return o;
}

fragment float4 quad_f(QOut i [[stage_in]], texture2d<float> cov [[texture(0)]], constant U &u [[buffer(1)]]) {
    float a = cov.read(uint2(i.pos.xy)).r * u.color.a;
    return float4(u.color.rgb * a, a);
}

// Authored linear RGB plus colour-estimated materials; no material identity
// or process physics is inferred. Matches solid-materials.js presentation.
struct MV { packed_float3 p; packed_float3 n; };
struct MOut { float4 pos [[position]]; float3 vc; float3 vn; float3 vp; float3 vm; };
float3 estimate(float3 c) {
  float hi=max(c.r,max(c.g,c.b)), lo=min(c.r,min(c.g,c.b));
  if(hi<0.12 && hi-lo<max(0.015,hi*0.45)) return float3(0.63,0.0,0.12);
  if(lo>0.9) return float3(0.82,0.0,0.0);
  if(c.r>0.18 && c.g>c.r*0.55 && c.g<c.r*1.02 && c.b<c.g*0.45) return float3(0.3,0.86,0.0);
  if(hi-lo<hi*0.16 && lo>0.18 && hi<0.9) return float3(0.28,0.82,0.0);
  if(c.r>c.b*1.25 && c.g>c.b*1.12 && c.r>0.06) return float3(0.72,0.0,0.0);
  if(hi-lo>max(0.015,hi*0.28)) return float3(0.36,0.0,0.35);
  return float3(0.55,0.0,0.0);
}
constant float PI = 3.14159265;
float3 fresnel(float3 f0, float h) { return f0 + (1.0 - f0) * pow(1.0 - h, 5.0); }
float3 light(float3 N, float3 V, float3 L, float3 base, float3 f0, float rough, float metal) {
  float3 H = normalize(V + L);
  float nl = max(dot(N,L), 0.0), nv = max(dot(N,V), 0.001), nh = max(dot(N,H), 0.0);
  float a = rough * rough, a2 = a * a, d = nh * nh * (a2 - 1.0) + 1.0;
  float D = a2 / max(PI * d * d, 0.0001), k = (rough + 1.0) * (rough + 1.0) / 8.0;
  float G = (nl / (nl * (1.0-k) + k)) * (nv / (nv * (1.0-k) + k));
  float3 F = fresnel(f0, max(dot(V,H), 0.0));
  return (base * (1.0-metal) * (1.0-F) / PI + F * D * G / max(4.0*nl*nv, 0.001)) * nl;
}
// An analytic studio environment: broad neutral panels, darker between them.
// Rough surfaces widen the panels; metals reflect them instead of turning gray.
float3 studio(float3 R, float rough, float metal, float3 right, float3 up, float3 forward) {
  float3 q = float3(dot(R,right), dot(R,up), dot(R,-forward));
  float power = mix(110.0, 3.0, rough * rough);
  float key = pow(max(dot(q, normalize(float3(-0.55,0.75,0.65))),0.0),power);
  float fill = pow(max(dot(q, normalize(float3(0.85,0.25,0.35))),0.0),power*0.65);
  float sky=0.08+0.2*smoothstep(-0.4,0.25,q.y)+0.7*smoothstep(0.45,0.85,q.y);
  return float3(sky*mix(0.15,1.0,metal)) + float3(1.5,1.48,1.42)*key + float3(0.65,0.72,0.85)*fill;
}
float3 srgb(float3 x) {
  return mix(12.92*x, 1.055*pow(max(x,float3(0.0)),float3(1.0/2.4))-0.055, step(float3(0.0031308),x));
}

vertex MOut mesh_v(uint vid [[vertex_id]], const device MV *vs [[buffer(0)]], const device uint *cs [[buffer(2)]], constant U &u [[buffer(1)]]) {
    MV v = vs[vid]; uint c = cs[vid]; MOut o;
    o.pos = u.mvp * float4(float3(v.p), 1);
    o.vc = float3((c >> 16) & 255, (c >> 8) & 255, c & 255) / 255.0;
    o.vn = float3(v.n) * float3(1,-1,1); o.vp = float3(v.p) * float3(1,-1,1); o.vm = estimate(o.vc);
    return o;
}
fragment float4 mesh_f(MOut i [[stage_in]], constant U &u [[buffer(1)]]) {
    float3 vc=i.vc, vn=i.vn, vp=i.vp, vm=i.vm;
    float3 eye=u.eye.xyz, right=u.right.xyz, up=u.up.xyz, forward=u.forward.xyz, thermal=u.thermal.xyz;
    float exposure=u.thermal.w, orthographic=0.0;

  float3 V = orthographic > 0.5 ? normalize(-forward) : normalize(eye-vp), N = normalize(vn);
  // The wire has double-sided CAD faces. Light whichever side is visible;
  // this does not change winding, handedness, geometry or silk orientation.
  if(dot(N,V)<0.0) N=-N;
  float rough=vm.x, metal=vm.y, nv=max(dot(N,V),0.0);
  float3 base=max(vc,float3(0.0)), f0=mix(float3(0.04),base,metal);
  float3 key=normalize(-right*0.55+up*0.75-forward*0.65);
  float3 fill=normalize(right*0.85+up*0.25-forward*0.35);
  float3 rgb=base*(1.0-metal)*(mix(0.27,0.74,step(0.8,rough))+0.08*max(dot(N,up),0.0));
  rgb+=light(N,V,key,base,f0,rough,metal)*1.5;
  rgb+=light(N,V,fill,base,f0,rough,metal)*0.65;
  rgb+=studio(reflect(-V,N),rough,metal,right,up,forward)*fresnel(f0,nv)*(1.0-rough*0.35);
  rgb+=studio(reflect(-V,N),0.3,0.0,right,up,forward)*fresnel(float3(0.04),nv)*vm.z;
  // A display-only thermal overlay. No claim of per-part temperature/flow.
  float heat=clamp((thermal.y-25.0)/235.0,0.0,1.0);
  float3 tint=mix(float3(0.12,0.32,0.85),float3(1.0,0.15,0.02),heat);
  rgb=mix(rgb,mix(rgb,tint,0.44)+float3(0.1)*thermal.z*metal,thermal.x);
  rgb*=exp2(exposure);
  // Soft highlight shoulder; retain diffuse whites and dark material detail.
  rgb=rgb/(float3(1.0)+max(rgb-float3(0.8),float3(0.0))*0.28);
  return float4(clamp(srgb(rgb),0.0,1.0),1.0);

}
"""#
}
