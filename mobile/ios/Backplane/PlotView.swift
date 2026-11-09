import MetalKit
import simd
import SwiftUI

// The board viewer: a plot on the GPU. A plot is uploaded once; panning,
// zooming and orbiting only move a transform, so every frame costs the
// same however big the board. Frames are drawn on demand: while a finger
// moves, a fade runs or a fling coasts, and never otherwise.

private struct Uniforms {
    var size: SIMD2<Float>
    var off: SIMD2<Float>
    var scale: Float
    var fade: Float
    var z: Float = 0
    var focal: Float = 0
    var color: SIMD4<Float>
    var mvp = matrix_identity_float4x4
    var light = SIMD4<Float>(0, 0, 1, 0)
    var eye = SIMD4<Float>(0, 0, 0, 0)
    var right = SIMD4<Float>(1, 0, 0, 0)
    var up = SIMD4<Float>(0, 0, 1, 0)
    var forward = SIMD4<Float>(0, 1, 0, 0)
    var thermal = SIMD4<Float>(0, 25, 0, 0)
}

// one layer's share of the buffers: capsules and fill vertices, the chunks
// already shown first, then those fading in
private struct LayerDraw {
    var layer: Int
    var color: SIMD4<Float>
    var caps: Range<Int>
    var freshCaps: Range<Int>
    var tris: Range<Int>
    var freshTris: Range<Int>
}

// a 3D camera turning freely about a pivot (a board's rectangle's centre), like
// SolidWorks, in a right-handed world where the board's y is flipped
// (KiCad's y points down the screen), micrometres. r, u, f: the view's
// right, up and forward; pan: in the view plane; dist: from the eye to the
// pivot's depth
struct Orbit {
    var pivot = SIMD3<Float>(0, 0, 0)
    var r = SIMD3<Float>(1, 0, 0)
    var u = SIMD3<Float>(0, 0, 1)
    var f = SIMD3<Float>(0, 1, 0)
    var pan = SIMD2<Float>(0, 0)
    var dist: Float = 100_000
    var fov: Float = 35

    var target: SIMD3<Float> { pivot + r * pan.x + u * pan.y }
    var eye: SIMD3<Float> { target - f * dist }

    // world units per point (or pixel) at the pivot's depth, in a view h tall
    func unit(_ h: Float) -> Float { dist * 2 * tan(fov * .pi / 360) / max(h, 1) }

    // looking at the pivot from yaw and pitch (radians), world z up
    mutating func aim(yaw: Float, pitch: Float) {
        f = SIMD3(-sin(yaw) * cos(pitch), cos(yaw) * cos(pitch), -sin(pitch))
        u = SIMD3(0, 0, 1)
        square()
    }

    // orthonormal again (no drift): f, then r = f x u, u = r x f
    private mutating func square() {
        f = simd_normalize(f)
        r = simd_normalize(simd_cross(f, u))
        u = simd_normalize(simd_cross(r, f))
    }

    // the view turned by a about a unit axis (the model turns by -a)
    private mutating func turn(_ k: SIMD3<Float>, _ a: Float) {
        let q = simd_quatf(angle: a, axis: k)
        r = q.act(r)
        u = q.act(u)
        f = q.act(f)
        square()
    }

    // a finger moved m on screen, radians per unit: the model under it follows
    mutating func spin(_ m: SIMD2<Float>, _ k: Float) {
        let l = simd_length(m)
        guard l > 1e-6 else { return }
        turn((u * m.x + r * m.y) / l, -l * k)
    }

    // the model turned clockwise on screen by a
    mutating func roll(_ a: Float) { turn(f, -a) }

    // the point under the fingers at the pivot's depth moves with them (s: world per unit)
    mutating func move(_ m: SIMD2<Float>, _ s: Float) { pan += SIMD2(-m.x, m.y) * s }

    // dist over k, keeping the point at p (world units from the view's centre) where it is
    // within 1/60 and 20 times the fitted distance (home), so any model's
    // size zooms alike
    var home: Float = 0

    mutating func zoom(_ k: Float, _ p: SIMD2<Float>) {
        let d = min(max(dist / k, home > 0 ? home / 60 : 2_000), home > 0 ? home * 20 : 5_000_000)
        pan += p * (1 - d / dist)
        dist = d
    }

    func proj(_ aspect: Float) -> float4x4 {
        let f = 1 / tan(fov * .pi / 360), n = max(dist * 0.01, 1), fa = dist * 20 + 1_000_000
        return float4x4(columns: (SIMD4(f / aspect, 0, 0, 0), SIMD4(0, f, 0, 0), SIMD4(0, 0, fa / (n - fa), -1), SIMD4(0, 0, n * fa / (n - fa), 0)))
    }

    var view: float4x4 {
        let e = eye
        return float4x4(columns: (SIMD4(r.x, u.x, -f.x, 0), SIMD4(r.y, u.y, -f.y, 0), SIMD4(r.z, u.z, -f.z, 0),
                                  SIMD4(-simd_dot(r, e), -simd_dot(u, e), simd_dot(f, e), 1)))
    }

    // board micrometres to clip space
    func mvp(_ aspect: Float) -> float4x4 {
        proj(aspect) * view * float4x4(diagonal: SIMD4(1, -1, 1, 1))
    }
}

@MainActor
final class PlotRenderer: NSObject, MTKViewDelegate {
    let device: MTLDevice
    private let queue: MTLCommandQueue
    private let lib: MTLLibrary
    private var pipes: [String: MTLRenderPipelineState] = [:]
    private let plain, stencilWrite, stencilCover, depthRead, depthWrite, depthStencilWrite, depthStencilCover: MTLDepthStencilState
    private var capBuf, triBuf, meshBuf, colorBuf, hiCaps, hiTris, slabBuf, colorSlab: MTLBuffer?
    private var layers: [LayerDraw] = []
    private var meshCount = 0
    // the parts: one buffer that grows by appending, a range per arrived group (never the board's buffer)
    private var partBuf, partColors: MTLBuffer?
    private var partCapacity = 0     // vertices
    private var partCount = 0        // vertices held
    private var partRanges: [(start: Int, count: Int, at: Double)] = []
    private var partEpoch = -1
    private var hiCount = (0, 0)
    private var hiLayer = -1
    private var slabCount = 0
    private var cov, stencil, depth, msColor, msDepth: MTLTexture?
    // the model's multisampling (4x where the GPU has it)
    private let samples: Int
    var bg = SIMD4<Float>(0, 0, 0, 1)
    // the 3D viewers' gradient, sRGB 0 to 1 (Settings > Appearance; the 2D plot keeps the hub's bg)
    var ground = (top: SIMD4<Float>(0, 0, 0, 1), bottom: SIMD4<Float>(0, 0, 0, 1))
    private struct Ground { var top: SIMD4<Float>; var bottom: SIMD4<Float>; var size: SIMD2<Float> }
    // the gradient under the model, first in a pass that clears (no depth test, no depth write)
    private func drawGround(_ e: MTLRenderCommandEncoder, _ w: Int, _ h: Int, samples n: Int) {
        var g = Ground(top: ground.top, bottom: ground.bottom, size: SIMD2(Float(w), Float(h)))
        e.setRenderPipelineState(pipe("quad_v", "ground_f", cov: false, three: true, samples: n))
        e.setDepthStencilState(plain)
        e.setFragmentBytes(&g, length: MemoryLayout<Ground>.stride, index: 0)
        e.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
    }
    // each layer's colour on the viewer's light ground (empty: the chunks' own)
    var look: [UInt32] = []
    // the layers the user turned off, a bit each
    var hiddenLayers: UInt32 = 0
    func shown(_ layer: Int) -> Bool { layer < 0 || layer > 31 || (hiddenLayers >> UInt32(layer)) & 1 == 0 }
    // the layer focus: layers drawn at a quarter, a bit
    // each, and the active one, drawn after the rest (255 none)
    var dimLayers: UInt32 = 0
    var liftLayer = 255
    func dimmed(_ layer: Int) -> Bool { layer >= 0 && layer <= 31 && (dimLayers >> UInt32(layer)) & 1 == 1 }
    private func tint(_ l: LayerDraw) -> SIMD4<Float> {
        var c = l.color
        if dimmed(l.layer) { c.w *= 0.25 }
        return c
    }
    // 2D view: pixels per micrometre and where 0,0 lands, in points
    var scale: Float = 0.01
    var off = SIMD2<Float>(0, 0)
    var fade: Float = 1
    // 3D
    var three = false
    var orbit = Orbit()
    var thermal = SIMD4<Float>(0, 25, 0, 0)
    var thick: Float = 1600
    var top: [Int] = []
    var bottom: [Int] = []

    private static var compiled: MTLLibrary?

    private static func library(_ d: MTLDevice) -> MTLLibrary? {
        if let l = compiled { return l }
        do {
            compiled = try d.makeLibrary(source: PlotShaders.source, options: nil)
        } catch {
            NSLog("plot shaders: %@", "\(error)")
        }
        return compiled
    }

    init?(view: MTKView) {
        guard let d = MTLCreateSystemDefaultDevice(), let q = d.makeCommandQueue(), let l = Self.library(d) else { return nil }
        device = d
        queue = q
        lib = l
        view.device = d
        view.colorPixelFormat = .bgra8Unorm
        samples = [4, 2].first { d.supportsTextureSampleCount($0) } ?? 1
        func ds(_ f: (MTLDepthStencilDescriptor) -> Void) -> MTLDepthStencilState { let x = MTLDepthStencilDescriptor(); f(x); return d.makeDepthStencilState(descriptor: x)! }
        // nonzero winding: counter-clockwise triangles count up, others down
        func windUp(_ x: MTLDepthStencilDescriptor) {
            let up = MTLStencilDescriptor(), down = MTLStencilDescriptor()
            up.depthStencilPassOperation = .incrementWrap
            down.depthStencilPassOperation = .decrementWrap
            x.frontFaceStencil = up
            x.backFaceStencil = down
        }
        // cover where the count is not zero, and zero it again
        func cover(_ x: MTLDepthStencilDescriptor) {
            let s = MTLStencilDescriptor()
            s.stencilCompareFunction = .notEqual
            s.depthStencilPassOperation = .zero
            x.frontFaceStencil = s
            x.backFaceStencil = s
        }
        plain = ds { _ in }
        stencilWrite = ds(windUp)
        stencilCover = ds(cover)
        depthRead = ds { $0.depthCompareFunction = .lessEqual }
        depthWrite = ds { $0.depthCompareFunction = .less; $0.isDepthWriteEnabled = true }
        depthStencilWrite = ds { $0.depthCompareFunction = .lessEqual; windUp($0) }
        depthStencilCover = ds { $0.depthCompareFunction = .lessEqual; cover($0) }
        super.init()
    }

    // a pipeline: vertex and fragment functions, the target (coverage or
    // the frame), writing or not, blending by max (coverage) or over; in 3D
    // with depth
    private func pipe(_ v: String, _ f: String, cov: Bool, write: Bool = true, three: Bool, samples n: Int = 1) -> MTLRenderPipelineState {
        let key = "\(v)|\(f)|\(cov)|\(write)|\(three)|\(n)"
        if let p = pipes[key] { return p }
        let p = MTLRenderPipelineDescriptor()
        p.vertexFunction = lib.makeFunction(name: v)
        p.fragmentFunction = lib.makeFunction(name: f)
        let c = p.colorAttachments[0]!
        c.pixelFormat = cov ? .r8Unorm : .bgra8Unorm
        c.writeMask = write ? .all : []
        if write && f != "mesh_f" {  // mesh_fa blends
            c.isBlendingEnabled = true
            if cov {
                c.rgbBlendOperation = .max
                c.alphaBlendOperation = .max
                c.sourceRGBBlendFactor = .one
                c.destinationRGBBlendFactor = .one
                c.sourceAlphaBlendFactor = .one
                c.destinationAlphaBlendFactor = .one
            } else {
                c.sourceRGBBlendFactor = .one
                c.sourceAlphaBlendFactor = .one
                c.destinationRGBBlendFactor = .oneMinusSourceAlpha
                c.destinationAlphaBlendFactor = .oneMinusSourceAlpha
            }
        }
        if n > 1 {
            // the model's multisampled pass: depth, no stencil
            p.rasterSampleCount = n
            p.depthAttachmentPixelFormat = .depth32Float
        } else if three {
            p.depthAttachmentPixelFormat = .depth32Float_stencil8
            p.stencilAttachmentPixelFormat = .depth32Float_stencil8
        } else if cov {
            p.stencilAttachmentPixelFormat = .stencil8
        }
        let s = try! device.makeRenderPipelineState(descriptor: p)
        pipes[key] = s
        return s
    }

    private func buffer(_ a: [Float]) -> MTLBuffer? {
        a.isEmpty ? nil : device.makeBuffer(bytes: a, length: a.count * 4, options: .storageModeShared)
    }

    // a plot's chunks into two buffers, layer by layer (the hub sends them
    // in paint order)
    func load(_ chunks: [PlotChunk], fresh: Set<Int>) {
        var caps: [Float] = [], tris: [Float] = []
        caps.reserveCapacity(chunks.reduce(0) { $0 + $1.caps.count })
        tris.reserveCapacity(chunks.reduce(0) { $0 + $1.tris.count })
        var out: [LayerDraw] = []
        var i = 0
        while i < chunks.count {
            let first = chunks[i]
            var j = i
            while j < chunks.count, chunks[j].layer == first.layer, chunks[j].own == first.own,
                  !first.own || (chunks[j].color == first.color && chunks[j].alpha == first.alpha) { j += 1 }
            let c0 = caps.count / 5, t0 = tris.count / 2
            for k in i..<j where !fresh.contains(k) { caps += chunks[k].caps; tris += chunks[k].tris }
            let c1 = caps.count / 5, t1 = tris.count / 2
            for k in i..<j where fresh.contains(k) { caps += chunks[k].caps; tris += chunks[k].tris }
            let rgb = !first.own && first.layer >= 0 && first.layer < look.count ? look[first.layer] : first.color
            out.append(LayerDraw(layer: first.layer,
                color: SIMD4(Float((rgb >> 16) & 255) / 255, Float((rgb >> 8) & 255) / 255, Float(rgb & 255) / 255, first.alpha),
                caps: c0..<c1, freshCaps: c1..<(caps.count / 5), tris: t0..<t1, freshTris: t1..<(tris.count / 2)))
            i = j
        }
        capBuf = buffer(caps)
        triBuf = buffer(tris)
        layers = out
    }

    func load(mesh m: PlotMesh?) {
        guard let m, !m.colors.isEmpty else { meshBuf = nil; colorBuf = nil; meshCount = 0; return }
        meshBuf = buffer(m.verts)
        colorBuf = device.makeBuffer(bytes: m.colors, length: m.colors.count * 4, options: .storageModeShared)
        meshCount = m.colors.count
    }

    /// The parts held: groups past the ones already in the buffer are appended; another epoch (the parts started again) starts over.
    func load(parts p: PartsFrame?) {
        guard let p else { partRanges = []; partCount = 0; partEpoch = -1; return }
        if p.epoch != partEpoch || p.groups.count < partRanges.count { partRanges = []; partCount = 0; partEpoch = p.epoch }
        for g in p.groups[partRanges.count...] where g.count > 0 {
            let need = partCount + g.count
            if need > partCapacity {
                let cap = max(need, partCapacity * 2, 4096)
                let nb = device.makeBuffer(length: cap * 24, options: .storageModeShared)
                let nc = device.makeBuffer(length: cap * 4, options: .storageModeShared)
                if let nb, let nc {
                    if let partBuf, let partColors, partCount > 0 {
                        memcpy(nb.contents(), partBuf.contents(), partCount * 24)
                        memcpy(nc.contents(), partColors.contents(), partCount * 4)
                    }
                    partBuf = nb; partColors = nc; partCapacity = cap
                } else { return }
            }
            g.verts.withUnsafeBytes { memcpy(partBuf!.contents() + partCount * 24, $0.baseAddress!, g.verts.count * 4) }
            g.colors.withUnsafeBytes { memcpy(partColors!.contents() + partCount * 4, $0.baseAddress!, g.count * 4) }
            partRanges.append((partCount, g.count, g.at))
            partCount = need
        }
        // groups with no triangles still take their place in the order
        while partRanges.count < p.groups.count { partRanges.append((partCount, 0, p.groups[partRanges.count].at)) }
    }

    /// Whether some part is still fading in at `now` (the display link runs while so)
    func fading(_ now: Double) -> Bool { partRanges.contains { $0.count > 0 && now - $0.at < PartFade.ms / 1000 } }

    // the board as a plain slab (before its model arrives)
    func slab(_ box: [Float], color: UInt32) {
        guard box.count == 4 else { slabBuf = nil; slabCount = 0; return }
        let (x0, y0, x1, y1, z0, z1) = (box[0], box[1], box[2], box[3], Float(0), thick)
        let p: [SIMD3<Float>] = [SIMD3(x0, y0, z0), SIMD3(x1, y0, z0), SIMD3(x1, y1, z0), SIMD3(x0, y1, z0),
                                 SIMD3(x0, y0, z1), SIMD3(x1, y0, z1), SIMD3(x1, y1, z1), SIMD3(x0, y1, z1)]
        let faces = [[0, 1, 2, 3], [4, 5, 6, 7], [0, 1, 5, 4], [1, 2, 6, 5], [2, 3, 7, 6], [3, 0, 4, 7]]
        var v: [Float] = [], c: [UInt32] = []
        for f in faces {
            let n = simd_normalize(simd_cross(p[f[1]] - p[f[0]], p[f[2]] - p[f[0]]))
            for k in [0, 1, 2, 0, 2, 3] { let q = p[f[k]]; v += [q.x, q.y, q.z, n.x, n.y, n.z]; c.append(color) }
        }
        slabBuf = buffer(v)
        colorSlab = device.makeBuffer(bytes: c, length: c.count * 4, options: .storageModeShared)
        slabCount = c.count
    }

    // the picked piece, drawn bright over everything
    func highlight(_ c: PlotChunk?, _ p: PlotChunk.Piece?) {
        guard let c, let p else { hiCaps = nil; hiTris = nil; hiCount = (0, 0); hiLayer = -1; return }
        hiCaps = buffer(Array(c.caps[(p.caps.lowerBound * 5)..<(p.caps.upperBound * 5)]))
        hiTris = buffer(Array(c.tris[(p.tris.lowerBound * 6)..<(p.tris.upperBound * 6)]))
        hiCount = (p.caps.count, p.tris.count * 3)
        hiLayer = c.layer
    }

    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        cov = nil
    }

    private func targets(_ w: Int, _ h: Int) {
        if let c = cov, c.width == w, c.height == h { return }
        func tex(_ f: MTLPixelFormat, _ u: MTLTextureUsage, _ m: MTLStorageMode, samples n: Int = 1) -> MTLTexture? {
            let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: f, width: w, height: h, mipmapped: false)
            d.usage = u
            d.storageMode = m
            if n > 1 {
                d.textureType = .type2DMultisample
                d.sampleCount = n
            }
            return device.makeTexture(descriptor: d)
        }
        cov = tex(.r8Unorm, [.renderTarget, .shaderRead], .private)
        stencil = tex(.stencil8, .renderTarget, .memoryless)
        depth = tex(.depth32Float_stencil8, .renderTarget, .private)
        if samples > 1 {
            msColor = tex(.bgra8Unorm, .renderTarget, .memoryless, samples: samples)
            msDepth = tex(.depth32Float, .renderTarget, .memoryless, samples: samples)
        }
    }

    private func frame(_ cb: MTLCommandBuffer, _ t: MTLTexture, clear: Bool, depth d: Bool, depthLoad: Bool = false, _ f: (MTLRenderCommandEncoder) -> Void) {
        let pass = MTLRenderPassDescriptor()
        pass.colorAttachments[0].texture = t
        pass.colorAttachments[0].loadAction = clear ? .clear : .load
        pass.colorAttachments[0].clearColor = MTLClearColor(red: Double(bg.x), green: Double(bg.y), blue: Double(bg.z), alpha: 1)
        pass.colorAttachments[0].storeAction = .store
        if d {
            pass.depthAttachment.texture = depth
            pass.depthAttachment.loadAction = .clear
            pass.depthAttachment.clearDepth = 1
            pass.depthAttachment.storeAction = .store
            pass.stencilAttachment.texture = depth
            pass.stencilAttachment.loadAction = .clear
            pass.stencilAttachment.storeAction = .store
        }
        if depthLoad {
            pass.depthAttachment.texture = depth
            pass.depthAttachment.loadAction = .load
            pass.depthAttachment.storeAction = .dontCare
            pass.stencilAttachment.texture = depth
            pass.stencilAttachment.loadAction = .load
            pass.stencilAttachment.storeAction = .dontCare
        }
        guard let e = cb.makeRenderCommandEncoder(descriptor: pass) else { return }
        f(e)
        e.endEncoding()
    }

    // one layer's coverage (fills by stencil, then capsules), laid over the frame
    private func layer(_ cb: MTLCommandBuffer, _ drawable: MTLTexture, _ u: inout Uniforms, color: SIMD4<Float>,
                       caps: [(MTLBuffer?, Range<Int>, Float)], tris: [(MTLBuffer?, Range<Int>, Float)]) {
        let pass = MTLRenderPassDescriptor()
        pass.colorAttachments[0].texture = cov
        pass.colorAttachments[0].loadAction = .clear
        pass.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)
        pass.colorAttachments[0].storeAction = .store
        if three {
            pass.depthAttachment.texture = depth
            pass.depthAttachment.loadAction = .load
            pass.depthAttachment.storeAction = .store
            pass.stencilAttachment.texture = depth
        } else {
            pass.stencilAttachment.texture = stencil
        }
        pass.stencilAttachment.loadAction = .clear
        pass.stencilAttachment.storeAction = three ? .store : .dontCare
        guard let e = cb.makeRenderCommandEncoder(descriptor: pass) else { return }
        e.setFrontFacing(.counterClockwise)
        e.setCullMode(.none)
        let fillV = three ? "fill3_v" : "fill_v", capV = three ? "cap3_v" : "cap_v"
        for (b, r, f) in tris where !r.isEmpty && b != nil {
            u.fade = f
            e.setVertexBuffer(b, offset: r.lowerBound * 8, index: 0)
            e.setVertexBytes(&u, length: MemoryLayout<Uniforms>.stride, index: 1)
            e.setFragmentBytes(&u, length: MemoryLayout<Uniforms>.stride, index: 1)
            e.setRenderPipelineState(pipe(fillV, "fill_f", cov: true, write: false, three: three))
            e.setDepthStencilState(three ? depthStencilWrite : stencilWrite)
            e.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: r.count)
            e.setRenderPipelineState(pipe(fillV, "fill_f", cov: true, three: three))
            e.setDepthStencilState(three ? depthStencilCover : stencilCover)
            e.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: r.count)
        }
        e.setRenderPipelineState(pipe(capV, "cap_f", cov: true, three: three))
        e.setDepthStencilState(three ? depthRead : plain)
        for (b, r, f) in caps where !r.isEmpty && b != nil {
            u.fade = f
            e.setVertexBuffer(b, offset: r.lowerBound * 20, index: 0)
            e.setVertexBytes(&u, length: MemoryLayout<Uniforms>.stride, index: 1)
            e.setFragmentBytes(&u, length: MemoryLayout<Uniforms>.stride, index: 1)
            e.drawPrimitives(type: .triangleStrip, vertexStart: 0, vertexCount: 4, instanceCount: r.count)
        }
        e.endEncoding()
        frame(cb, drawable, clear: false, depth: false) { q in
            u.color = color
            q.setRenderPipelineState(pipe("quad_v", "quad_f", cov: false, three: false))
            q.setFragmentTexture(cov, index: 0)
            q.setFragmentBytes(&u, length: MemoryLayout<Uniforms>.stride, index: 1)
            q.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
        }
    }

    private func draws(_ l: LayerDraw) -> ([(MTLBuffer?, Range<Int>, Float)], [(MTLBuffer?, Range<Int>, Float)]) {
        ([(capBuf, l.caps, 1), (capBuf, l.freshCaps, fade)], [(triBuf, l.tris, 1), (triBuf, l.freshTris, fade)])
    }

    private func hi(_ cb: MTLCommandBuffer, _ t: MTLTexture, _ u: inout Uniforms) {
        guard hiCount.0 + hiCount.1 > 0 else { return }
        layer(cb, t, &u, color: SIMD4(1, 1, 1, 0.8), caps: [(hiCaps, 0..<hiCount.0, 1)], tris: [(hiTris, 0..<hiCount.1, 1)])
    }

    func draw(in view: MTKView) {
        guard let drawable = view.currentDrawable, let cb = queue.makeCommandBuffer() else { return }
        let w = Int(view.drawableSize.width), h = Int(view.drawableSize.height)
        guard w > 0, h > 0 else { return }
        targets(w, h)
        let k = Float(view.contentScaleFactor)
        var u = Uniforms(size: SIMD2(Float(w), Float(h)), off: off * k, scale: scale * k, fade: 1, color: .zero)
        let t = drawable.texture
        if three {
            u.mvp = orbit.mvp(Float(w) / Float(h))
            u.focal = 1 / tan(orbit.fov * .pi / 360) * Float(h) / 2
            u.eye = SIMD4(orbit.eye, 0); u.right = SIMD4(orbit.r, 0)
            u.up = SIMD4(orbit.u, 0); u.forward = SIMD4(orbit.f, 0)
            u.thermal = thermal
            // the model (or a slab) with depth, then the faces' layers over it
            let (mb, cbuf, mc) = meshCount > 0 ? (meshBuf, colorBuf, meshCount) : (slabBuf, colorSlab, slabCount)
            let dw = depthWrite
            let now = CACurrentMediaTime()
            var solidGroups = 0
            while solidGroups < partRanges.count, now - partRanges[solidGroups].at >= PartFade.ms / 1000 { solidGroups += 1 }
            let opaque = solidGroups < partRanges.count ? partRanges[solidGroups].start : partCount
            func model(_ e: MTLRenderCommandEncoder, _ p: MTLRenderPipelineState) {
                e.setFrontFacing(.counterClockwise)
                e.setCullMode(.none)
                e.setDepthStencilState(dw)
                e.setRenderPipelineState(p)
                e.setVertexBytes(&u, length: MemoryLayout<Uniforms>.stride, index: 1)
                e.setFragmentBytes(&u, length: MemoryLayout<Uniforms>.stride, index: 1)
                guard mc > 0, let mb, let cbuf else { return }
                e.setVertexBuffer(mb, offset: 0, index: 0)
                e.setVertexBuffer(cbuf, offset: 0, index: 2)
                e.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: mc)
                // the parts that have finished fading (arrival order, so they are a prefix), with the opaque model
                if opaque > 0, let pb = partBuf, let pc = partColors {
                    e.setVertexBuffer(pb, offset: 0, index: 0)
                    e.setVertexBuffer(pc, offset: 0, index: 2)
                    e.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: opaque)
                }
            }
            if samples > 1, let mcol = msColor, let mdep = msDepth {
                // multisampled and resolved into the frame; the layers test
                // against the model's depth drawn again single-sampled
                let pass = MTLRenderPassDescriptor()
                pass.colorAttachments[0].texture = mcol
                pass.colorAttachments[0].resolveTexture = t
                pass.colorAttachments[0].loadAction = .clear
                pass.colorAttachments[0].clearColor = MTLClearColor(red: Double(bg.x), green: Double(bg.y), blue: Double(bg.z), alpha: 1)
                pass.colorAttachments[0].storeAction = .multisampleResolve
                pass.depthAttachment.texture = mdep
                pass.depthAttachment.loadAction = .clear
                pass.depthAttachment.clearDepth = 1
                pass.depthAttachment.storeAction = .dontCare
                let ms = pipe("mesh_v", "mesh_f", cov: false, three: true, samples: samples)
                if let e = cb.makeRenderCommandEncoder(descriptor: pass) {
                    drawGround(e, w, h, samples: samples)
                    model(e, ms)
                    e.endEncoding()
                }
                let dp = pipe("mesh_v", "mesh_f", cov: false, write: false, three: true)
                frame(cb, t, clear: false, depth: true) { e in model(e, dp) }
            } else {
                let mp = pipe("mesh_v", "mesh_f", cov: false, three: true)
                frame(cb, t, clear: true, depth: true) { e in drawGround(e, w, h, samples: 1); model(e, mp) }
            }
            // the board's layers lie on a plain slab until its model comes,
            // which has its own mask, silk and pads (a picked piece still shows)
            for (face, z) in [(top, thick + 40), (bottom, Float(-40))] {
                u.z = z
                for id in face where meshCount == 0 {
                    for l in layers where l.layer == id && shown(l.layer) {
                        let (c, tr) = draws(l)
                        layer(cb, t, &u, color: l.color, caps: c, tris: tr)
                    }
                }
                if face.contains(hiLayer) { hi(cb, t, &u) }
            }
            // the parts still fading: blended over everything drawn, tested against the depth, not written to it
            if solidGroups < partRanges.count, let pb = partBuf, let pc = partColors {
                let fp = pipe("mesh_v", "mesh_fa", cov: false, three: true)
                frame(cb, t, clear: false, depth: false, depthLoad: true) { e in
                    e.setFrontFacing(.counterClockwise)
                    e.setCullMode(.none)
                    e.setDepthStencilState(depthRead)
                    e.setRenderPipelineState(fp)
                    e.setVertexBuffer(pb, offset: 0, index: 0)
                    e.setVertexBuffer(pc, offset: 0, index: 2)
                    for r in partRanges[solidGroups...] where r.count > 0 {
                        u.fade = PartFade.alpha(now - r.at)
                        e.setVertexBytes(&u, length: MemoryLayout<Uniforms>.stride, index: 1)
                        e.setFragmentBytes(&u, length: MemoryLayout<Uniforms>.stride, index: 1)
                        e.drawPrimitives(type: .triangle, vertexStart: r.start, vertexCount: r.count)
                    }
                }
            }
        } else {
            frame(cb, t, clear: true, depth: false) { _ in }
            // the active layer on top, as KiCad draws it
            for l in layers where shown(l.layer) && l.layer != liftLayer {
                let (c, tr) = draws(l)
                layer(cb, t, &u, color: tint(l), caps: c, tris: tr)
            }
            for l in layers where shown(l.layer) && l.layer == liftLayer {
                let (c, tr) = draws(l)
                layer(cb, t, &u, color: tint(l), caps: c, tris: tr)
            }
            hi(cb, t, &u)
        }
        cb.present(drawable)
        cb.commit()
    }
}

// The view: gestures move the transform; a display link runs only while
// something moves by itself (a fade, a fling). A tap asks what lies under
// the finger.
final class PlotCanvas: MTKView {
    var renderer: PlotRenderer!
    var box: [Float] = []
    // the board's rectangle: the 3D view turns about its centre
    var edge: [Float] = []
    var margin: Float = 0.9
    var zmin: Float = 0.5
    var zmax: Float = 0.5
    var tap: Float = 14
    var fadeMs: Double = 220
    var chunks: [PlotChunk] = []
    var onPick: (String) -> Void = { _ in }
    // a schematic's sheet symbols (micrometres) and what a tap on one opens
    var sheetHits: [(value: String, box: [Float])] = []
    var onSheet: (String) -> Void = { _ in }
    // review comments: a long press at a place (mm of the file) asks for a
    // note; a pin is a button over the plot that follows the camera
    var onNote: (Double, Double) -> Void = { _, _ in }
    var onPin: (String) -> Void = { _ in }
    private var pins: [ReviewPin] = []
    private var pinLanguage = ""
    private var pinViews: [String: UIButton] = [:]
    private var fitted = false
    private var shownFam = ""
    private var fadeFrom: Date?
    private var fling = SIMD2<Float>(0, 0)
    private var link: CADisplayLink?
    private var lastTick = CACurrentMediaTime()

    private struct Pose { let scale: Float; let off: SIMD2<Float>; let orbit: Orbit; let size: CGSize }
    private static var poses: [String: Pose] = [:]
    private static var poseOrder: [String] = []
    private var stateKey = ""
    private var pendingPose: Pose?

    private func savePose() {
        guard fitted, !stateKey.isEmpty else { return }
        Self.poses[stateKey] = Pose(scale: renderer.scale, off: renderer.off, orbit: renderer.orbit, size: bounds.size)
        Self.poseOrder.removeAll { $0 == stateKey }; Self.poseOrder.append(stateKey)
        while Self.poseOrder.count > 64 { Self.poses.removeValue(forKey: Self.poseOrder.removeFirst()) }
    }
    func bindSource(_ key: String) {
        guard key != stateKey else { return }
        savePose(); stateKey = key; pendingPose = Self.poses[key]
        fitted = false; box = []; edge = []; solid = []; chunks = []
        renderer.load([], fresh: []); renderer.load(mesh: nil); renderer.load(parts: nil); renderer.slab([], color: 0)
        renderer.highlight(nil, nil); fling = .zero
    }
    private func restorePose() {
        guard let pose = pendingPose, bounds.width > 0 else { return }
        renderer.scale = pose.scale
        renderer.off = pose.off + SIMD2(Float(bounds.width - pose.size.width), Float(bounds.height - pose.size.height)) / 2
        renderer.orbit = pose.orbit; fitted = true; pendingPose = nil
    }
    /// Parts arrived (or started again): appended to the parts buffer, and the display link runs for their fade
    func showParts(_ p: PartsFrame?) {
        renderer.load(parts: p)
        if renderer.fading(CACurrentMediaTime()) { run() }
        setNeedsDisplay()
    }
    func showMesh(_ frame: MeshFrame, standalone: Bool) {
        renderer.load(mesh: frame.mesh)
        if standalone, let b = frame.mesh?.box, b.count == 6 {
            solid = b
            if !fitted { fitSolid(b) }
        }
        restorePose(); setNeedsDisplay()
    }

    init?(canvas frame: CGRect) {
        super.init(frame: frame, device: nil)
        guard let r = PlotRenderer(view: self) else { return nil }
        renderer = r
        delegate = r
        isPaused = true
        enableSetNeedsDisplay = true
        preferredFramesPerSecond = 120
        isMultipleTouchEnabled = true
        let pinch = UIPinchGestureRecognizer(target: self, action: #selector(pinched(_:)))
        let pan = UIPanGestureRecognizer(target: self, action: #selector(panned(_:)))
        pan.maximumNumberOfTouches = 2
        let twist = UIRotationGestureRecognizer(target: self, action: #selector(twisted(_:)))
        let twice = UITapGestureRecognizer(target: self, action: #selector(tapped(_:)))
        twice.numberOfTapsRequired = 2
        let once = UITapGestureRecognizer(target: self, action: #selector(picked(_:)))
        once.require(toFail: twice)
        let hold = UILongPressGestureRecognizer(target: self, action: #selector(held(_:)))
        hold.minimumPressDuration = 0.6
        for g in [pinch, pan, twist, twice, once, hold] as [UIGestureRecognizer] {
            g.delegate = self
            addGestureRecognizer(g)
        }
    }

    required init(coder: NSCoder) { fatalError() }

    // every physical pixel (the default scale can be below the panel's)
    override func didMoveToWindow() {
        super.didMoveToWindow()
        if let s = window?.windowScene?.screen { contentScaleFactor = s.nativeScale }
    }

    override var canBecomeFirstResponder: Bool { true }
    override var keyCommands: [UIKeyCommand]? {
        let arrows = [UIKeyCommand.inputLeftArrow, UIKeyCommand.inputRightArrow, UIKeyCommand.inputUpArrow, UIKeyCommand.inputDownArrow]
        return arrows.flatMap { input in [UIKeyCommand(input: input, modifierFlags: [], action: #selector(keyPressed(_:))), UIKeyCommand(input: input, modifierFlags: .shift, action: #selector(keyPressed(_:)))] }
            + ["+", "-", "=", "f"].map { UIKeyCommand(input: $0, modifierFlags: [], action: #selector(keyPressed(_:))) }
    }
    @objc private func keyPressed(_ key: UIKeyCommand) {
        let delta: SIMD2<Float>
        switch key.input {
        case UIKeyCommand.inputLeftArrow: delta = SIMD2(-12, 0)
        case UIKeyCommand.inputRightArrow: delta = SIMD2(12, 0)
        case UIKeyCommand.inputUpArrow: delta = SIMD2(0, -12)
        case UIKeyCommand.inputDownArrow: delta = SIMD2(0, 12)
        case "+", "=": zoom(by: 1.15, at: CGPoint(x: bounds.midX, y: bounds.midY)); setNeedsDisplay(); return
        case "-": zoom(by: 1 / 1.15, at: CGPoint(x: bounds.midX, y: bounds.midY)); setNeedsDisplay(); return
        case "f": if box.count < 4, solid.count == 6 { fitSolid(solid) } else { refit() }; setNeedsDisplay(); return
        default: return
        }
        if three {
            var camera = renderer.orbit
            let units = camera.unit(Float(bounds.height))
            if key.modifierFlags.contains(.shift) { camera.move(delta, units) }
            else { camera.spin(delta, 0.008) }
            renderer.orbit = camera
        } else { renderer.off += delta }
        setNeedsDisplay()
    }

    private var three: Bool { renderer.three }

    // the zoom that shows the whole box, and the range around it
    private var fit: Float {
        guard box.count == 4 else { return renderer.scale }
        let w = Float(bounds.width), h = Float(bounds.height)
        return margin * min(w / max(box[2] - box[0], 1), h / max(box[3] - box[1], 1))
    }

    func refit() {
        guard box.count == 4, bounds.width > 0 else { return }
        let s = fit
        renderer.scale = s
        renderer.off = SIMD2(Float(bounds.width) / 2 - (box[0] + box[2]) / 2 * s, Float(bounds.height) / 2 - (box[1] + box[3]) / 2 * s)
        var o = Orbit(fov: renderer.orbit.fov)
        let e = edge.count == 4 ? edge : box
        let cx = (e[0] + e[2]) / 2, cy = (e[1] + e[3]) / 2
        o.pivot = SIMD3(cx, -cy, renderer.thick / 2)
        o.aim(yaw: 0.5, pitch: 0.75)
        // far enough to show the whole box round the pivot
        let reach = simd_length(SIMD2(max(abs(box[0] - cx), abs(box[2] - cx)), max(abs(box[1] - cy), abs(box[3] - cy))))
        // the narrower of the two fields of view takes the whole board
        let half = tan(o.fov * .pi / 360), aspect = Float(bounds.width) / Float(max(bounds.height, 1))
        o.dist = reach / (half * min(aspect, 1)) * 1.1
        o.home = o.dist
        renderer.orbit = o
        fitted = true
        setNeedsDisplay()
    }

    // the camera round a model's 3D box (x0 y0 z0 x1 y1 z1, the mesh's own
    // axes; the pivot's y is flipped, as refit's is)
    // a part alone: its box, fitted once there is room
    var solid: [Float] = []

    func fitSolid(_ b: [Float]) {
        guard bounds.width > 0 else { return }
        var o = Orbit(fov: renderer.orbit.fov)
        o.pivot = SIMD3((b[0] + b[3]) / 2, -(b[1] + b[4]) / 2, (b[2] + b[5]) / 2)
        o.aim(yaw: 0.5, pitch: 0.75)
        let diag = simd_length(SIMD3(b[3] - b[0], b[4] - b[1], b[5] - b[2]))
        let half = tan(o.fov * .pi / 360), aspect = Float(bounds.width) / Float(max(bounds.height, 1))
        o.dist = diag / 2 / (half * min(aspect, 1)) * 1.1
        o.home = o.dist
        renderer.orbit = o
        fitted = true
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        if !fitted { if box.count < 4, solid.count == 6 { fitSolid(solid) } else { refit() } }
        if pendingPose != nil { restorePose(); setNeedsDisplay() }
    }

    /// the 3D viewers' background; redraws only when it changed
    func setGround(_ b: ViewerBackdrop) {
        func v(_ c: UInt32) -> SIMD4<Float> { let (r, g, b) = ViewerBackdrop.unit(c); return SIMD4(r, g, b, 1) }
        let n = (top: v(b.top), bottom: v(b.bottom))
        guard n.top != renderer.ground.top || n.bottom != renderer.ground.bottom else { return }
        renderer.ground = n
        setNeedsDisplay()
    }

    func show(_ f: PlotFrame, bg: UInt32, slab: UInt32, look: [UInt32]) {
        renderer.bg = SIMD4(Float((bg >> 16) & 255) / 255, Float((bg >> 8) & 255) / 255, Float(bg & 255) / 255, 1)
        renderer.look = look
        renderer.thick = f.thick > 0 ? f.thick : 1600
        renderer.load(f.chunks, fresh: f.fresh)
        chunks = f.chunks
        // a new source (board to schematic, another sheet) fits anew; another
        // version of the same file keeps the view where the user left it
        let first = box.isEmpty || !fitted || f.fam != shownFam
        shownFam = f.fam
        box = f.box
        edge = f.edge.count == 4 && f.edge[0] <= f.edge[2] ? f.edge : f.box
        renderer.slab(edge, color: slab)
        if first { fitted = false; setNeedsLayout() }
        let animate = !f.fresh.isEmpty && !UIAccessibility.isReduceMotionEnabled
        fadeFrom = animate ? f.at : nil
        renderer.fade = animate ? 0 : 1
        restorePose()
        run()
        setNeedsDisplay()
    }

    // the picked piece ("chunk,info" of the held chunks), drawn bright
    func mark(_ picked: String) {
        let ps = picked.split(separator: ",").compactMap { Int($0) }
        if ps.count == 2, ps[0] < chunks.count, let p = chunks[ps[0]].pieces.first(where: { $0.info == ps[1] }) {
            renderer.highlight(chunks[ps[0]], p)
        } else {
            renderer.highlight(nil, nil)
        }
        setNeedsDisplay()
    }

    private func zoom(by f: Float, at p: CGPoint) {
        if three {
            // toward the point under the fingers
            let s = renderer.orbit.unit(Float(bounds.height))
            renderer.orbit.zoom(f, SIMD2(Float(p.x - bounds.width / 2), Float(bounds.height / 2 - p.y)) * s)
            return
        }
        let s0 = renderer.scale
        let s = min(max(s0 * f, fit * zmin), max(zmax, fit))
        let k = s / s0
        let a = SIMD2(Float(p.x), Float(p.y))
        renderer.off = a - (a - renderer.off) * k
        renderer.scale = s
    }

    @objc private func pinched(_ g: UIPinchGestureRecognizer) {
        zoom(by: Float(g.scale), at: g.location(in: self))
        g.scale = 1
        setNeedsDisplay()
    }

    @objc private func panned(_ g: UIPanGestureRecognizer) {
        if g.state == .began { becomeFirstResponder() }
        let t = g.translation(in: self)
        g.setTranslation(.zero, in: self)
        if three {
            // one finger turns the model under it, two move it with them
            var o = renderer.orbit
            let m = SIMD2(Float(t.x), Float(t.y))
            if g.numberOfTouches >= 2 { o.move(m, o.unit(Float(bounds.height))) } else { o.spin(m, 0.008) }
            renderer.orbit = o
            setNeedsDisplay()
            return
        }
        renderer.off += SIMD2(Float(t.x), Float(t.y))
        if g.state == .began { fling = .zero }
        if g.state == .ended && !UIAccessibility.isReduceMotionEnabled {
            let v = g.velocity(in: self)
            fling = SIMD2(Float(v.x), Float(v.y))
            run()
        }
        setNeedsDisplay()
    }

    // two fingers twisting roll the model about the view axis
    @objc private func twisted(_ g: UIRotationGestureRecognizer) {
        let a = Float(g.rotation)
        g.rotation = 0
        guard three else { return }
        renderer.orbit.roll(a)
        setNeedsDisplay()
    }

    @objc private func tapped(_ g: UITapGestureRecognizer) {
        if three || renderer.scale > fit * 1.5 { refit() } else { zoom(by: 3, at: g.location(in: self)) }
        setNeedsDisplay()
    }

    // where on the board a point of the view lands (micrometres), and on
    // which face's layers in 3D (the face toward the viewer)
    private func board(_ p: CGPoint) -> (SIMD2<Float>, [Int]?)? {
        guard three else {
            return ((SIMD2(Float(p.x), Float(p.y)) - renderer.off) / renderer.scale, nil)
        }
        let w = Float(bounds.width), h = Float(bounds.height)
        let inv = renderer.orbit.mvp(w / h).inverse
        let n = SIMD2(Float(p.x) / w * 2 - 1, 1 - Float(p.y) / h * 2)
        func at(_ z: Float) -> SIMD3<Float> { let q = inv * SIMD4(n.x, n.y, z, 1); return SIMD3(q.x, q.y, q.z) / q.w }
        let a = at(0), b = at(1)
        let above = renderer.orbit.eye.z > renderer.thick / 2
        let z = above ? renderer.thick + 40 : -40
        guard abs(b.z - a.z) > 1e-6 else { return nil }
        let t = (z - a.z) / (b.z - a.z)
        guard t > 0 else { return nil }
        let q = a + (b - a) * t
        return (SIMD2(q.x, q.y), above ? renderer.top : renderer.bottom)
    }

    // a long press on the board: where in the file's mm (2D views only)
    @objc private func held(_ g: UILongPressGestureRecognizer) {
        guard g.state == .began else { return }
        hold(at: g.location(in: self))
    }

    func hold(at p: CGPoint) {
        guard !three, renderer.scale > 0 else { return }
        let q = (SIMD2(Float(p.x), Float(p.y)) - renderer.off) / renderer.scale
        Haptics.impact()
        onNote(Double(q.x) / 1000, Double(q.y) / 1000)
    }

    // the pins: thumb-sized buttons on the plot's own transform, moved
    // whenever the camera moves (setNeedsDisplay), no redraw of the plot
    func setPins(_ ps: [ReviewPin], lang: String) {
        guard ps != pins || lang != pinLanguage else { return }
        pins = ps
        pinLanguage = lang
        let ids = Set(ps.map { $0.id })
        for (id, v) in pinViews where !ids.contains(id) { v.removeFromSuperview(); pinViews[id] = nil }
        for p in ps where pinViews[p.id] == nil {
            let b = UIButton(type: .custom)
            b.frame = CGRect(x: 0, y: 0, width: 44, height: 44)
            let box = UIView(frame: CGRect(x: 11, y: 11, width: 22, height: 22))
            box.isUserInteractionEnabled = false
            box.tag = 1
            let n = UILabel(frame: box.bounds)
            n.font = .systemFont(ofSize: 11, weight: .bold)
            n.textColor = .white
            n.textAlignment = .center
            n.tag = 2
            box.addSubview(n)
            b.addSubview(box)
            b.accessibilityIdentifier = "pin-" + p.id
            b.addAction(UIAction { [weak self] _ in self?.onPin(p.id) }, for: .touchUpInside)
            addSubview(b)
            pinViews[p.id] = b
        }
        for p in ps {
            guard let b = pinViews[p.id], let box = b.viewWithTag(1), let n = box.viewWithTag(2) as? UILabel else { continue }
            box.backgroundColor = tintColor.withAlphaComponent(p.resolved ? 0.4 : 1)
            box.layer.borderColor = UIColor.white.cgColor
            box.layer.borderWidth = 1.5
            n.text = p.count > 0 ? String(p.count) : ""
            b.accessibilityLabel = Tr.native_review_pin(kind: p.count > 0 ? "replies" : "none", count: String(p.count))
        }
        layoutPins()
    }

    private func layoutPins() {
        for p in pins {
            guard let b = pinViews[p.id] else { continue }
            b.isHidden = three
            let x = CGFloat(Float(p.x) * 1000 * renderer.scale + renderer.off.x)
            let y = CGFloat(Float(p.y) * 1000 * renderer.scale + renderer.off.y)
            b.center = CGPoint(x: x, y: y)
        }
    }

    override func setNeedsDisplay() {
        super.setNeedsDisplay()
        layoutPins()
    }

    @objc private func picked(_ g: UITapGestureRecognizer) {
        tap(at: g.location(in: self))
    }

    // what lies under a point, for to pick from (View.pick)
    func tap(at p: CGPoint) {
        guard let (q, face) = board(p) else { onPick("[]"); return }
        // a sheet symbol opens that sheet (the smallest when they nest)
        if !three, let hit = sheetHits.filter({ q.x >= $0.box[0] && q.x <= $0.box[2] && q.y >= $0.box[1] && q.y <= $0.box[3] })
            .min(by: { ($0.box[2] - $0.box[0]) * ($0.box[3] - $0.box[1]) < ($1.box[2] - $1.box[0]) * ($1.box[3] - $1.box[1]) }) {
            onSheet(hit.value)
            return
        }
        // the finger's reach, in micrometres
        let tol: Float
        if three {
            tol = tap * renderer.orbit.unit(Float(bounds.height))
        } else {
            tol = tap / renderer.scale
        }
        var cands: [[String: Any]] = []
        for (ci, c) in chunks.enumerated() {
            if let face, !face.contains(c.layer) { continue }
            for piece in c.pieces {
                let b = piece.box
                guard q.x >= b.x - tol, q.x <= b.z + tol, q.y >= b.y - tol, q.y <= b.w + tol else { continue }
                guard c.distance(piece, q.x, q.y) <= tol else { continue }
                let area = max(b.z - b.x, 1) * max(b.w - b.y, 1) / 100
                cands.append(["c": ci, "p": piece.info, "a": Int(min(area, 4e9)), "l": c.layer])
            }
        }
        let json = (try? JSONSerialization.data(withJSONObject: cands)).flatMap { String(data: $0, encoding: .utf8) } ?? "[]"
        onPick(json)
    }

    private func run() {
        guard link == nil else { return }
        lastTick = CACurrentMediaTime()
        let l = CADisplayLink(target: self, selector: #selector(tick))
        l.preferredFrameRateRange = CAFrameRateRange(minimum: 60, maximum: 120, preferred: 120)
        l.add(to: .main, forMode: .common)
        link = l
    }

    @objc private func tick() {
        let now = CACurrentMediaTime()
        let dt = Float(now - lastTick)
        lastTick = now
        var busy = false
        if let t0 = fadeFrom {
            let f = Float(min(1, Date().timeIntervalSince(t0) * 1000 / fadeMs))
            renderer.fade = f
            if f >= 1 { fadeFrom = nil } else { busy = true }
        }
        if renderer.fading(now) { busy = true }
        if simd_length(fling) > 8 {
            renderer.off += fling * dt
            fling *= pow(0.004, dt)
            busy = true
        } else {
            fling = .zero
        }
        setNeedsDisplay()
        if !busy {
            link?.invalidate()
            link = nil
        }
    }

    override func removeFromSuperview() {
        savePose()
        link?.invalidate()
        link = nil
        super.removeFromSuperview()
    }
}

extension PlotCanvas: UIGestureRecognizerDelegate {
    func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { true }
}

struct PlotCanvasView: UIViewRepresentable {
    @Environment(\.backplaneLanguage) private var lang
    @Environment(\.colorScheme) private var scheme
    let frame: PlotFrame?
    let mesh: MeshFrame?
    /// the parts of this model streaming in after the bare board (nil for a model that has them in it)
    var parts: PartsFrame?
    var scope = ""
    var thermal = SIMD4<Float>(0, 25, 0, 0)
    let viewer: Viewer
    let pick: (String) -> Void
    var sheet: (String) -> Void = { _ in }
    var onNote: (Double, Double) -> Void = { _, _ in }
    var onPin: (String) -> Void = { _ in }

    func makeUIView(context: Context) -> UIView {
        guard let c = PlotCanvas(canvas: .zero) else {
            let l = UILabel()
            l.text = Tr.k("native.viewer.metal")
            l.textAlignment = .center
            return l
        }
        return c
    }

    func updateUIView(_ v: UIView, context: Context) {
        guard let c = v as? PlotCanvas else {
            (v as? UILabel)?.text = Tr.k("native.viewer.metal")
            return
        }
        let source = scope + "\0" + (viewer.key.isEmpty ? viewer.layers : viewer.key) + "\0" + viewer.open
        if source != context.coordinator.source {
            context.coordinator.source = source; context.coordinator.shown = nil; context.coordinator.mesh = nil; context.coordinator.partsEpoch = -2; context.coordinator.partsCount = -1; context.coordinator.picked = ""
            c.bindSource(source)
        }
        // the setting and the theme both redraw it at once (AppSettings is observed here)
        c.setGround(AppSettings.shared.viewerBackdrop(dark: scheme == .dark))
        if c.renderer.thermal != thermal { c.renderer.thermal = thermal; c.setNeedsDisplay() }
        c.margin = viewer.margin
        c.zmin = viewer.zmin
        c.zmax = viewer.zmax
        c.tap = viewer.tap
        c.fadeMs = Double(viewer.fade)
        c.onPick = pick
        c.onSheet = sheet
        c.sheetHits = (viewer.sheets ?? []).compactMap { s in
            let n = (s.box ?? "").split(separator: " ").compactMap { Float($0) }
            guard n.count == 4 else { return nil }
            return (s.value, [min(n[0], n[2]) * 1000, min(n[1], n[3]) * 1000, max(n[0], n[2]) * 1000, max(n[1], n[3]) * 1000])
        }
        c.onNote = onNote
        c.onPin = onPin
        c.setPins(viewer.comments?.pins ?? [], lang: lang)
        c.renderer.top = viewer.top
        c.renderer.bottom = viewer.bottom
        c.renderer.orbit.fov = viewer.fov
        let three = viewer.open == "3d" || viewer.open == "mech"
        if c.renderer.three != three {
            c.renderer.three = three
        }
        if c.renderer.hiddenLayers != (viewer.off ?? 0) || c.renderer.dimLayers != (viewer.dim ?? 0) || c.renderer.liftLayer != (viewer.lift ?? 255) {
            c.renderer.hiddenLayers = viewer.off ?? 0
            c.renderer.dimLayers = viewer.dim ?? 0
            c.renderer.liftLayer = viewer.lift ?? 255
            c.setNeedsDisplay()
        }
        if let f = frame, f.at != context.coordinator.shown || (viewer.look ?? []) != context.coordinator.look {
            context.coordinator.shown = f.at
            context.coordinator.look = viewer.look ?? []
            c.show(f, bg: viewer.bg, slab: viewer.slab, look: viewer.look ?? [])
            #if DEBUG
            // headless checks: SIMCTL_CHILD_BACKPLANE_TAP=x,y (points) taps there once
            if let t = ProcessInfo.processInfo.environment["BACKPLANE_TAP"], !context.coordinator.tapped {
                context.coordinator.tapped = true
                let xy = t.split(separator: ",").compactMap { Double($0) }
                if xy.count == 2 { DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { c.tap(at: CGPoint(x: xy[0], y: xy[1])) } }
            }
            if let t = ProcessInfo.processInfo.environment["BACKPLANE_HOLD"], !context.coordinator.held {
                // a long press there (points), as a finger would
                context.coordinator.held = true
                let xy = t.split(separator: ",").compactMap { Double($0) }
                if xy.count == 2 { DispatchQueue.main.asyncAfter(deadline: .now() + 6) { c.hold(at: CGPoint(x: xy[0], y: xy[1])) } }
            }
            #endif
        }
        if three, let m = mesh, m.at != context.coordinator.mesh {
            context.coordinator.mesh = m.at
            c.showMesh(m, standalone: frame == nil)
        }
        if three, parts?.epoch != context.coordinator.partsEpoch || (parts?.groups.count ?? 0) != context.coordinator.partsCount {
            context.coordinator.partsEpoch = parts?.epoch ?? -1
            context.coordinator.partsCount = parts?.groups.count ?? 0
            c.showParts(parts)
        }
        if viewer.picked != context.coordinator.picked {
            context.coordinator.picked = viewer.picked
            c.mark(viewer.picked)
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    final class Coordinator {
        var source = ""
        var shown: Date?
        var look: [UInt32] = []
        var mesh: Date?
        var partsEpoch = -2
        var partsCount = -1
        var picked = ""
        var tapped = false
        var held = false
    }
}

extension Color {
    init(rgb: UInt32) {
        self.init(red: Double((rgb >> 16) & 255) / 255, green: Double((rgb >> 8) & 255) / 255, blue: Double(rgb & 255) / 255)
    }
}


private func plotError(_ raw: String, _ lang: String) -> String {
    if raw.hasPrefix("@bp.file-missing:") { return Tr.viewer_file_missing(name: String(raw.dropFirst(17))) }
    if raw == "@bp.file-unread" { return Tr.k("viewer.file.unread") }
    return Tr.t(raw)
}
