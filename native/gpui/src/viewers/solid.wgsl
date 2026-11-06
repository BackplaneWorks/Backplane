// The 3D page's GPU shader. Colours are linear light, as the wire's are: the target is sRGB, so the one transfer is the hardware's, once, after blending and the MSAA resolve.
// The material model is the phones' and the old web viewer's (solid-materials.js, PlotShaders.swift): the wire has colours only, so roughness, metalness and coat are estimated
// from the colour. raster.rs is the same model in Rust; the test `gpu_and_cpu_agree` holds them together.
struct Params { eye: vec4<f32>, right: vec4<f32>, up: vec4<f32>, fwd: vec4<f32>, proj: vec4<f32>, bg: vec4<f32> }
var<uniform> params: Params;
struct Vert { px: f32, py: f32, pz: f32, nxy: u32, nz: u32, color: u32 }
var<storage, read> verts: array<Vert>;

struct Out {
  @invariant @builtin(position) clip: vec4<f32>,
  @location(0) p: vec3<f32>,
  @location(1) n: vec3<f32>,
  @location(2) @interpolate(flat) color: u32,
}

@vertex fn vs_model(@builtin(vertex_index) i: u32) -> Out {
  let v = verts[i];
  let p = vec3<f32>(v.px, v.py, v.pz);
  let d = p - params.eye.xyz;
  // reversed z, far at infinity: 1 at the near plane, towards 0 far away, so the 40 micrometres between a board's layers stay apart at any zoom
  let z = dot(d, params.fwd.xyz);
  var o: Out;
  o.clip = vec4<f32>(dot(d, params.right.xyz) * params.proj.x, dot(d, params.up.xyz) * params.proj.y, params.proj.z, z);
  o.p = p;
  o.n = vec3<f32>(unpack2x16snorm(v.nxy), unpack2x16snorm(v.nz).x);
  o.color = v.color;
  return o;
}

@vertex fn vs_bg(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
  return vec4<f32>(f32(i & 1u) * 4.0 - 1.0, f32(i >> 1u) * 4.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs_bg() -> @location(0) vec4<f32> { return vec4<f32>(params.bg.xyz, 1.0); }

const PI = 3.14159265;
fn estimate(c: vec3<f32>) -> vec3<f32> {
  let hi = max(c.r, max(c.g, c.b)); let lo = min(c.r, min(c.g, c.b));
  if (hi < 0.12 && hi - lo < max(0.015, hi * 0.45)) { return vec3<f32>(0.63, 0.0, 0.12); }
  if (lo > 0.9) { return vec3<f32>(0.82, 0.0, 0.0); }
  if (c.r > 0.18 && c.g > c.r * 0.55 && c.g < c.r * 1.02 && c.b < c.g * 0.45) { return vec3<f32>(0.3, 0.86, 0.0); }
  if (hi - lo < hi * 0.16 && lo > 0.18 && hi < 0.9) { return vec3<f32>(0.28, 0.82, 0.0); }
  if (c.r > c.b * 1.25 && c.g > c.b * 1.12 && c.r > 0.06) { return vec3<f32>(0.72, 0.0, 0.0); }
  if (hi - lo > max(0.015, hi * 0.28)) { return vec3<f32>(0.36, 0.0, 0.35); }
  return vec3<f32>(0.55, 0.0, 0.0);
}
fn fresnel(f0: vec3<f32>, h: f32) -> vec3<f32> { return f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - h, 5.0); }
fn lit(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, base: vec3<f32>, f0: vec3<f32>, rough: f32, metal: f32) -> vec3<f32> {
  let H = normalize(V + L);
  let nl = max(dot(N, L), 0.0); let nv = max(dot(N, V), 0.001); let nh = max(dot(N, H), 0.0);
  let a = rough * rough; let a2 = a * a; let d = nh * nh * (a2 - 1.0) + 1.0;
  let D = a2 / max(PI * d * d, 0.0001); let k = (rough + 1.0) * (rough + 1.0) / 8.0;
  let G = (nl / (nl * (1.0 - k) + k)) * (nv / (nv * (1.0 - k) + k));
  let F = fresnel(f0, max(dot(V, H), 0.0));
  return (base * (1.0 - metal) * (vec3<f32>(1.0) - F) / PI + F * D * G / max(4.0 * nl * nv, 0.001)) * nl;
}
fn studio(R: vec3<f32>, rough: f32, metal: f32) -> vec3<f32> {
  let q = vec3<f32>(dot(R, params.right.xyz), dot(R, params.up.xyz), -dot(R, params.fwd.xyz));
  let power = mix(110.0, 3.0, rough * rough);
  let key = pow(max(dot(q, normalize(vec3<f32>(-0.55, 0.75, 0.65))), 0.0), power);
  let fill = pow(max(dot(q, normalize(vec3<f32>(0.85, 0.25, 0.35))), 0.0), power * 0.65);
  let sky = 0.08 + 0.2 * smoothstep(-0.4, 0.25, q.y) + 0.7 * smoothstep(0.45, 0.85, q.y);
  return vec3<f32>(sky * mix(0.15, 1.0, metal)) + vec3<f32>(1.5, 1.48, 1.42) * key + vec3<f32>(0.65, 0.72, 0.85) * fill;
}

@fragment fn fs_model(i: Out) -> @location(0) vec4<f32> {
  let right = params.right.xyz; let up = params.up.xyz; let fwd = params.fwd.xyz;
  let base = vec3<f32>(f32((i.color >> 16u) & 255u), f32((i.color >> 8u) & 255u), f32(i.color & 255u)) / 255.0;
  let clear = i.color >> 24u; // the wire's transparency: 0 opaque, 255 clear
  var m = estimate(base);
  if (clear != 0u && m.x < 0.8) { m = vec3<f32>(0.24, 0.0, 1.0); } // a translucent coat that is not silk is the soldermask: glossy, the person's choice (2026-10-31)
  let rough = m.x; let metal = m.y; let coat = m.z;
  let V = normalize(params.eye.xyz - i.p);
  var N = normalize(i.n);
  if (dot(N, V) < 0.0) { N = -N; } // CAD faces are double-sided: light whichever side shows
  let nv = max(dot(N, V), 0.0);
  let f0 = mix(vec3<f32>(0.04), base, metal);
  let key = normalize(-right * 0.55 + up * 0.75 - fwd * 0.65);
  let fill = normalize(right * 0.85 + up * 0.25 - fwd * 0.35);
  var rgb = base * (1.0 - metal) * (mix(0.27, 0.74, step(0.8, rough)) + 0.08 * max(dot(N, up), 0.0));
  rgb += lit(N, V, key, base, f0, rough, metal) * 1.5;
  rgb += lit(N, V, fill, base, f0, rough, metal) * 0.65;
  let R = reflect(-V, N);
  rgb += studio(R, rough, metal) * fresnel(f0, nv) * (1.0 - rough * 0.35);
  rgb += studio(R, 0.3, 0.0) * fresnel(vec3<f32>(0.04), nv) * coat;
  rgb = rgb / (vec3<f32>(1.0) + max(rgb - vec3<f32>(0.8), vec3<f32>(0.0)) * 0.28); // a soft shoulder for highlights
  return vec4<f32>(rgb, 1.0 - f32(clear) / 255.0);
}
