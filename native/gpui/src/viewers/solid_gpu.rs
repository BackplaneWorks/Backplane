//! The 3D page on the GPU. The model goes into one device buffer when it arrives and stays there; a picture is then one small uniform block, three draws and a read back, on the
//! worker thread, with a context of its own (gpui lends out neither its device nor a way to draw into its frame on Linux, so the picture reaches it as an image). None from `new`
//! when the machine has no usable device, or only a software one, which draws slower than raster.rs: the page then uses that. BACKPLANE_SOLID_RENDERER=cpu or gpu forces the choice.
use super::raster::{to_linear, BG};
use super::solid::{Camera, Mesh, FOV};
use blade_graphics as gpu;
use std::sync::{Arc, Mutex, OnceLock, Weak};

const SAMPLES: u32 = 4;
const FORMAT: gpu::TextureFormat = gpu::TextureFormat::Bgra8UnormSrgb; // the byte order gpui's images take, encoded by the hardware
const DEPTH: gpu::TextureFormat = gpu::TextureFormat::Depth32Float;
/// A GPU that takes this long for one picture is stuck: the page draws that picture itself.
const WAIT_MS: u32 = 2000;

#[repr(C)] #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params { eye: [f32; 4], right: [f32; 4], up: [f32; 4], fwd: [f32; 4], proj: [f32; 4], bg: [f32; 4] }
#[repr(C)] #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vert { p: [f32; 3], nxy: u32, nz: u32, color: u32, group: u32 }
#[derive(blade_macros::ShaderData)] struct ModelData { params: Params, verts: gpu::BufferPiece, fades: gpu::BufferPiece }
#[derive(blade_macros::ShaderData)] struct BgData { params: Params }

fn snorm(v: f32) -> u32 { ((v.clamp(-1.0, 1.0) * 32767.0).round() as i16 as u16) as u32 }
/// The vertices to draw, every opaque triangle first, and how many of them are opaque. `group`: the fade group of all of them (a layer of
/// the model), else each triangle's own (Mesh::groups).
fn vertices(m: &Mesh, group: Option<u32>) -> (Vec<Vert>, u32) {
    let mut out = Vec::with_capacity(m.tris.len() * 3);
    for clear in [false, true] {
        for (i, t) in m.tris.iter().enumerate() {
            if (m.colors[i] >> 24 != 0) != clear { continue }
            for k in 0..3 {
                let n = &m.normals[i][3 * k..3 * k + 3];
                out.push(Vert { p: [t[3 * k], t[3 * k + 1], t[3 * k + 2]], nxy: snorm(n[0]) | snorm(n[1]) << 16, nz: snorm(n[2]), color: m.colors[i], group: group.unwrap_or_else(|| m.groups.get(i).copied().unwrap_or(0)) });
            }
        }
    }
    let opaque = m.colors.iter().filter(|c| **c >> 24 == 0).count() as u32 * 3;
    (out, opaque)
}

#[cfg(test)] pub fn bench_vertices(m: &Mesh) -> usize { vertices(m, None).0.len() }
struct Target { size: (u32, u32), msaa: gpu::Texture, msaa_view: gpu::TextureView, depth: gpu::Texture, depth_view: gpu::TextureView, resolve: gpu::Texture, resolve_view: gpu::TextureView, read: gpu::Buffer }
/// A layer of the model in a buffer of its own (the board, a part frame): `mesh` names it (a Weak keeps its allocation, so no later mesh can
/// take its address and be mistaken for it), `group` its fade group (None: each triangle's own).
struct Model { mesh: Weak<Mesh>, group: Option<u32>, verts: gpu::Buffer, opaque: u32, total: u32 }
struct Pipelines { bg: gpu::RenderPipeline, opaque: gpu::RenderPipeline, depth: gpu::RenderPipeline, blend: gpu::RenderPipeline, fade: gpu::RenderPipeline }
/// `fades`: each group's fade (Mesh::groups), host memory rewritten for each picture, `cap` floats long
pub struct Gpu { ctx: gpu::Context, encoder: gpu::CommandEncoder, pipes: Pipelines, targets: Vec<Target>, models: Vec<Model>, fades: Option<(gpu::Buffer, usize)>, last: Option<gpu::SyncPoint>, pub device: String }

/// Copies `n` bytes out of the read buffer. It is host memory the CPU does not cache (blade offers none that is: `Upload` is write-combined system memory, and
/// `Shared` can be the card's own memory across the bus, 75 to 150 ms for one 1600 x 1000 picture on an RTX 5080), where ordinary loads crawl and streaming
/// loads (SSE4.1's movntdqa) do not: 12 to 25 ms down to about one.
/// SAFETY: `src` and `dst` are valid for `n` bytes and do not overlap; `src` comes from blade's buffer mapping, which is aligned well beyond 16 bytes.
unsafe fn read_back(src: *const u8, dst: *mut u8, n: usize) {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("sse4.1") && src as usize % 16 == 0 { unsafe { stream(src, dst, n) }; return }
    unsafe { std::ptr::copy_nonoverlapping(src, dst, n) }
}
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn stream(src: *const u8, dst: *mut u8, n: usize) {
    use std::arch::x86_64::{__m128i, _mm_storeu_si128, _mm_stream_load_si128};
    let blocks = n / 64;
    for b in 0..blocks {
        // four loads in flight a line: the write-combining buffer is read a whole 64-byte line at a time
        unsafe {
            let (s, d) = (src.add(b * 64) as *mut __m128i, dst.add(b * 64) as *mut __m128i);
            let (x0, x1, x2, x3) = (_mm_stream_load_si128(s), _mm_stream_load_si128(s.add(1)), _mm_stream_load_si128(s.add(2)), _mm_stream_load_si128(s.add(3)));
            _mm_storeu_si128(d, x0); _mm_storeu_si128(d.add(1), x1); _mm_storeu_si128(d.add(2), x2); _mm_storeu_si128(d.add(3), x3);
        }
    }
    unsafe { std::ptr::copy_nonoverlapping(src.add(blocks * 64), dst.add(blocks * 64), n - blocks * 64) }
}
fn view(ctx: &gpu::Context, t: gpu::Texture, format: gpu::TextureFormat, name: &str) -> gpu::TextureView {
    ctx.create_texture_view(t, gpu::TextureViewDesc { name, format, dimension: gpu::ViewDimension::D2, subresources: &gpu::TextureSubresources::default() })
}
fn pipelines(ctx: &gpu::Context) -> Pipelines {
    let shader = ctx.create_shader(gpu::ShaderDesc { source: include_str!("solid.wgsl") });
    let (model, bg) = (<ModelData as gpu::ShaderData>::layout(), <BgData as gpu::ShaderData>::layout());
    let samples = gpu::MultisampleState { sample_count: SAMPLES, ..Default::default() };
    let depth = |write: bool, compare: gpu::CompareFunction| Some(gpu::DepthStencilState { format: DEPTH, depth_write_enabled: write, depth_compare: compare, stencil: Default::default(), bias: Default::default() });
    let draw = |name: &str, depth_state, blend: Option<gpu::BlendState>, write_mask: gpu::ColorWrites| ctx.create_render_pipeline(gpu::RenderPipelineDesc {
        name, data_layouts: &[&model], vertex: shader.at("vs_model"), vertex_fetches: &[], primitive: gpu::PrimitiveState { topology: gpu::PrimitiveTopology::TriangleList, ..Default::default() },
        depth_stencil: depth_state, fragment: Some(shader.at("fs_model")), color_targets: &[gpu::ColorTargetState { format: FORMAT, blend, write_mask }], multisample_state: samples });
    use gpu::CompareFunction::{Always, Equal, Greater};
    Pipelines {
        bg: ctx.create_render_pipeline(gpu::RenderPipelineDesc { name: "solid-bg", data_layouts: &[&bg], vertex: shader.at("vs_bg"), vertex_fetches: &[], primitive: gpu::PrimitiveState { topology: gpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: depth(false, Always), fragment: Some(shader.at("fs_bg")), color_targets: &[gpu::ColorTargetState { format: FORMAT, blend: None, write_mask: gpu::ColorWrites::default() }], multisample_state: samples }),
        opaque: draw("solid-opaque", depth(true, Greater), None, gpu::ColorWrites::default()),
        // transparency as the hub's renderer has it: only the nearest translucent layer shows, over what is opaque. Its depth first, then its colour where the depth is its own.
        depth: draw("solid-depth", depth(true, Greater), None, gpu::ColorWrites::empty()),
        blend: draw("solid-blend", depth(false, Equal), Some(gpu::BlendState::ALPHA_BLENDING), gpu::ColorWrites::COLOR),
        // a part fading in: over everything drawn, the coat included, its opacity its fade; it writes no depth, so what is behind it still shows through
        fade: draw("solid-fade", depth(false, Greater), Some(gpu::BlendState::ALPHA_BLENDING), gpu::ColorWrites::COLOR),
    }
}

/// The one GPU context of the process, made at the first picture of any 3D page and never torn down: dropped by a page's worker as the app quits, it would race
/// the driver's own teardown in `exit` (NVIDIA: a double free, or a hang), and a page closing should not cost a new device for the next one. None inside when
/// there is no usable device, or after one failed a picture (then left as it is, not destroyed: it may be wedged).
pub fn shared() -> &'static Mutex<Option<Gpu>> { static GPU: OnceLock<Mutex<Option<Gpu>>> = OnceLock::new(); GPU.get_or_init(|| Mutex::new(Gpu::new())) }
impl Gpu {
    pub fn new() -> Option<Gpu> {
        let want = std::env::var("BACKPLANE_SOLID_RENDERER").unwrap_or_default();
        if want == "cpu" { return None }
        let ctx = unsafe { gpu::Context::init(gpu::ContextDesc { presentation: false, ..Default::default() }) }.ok()?;
        let info = ctx.device_information().clone();
        if ctx.capabilities().sample_count_mask & SAMPLES == 0 || (info.is_software_emulated && want != "gpu") { return None }
        let encoder = ctx.create_command_encoder(gpu::CommandEncoderDesc { name: "solid", buffer_count: 2 });
        let pipes = pipelines(&ctx);
        Some(Gpu { ctx, encoder, pipes, targets: Vec::new(), models: Vec::new(), fades: None, last: None, device: info.device_name })
    }
    fn wait(&mut self) -> bool { match self.last.take() { Some(s) => self.ctx.wait_for(&s, WAIT_MS), None => true } }
    /// The layers into buffers of their own, each once: a layer already there stays (a part that arrives uploads only itself), one no
    /// longer in the model goes.
    fn upload(&mut self, layers: &[(&Arc<Mesh>, Option<u32>)]) -> bool {
        let same = |m: &Model, l: &(&Arc<Mesh>, Option<u32>)| Weak::as_ptr(&m.mesh) == Arc::as_ptr(l.0) && m.group == l.1;
        if self.models.len() == layers.len() && self.models.iter().zip(layers).all(|(m, l)| same(m, l)) { return true }
        if !self.wait() { return false }
        let mut old = std::mem::take(&mut self.models);
        let mut ok = true;
        for l in layers {
            if let Some(i) = old.iter().position(|m| same(m, l)) { self.models.push(old.swap_remove(i)); continue }
            if !ok { continue }
            let (data, opaque) = vertices(l.0, l.1);
            if data.is_empty() { continue }
            let bytes: &[u8] = bytemuck::cast_slice(&data);
            let staging = self.ctx.create_buffer(gpu::BufferDesc { name: "solid-upload", size: bytes.len() as u64, memory: gpu::Memory::Upload });
            let verts = self.ctx.create_buffer(gpu::BufferDesc { name: "solid-verts", size: bytes.len() as u64, memory: gpu::Memory::Device });
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), staging.data(), bytes.len()); }
            self.ctx.sync_buffer(staging);
            self.encoder.start();
            self.encoder.transfer("solid-upload").copy_buffer_to_buffer(staging.into(), verts.into(), bytes.len() as u64);
            let done = self.ctx.submit(&mut self.encoder);
            ok = self.ctx.wait_for(&done, WAIT_MS);
            self.ctx.destroy_buffer(staging);
            if !ok { self.ctx.destroy_buffer(verts); continue }
            self.models.push(Model { mesh: Arc::downgrade(l.0), group: l.1, verts, opaque, total: data.len() as u32 });
        }
        for m in old { self.ctx.destroy_buffer(m.verts); } // in the model's order: kept or uploaded, as asked
        ok
    }
    /// The targets for this size; the draft's and the picture at rest's both stay, they alternate through a drag.
    fn target(&mut self, size: (u32, u32)) -> usize {
        if let Some(i) = self.targets.iter().position(|t| t.size == size) { return i }
        if self.targets.len() >= 2 { let t = self.targets.remove(0); self.drop_target(t); }
        let extent = gpu::Extent { width: size.0, height: size.1, depth: 1 };
        let tex = |name: &str, format, sample_count, usage| self.ctx.create_texture(gpu::TextureDesc { name, format, size: extent, array_layer_count: 1, mip_level_count: 1, sample_count, dimension: gpu::TextureDimension::D2, usage, external: None });
        let (msaa, depth, resolve) = (tex("solid-msaa", FORMAT, SAMPLES, gpu::TextureUsage::TARGET), tex("solid-depth", DEPTH, SAMPLES, gpu::TextureUsage::TARGET), tex("solid-resolve", FORMAT, 1, gpu::TextureUsage::TARGET | gpu::TextureUsage::COPY));
        let read = self.ctx.create_buffer(gpu::BufferDesc { name: "solid-read", size: size.0 as u64 * size.1 as u64 * 4, memory: gpu::Memory::Upload });
        let t = Target { size, msaa, msaa_view: view(&self.ctx, msaa, FORMAT, "solid-msaa"), depth, depth_view: view(&self.ctx, depth, DEPTH, "solid-depth"), resolve, resolve_view: view(&self.ctx, resolve, FORMAT, "solid-resolve"), read };
        self.encoder.start();
        for x in [t.msaa, t.depth, t.resolve] { self.encoder.init_texture(x); }
        let done = self.ctx.submit(&mut self.encoder); self.last = Some(done);
        self.targets.push(t); self.targets.len() - 1
    }
    fn drop_target(&mut self, t: Target) {
        self.wait();
        for v in [t.msaa_view, t.depth_view, t.resolve_view] { self.ctx.destroy_texture_view(v); }
        for x in [t.msaa, t.depth, t.resolve] { self.ctx.destroy_texture(x); }
        self.ctx.destroy_buffer(t.read);
    }
    /// One picture as BGRA bytes, None when the GPU failed it.
    pub fn render(&mut self, mesh: &Arc<Mesh>, cam: &Camera, w: usize, h: usize) -> Option<Vec<u8>> { self.render_faded(mesh, &[], cam, w, h) }
    /// `render` with each group of triangles at its fade.
    pub fn render_faded(&mut self, mesh: &Arc<Mesh>, fades: &[f32], cam: &Camera, w: usize, h: usize) -> Option<Vec<u8>> { self.render_layers(&[(mesh, None)], fades, cam, w, h) }
    /// The fades into their buffer (group 0, the board, is always 1); whether any group is between 0 and 1.
    fn put_fades(&mut self, fades: &[f32]) -> bool {
        let n = fades.len().max(1);
        if self.fades.as_ref().map_or(true, |f| f.1 < n) {
            if let Some((old, _)) = self.fades.take() { self.wait(); self.ctx.destroy_buffer(old); }
            let cap = n.next_power_of_two().max(64);
            self.fades = Some((self.ctx.create_buffer(gpu::BufferDesc { name: "solid-fades", size: cap as u64 * 4, memory: gpu::Memory::Upload }), cap));
        }
        let (buf, _) = self.fades.unwrap();
        let mut any = false;
        let vals: Vec<f32> = (0..n).map(|g| if g == 0 { 1.0 } else { let f = fades.get(g).copied().unwrap_or(1.0).clamp(0.0, 1.0); any |= f < 1.0; f }).collect();
        unsafe { std::ptr::copy_nonoverlapping(vals.as_ptr() as *const u8, buf.data(), vals.len() * 4); }
        self.ctx.sync_buffer(buf);
        any
    }
    /// A picture of the model in layers (each a mesh and its fade group, None: each triangle's own), each group at its fade.
    pub fn render_layers(&mut self, layers: &[(&Arc<Mesh>, Option<u32>)], fades: &[f32], cam: &Camera, w: usize, h: usize) -> Option<Vec<u8>> {
        if !self.upload(layers) { return None }
        let size = (w as u32, h as u32);
        let at = self.target(size);
        if !self.wait() { return None }
        let (msaa, depth, resolve, read) = { let t = &self.targets[at]; (t.msaa_view, t.depth_view, t.resolve_view, t.read) };
        if !self.wait() { return None }
        let fading = self.put_fades(fades);
        let fbuf = self.fades?.0;
        let src = self.targets[at].resolve;
        let draws: Vec<(gpu::Buffer, u32, u32)> = self.models.iter().map(|m| (m.verts, m.opaque, m.total)).collect();
        let f = 1.0 / (FOV / 2.0).tan(); let v = |a: [f32; 3]| [a[0], a[1], a[2], 0.0]; let e = cam.eye();
        let params = Params { eye: v(e), right: v(cam.right), up: v(cam.up), fwd: v(cam.fwd), proj: [f * h as f32 / w as f32, f, (cam.dist * 0.02).max(1e-3), 0.0 /* the pass: 0 at rest, 1 fading parts only */],
            bg: [to_linear(BG[2]), to_linear(BG[1]), to_linear(BG[0]), 1.0] };
        self.encoder.start();
        {
            let mut pass = self.encoder.render("solid", gpu::RenderTargetSet {
                colors: &[gpu::RenderTarget { view: msaa, init_op: gpu::InitOp::Clear(gpu::TextureColor::OpaqueBlack), finish_op: gpu::FinishOp::ResolveTo(resolve) }],
                depth_stencil: Some(gpu::RenderTarget { view: depth, init_op: gpu::InitOp::Clear(gpu::TextureColor::TransparentBlack), finish_op: gpu::FinishOp::Discard }) });
            { let mut p = pass.with(&self.pipes.bg); p.bind(0, &BgData { params }); p.draw(0, 3, 0, 1); }
            // every layer's opaque triangles, then the translucent ones' depth, then their colour, then the parts fading in, over all of it
            let data = |verts: gpu::Buffer, params: Params| ModelData { params, verts: verts.into(), fades: fbuf.into() };
            { let mut p = pass.with(&self.pipes.opaque); for &(v, opaque, _) in &draws { if opaque > 0 { p.bind(0, &data(v, params)); p.draw(0, opaque, 0, 1); } } }
            if draws.iter().any(|d| d.2 > d.1) {
                { let mut p = pass.with(&self.pipes.depth); for &(v, opaque, total) in &draws { if total > opaque { p.bind(0, &data(v, params)); p.draw(opaque, total - opaque, 0, 1); } } }
                { let mut p = pass.with(&self.pipes.blend); for &(v, opaque, total) in &draws { if total > opaque { p.bind(0, &data(v, params)); p.draw(opaque, total - opaque, 0, 1); } } }
            }
            if fading { let mut q = params; q.proj[3] = 1.0; let mut p = pass.with(&self.pipes.fade); for &(v, _, total) in &draws { p.bind(0, &data(v, q)); p.draw(0, total, 0, 1); } }
        }
        self.encoder.transfer("solid-read").copy_texture_to_buffer(src.into(), read.into(), size.0 * 4, gpu::Extent { width: size.0, height: size.1, depth: 1 });
        let done = self.ctx.submit(&mut self.encoder);
        if !self.ctx.wait_for(&done, WAIT_MS) { self.last = Some(done); return None }
        self.ctx.sync_buffer(read);
        let n = w * h * 4; let mut px = Vec::with_capacity(n);
        unsafe { read_back(read.data(), px.as_mut_ptr(), n); px.set_len(n); }
        Some(px)
    }
}
impl Gpu {
    /// The model's buffer off the device (its page was put away); the next picture uploads it again.
    pub fn forget(&mut self) { if self.wait() { for m in std::mem::take(&mut self.models) { self.ctx.destroy_buffer(m.verts); } } }
}
impl Drop for Gpu {
    fn drop(&mut self) {
        self.wait();
        for t in std::mem::take(&mut self.targets) { self.drop_target(t); }
        for m in std::mem::take(&mut self.models) { self.ctx.destroy_buffer(m.verts); }
        if let Some((f, _)) = self.fades.take() { self.ctx.destroy_buffer(f); }
        for p in [&mut self.pipes.bg, &mut self.pipes.opaque, &mut self.pipes.depth, &mut self.pipes.blend, &mut self.pipes.fade] { self.ctx.destroy_render_pipeline(p); }
        self.ctx.destroy_command_encoder(&mut self.encoder);
    }
}
