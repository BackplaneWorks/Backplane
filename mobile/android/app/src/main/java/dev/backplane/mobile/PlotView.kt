package dev.backplane.mobile

import android.annotation.SuppressLint
import android.content.Context
import android.opengl.GLES30.*
import android.opengl.GLSurfaceView
import android.opengl.Matrix
import android.view.Choreographer
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import java.nio.ByteBuffer
import java.nio.ByteOrder
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.opengles.GL10
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sqrt
import kotlin.math.sin
import kotlin.math.tan

// The board viewer: a plot on the GPU (OpenGL ES 3). A plot is uploaded
// once; panning, zooming and orbiting only move a transform, so every
// frame costs the same however big the board. Each layer is drawn as
// coverage (max blending: overlapping copper never darkens), then laid
// over the frame in its colour and opacity; capsules have an analytic edge
// measured in pixels, fills go through the stencil (nonzero). In 3D the
// model is drawn with depth and the board's layers lie on its faces, hidden
// behind parts. Frames are drawn on demand: while a finger moves, a fade
// runs or a fling coasts, and never otherwise.

private const val VIEW = """
uniform vec2 size;
uniform vec2 off;
uniform float scale;
uniform int three;
uniform mat4 mvp;
uniform float z;
uniform float focal;
"""

private const val CAP_V = """#version 300 es
layout(location = 0) in vec4 ab;
layout(location = 1) in float rr;
$VIEW
flat out vec2 A;
flat out vec2 B;
flat out float R;
void main() {
  vec2 a; vec2 b; float r; vec4 ca = vec4(0.0, 0.0, 0.0, 1.0); vec4 cb = ca;
  if (three == 1) {
    ca = mvp * vec4(ab.xy, z, 1.0);
    cb = mvp * vec4(ab.zw, z, 1.0);
    ca.w = max(ca.w, 1e-3); cb.w = max(cb.w, 1e-3);
    a = vec2(ca.x / ca.w * 0.5 + 0.5, 0.5 - ca.y / ca.w * 0.5) * size;
    b = vec2(cb.x / cb.w * 0.5 + 0.5, 0.5 - cb.y / cb.w * 0.5) * size;
    r = max(rr * focal / min(ca.w, cb.w), 0.5);
  } else {
    a = ab.xy * scale + off;
    b = ab.zw * scale + off;
    // never thinner than a pixel (a zero-width KiCad line is a hairline)
    r = max(rr * scale, 0.5);
  }
  vec2 d = b - a;
  float l = length(d);
  vec2 dir = l > 0.001 ? d / l : vec2(1.0, 0.0);
  vec2 n = vec2(-dir.y, dir.x);
  float pad = r + 1.0;
  bool bend = (gl_VertexID & 1) == 1;
  vec2 q = (bend ? b + dir * pad : a - dir * pad) + n * (((gl_VertexID & 2) == 2 ? 1.0 : -1.0) * pad);
  A = a; B = b; R = r;
  vec2 c = q / size * 2.0 - 1.0;
  if (three == 1) {
    vec4 e = bend ? cb : ca;
    gl_Position = vec4(c.x * e.w, -c.y * e.w, e.z, e.w);
  } else {
    gl_Position = vec4(c.x, -c.y, 0.0, 1.0);
  }
}"""

// the edge is measured at the pixel itself
private const val CAP_F = """#version 300 es
precision highp float;
flat in vec2 A;
flat in vec2 B;
flat in float R;
uniform vec2 size;
uniform float fade;
out vec4 o;
void main() {
  vec2 p = vec2(gl_FragCoord.x, size.y - gl_FragCoord.y);
  vec2 pa = p - A, ba = B - A;
  float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
  float d = length(pa - ba * h);
  o = vec4(clamp(R + 0.5 - d, 0.0, 1.0) * fade);
}"""

private const val FILL_V = """#version 300 es
layout(location = 0) in vec2 pt;
$VIEW
void main() {
  if (three == 1) {
    gl_Position = mvp * vec4(pt, z, 1.0);
  } else {
    vec2 c = (pt * scale + off) / size * 2.0 - 1.0;
    gl_Position = vec4(c.x, -c.y, 0.0, 1.0);
  }
}"""

private const val FILL_F = """#version 300 es
precision mediump float;
uniform float fade;
out vec4 o;
void main() { o = vec4(fade); }"""

// Same linear-colour studio estimates as the web; not measured materials.
private const val MESH_V = """#version 300 es
precision highp float;
layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 nrm;
layout(location = 2) in vec3 col;
uniform mat4 mvp;
out vec3 vc; out vec3 vn; out vec3 vp; out vec3 vm;
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
void main() { gl_Position=mvp*vec4(pos,1.0); vc=col; vn=nrm*vec3(1.0,-1.0,1.0); vp=pos*vec3(1.0,-1.0,1.0); vm=estimate(col); }"""

private const val MESH_F = """#version 300 es

#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
in vec3 vc; in vec3 vn; in vec3 vp; in vec3 vm;
out vec4 o;
uniform vec3 eye; uniform vec3 right; uniform vec3 up; uniform vec3 forward;
uniform float exposure; uniform float orthographic; uniform vec3 thermal;
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
  float rough=vm.x, metal=vm.y, nv=max(dot(N,V),0.0);
  vec3 base=max(vc,vec3(0.0)), f0=mix(vec3(0.04),base,metal);
  vec3 key=normalize(-right*0.55+up*0.75-forward*0.65);
  vec3 fill=normalize(right*0.85+up*0.25-forward*0.35);
  vec3 rgb=base*(1.0-metal)*(mix(0.27,0.74,step(0.8,rough))+0.08*max(dot(N,up),0.0));
  rgb+=light(N,V,key,base,f0,rough,metal)*1.5;
  rgb+=light(N,V,fill,base,f0,rough,metal)*0.65;
  rgb+=studio(reflect(-V,N),rough,metal)*fresnel(f0,nv)*(1.0-rough*0.35);
  rgb+=studio(reflect(-V,N),0.3,0.0)*fresnel(vec3(0.04),nv)*vm.z;
  // A display-only thermal overlay. No claim of per-part temperature/flow.
  float heat=clamp((thermal.y-25.0)/235.0,0.0,1.0);
  vec3 tint=mix(vec3(0.12,0.32,0.85),vec3(1.0,0.15,0.02),heat);
  rgb=mix(rgb,mix(rgb,tint,0.44)+vec3(0.1)*thermal.z*metal,thermal.x);
  rgb*=exp2(exposure);
  // Soft highlight shoulder; retain diffuse whites and dark material detail.
  rgb=rgb/(vec3(1.0)+max(rgb-vec3(0.8),vec3(0.0))*0.28);
  o=vec4(clamp(srgb(rgb),0.0,1.0),1.0);
}"""

private const val QUAD_V = """#version 300 es
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}"""

private const val QUAD_F = """#version 300 es
precision mediump float;
uniform sampler2D cov;
uniform vec4 color;
out vec4 o;
void main() {
  float a = texelFetch(cov, ivec2(gl_FragCoord.xy), 0).r * color.a;
  o = vec4(color.rgb * a, a);
}"""

private const val COPY_F = """#version 300 es
precision mediump float;
uniform sampler2D scene;
out vec4 o;
void main() { o = texelFetch(scene, ivec2(gl_FragCoord.xy), 0); }"""

private class Layer(val layer: Int, val color: FloatArray, val caps: IntRange, val freshCaps: IntRange, val tris: IntRange, val freshTris: IntRange)

private class Program(v: String, f: String) {
    val id: Int = glCreateProgram()
    private val at = HashMap<String, Int>()
    init {
        for ((kind, src) in listOf(GL_VERTEX_SHADER to v, GL_FRAGMENT_SHADER to f)) {
            val s = glCreateShader(kind)
            glShaderSource(s, src)
            glCompileShader(s)
            val ok = IntArray(1)
            glGetShaderiv(s, GL_COMPILE_STATUS, ok, 0)
            if (ok[0] == 0) throw IllegalStateException("plot shader: " + glGetShaderInfoLog(s))
            glAttachShader(id, s)
        }
        glLinkProgram(id)
        val linked = IntArray(1); glGetProgramiv(id, GL_LINK_STATUS, linked, 0)
        check(linked[0] != 0) { "plot program: " + glGetProgramInfoLog(id) }
    }
    fun at(name: String) = at.getOrPut(name) { glGetUniformLocation(id, name) }
}

private fun norm(v: FloatArray) {
    val l = max(hypot(hypot(v[0], v[1]), v[2]), 1e-12f)
    for (i in 0..2) v[i] /= l
}

private fun cross(a: FloatArray, b: FloatArray) =
    floatArrayOf(a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])

// v turned by an angle about a unit axis (Rodrigues)
private fun turned(v: FloatArray, k: FloatArray, a: Float): FloatArray {
    val c = cos(a); val s = sin(a); val kv = cross(k, v); val d = (k[0] * v[0] + k[1] * v[1] + k[2] * v[2]) * (1 - c)
    return FloatArray(3) { v[it] * c + kv[it] * s + k[it] * d }
}

// a 3D camera turning freely about a pivot (a board's rectangle's centre), like
// SolidWorks, in a right-handed world where the board's y is flipped
// (KiCad's y points down the screen), micrometres. r, u, f: the view's
// right, up and forward; sx, sy: the pan in the view plane; dist: from the
// eye to the pivot's depth. Gestures copy it, change the copy and swap it in.
class Orbit(val pivot: FloatArray = floatArrayOf(0f, 0f, 0f), var r: FloatArray = floatArrayOf(1f, 0f, 0f),
            var u: FloatArray = floatArrayOf(0f, 0f, 1f), var f: FloatArray = floatArrayOf(0f, 1f, 0f),
            var sx: Float = 0f, var sy: Float = 0f, var dist: Float = 100_000f, var fov: Float = 35f, var home: Float = 0f) {
    fun copy() = Orbit(pivot.copyOf(), r.copyOf(), u.copyOf(), f.copyOf(), sx, sy, dist, fov, home)
    fun target() = FloatArray(3) { pivot[it] + r[it] * sx + u[it] * sy }
    fun eye(): FloatArray { val t = target(); return FloatArray(3) { t[it] - f[it] * dist } }

    // world units per pixel at the pivot's depth
    fun unit(h: Int) = dist * 2 * tan(fov * Math.PI.toFloat() / 360f) / max(h, 1)

    // looking at the pivot from yaw and pitch (radians), world z up
    fun aim(yaw: Float, pitch: Float): Orbit {
        f = floatArrayOf(-sin(yaw) * cos(pitch), cos(yaw) * cos(pitch), -sin(pitch))
        u = floatArrayOf(0f, 0f, 1f)
        return square()
    }

    // orthonormal again (no drift): f, then r = f x u, u = r x f
    private fun square(): Orbit {
        norm(f)
        r = cross(f, u); norm(r)
        u = cross(r, f); norm(u)
        return this
    }

    // the view turned by a about a unit axis (the model turns by -a)
    private fun turn(k: FloatArray, a: Float): Orbit {
        r = turned(r, k, a); u = turned(u, k, a); f = turned(f, k, a)
        return square()
    }

    // a finger moved (mx, my) on screen, radians per unit: the model under it follows
    fun spin(mx: Float, my: Float, k: Float): Orbit {
        val l = hypot(mx, my)
        if (l < 1e-6f) return this
        return turn(FloatArray(3) { (u[it] * mx + r[it] * my) / l }, -l * k)
    }

    // the model turned clockwise on screen by a
    fun roll(a: Float) = turn(f.copyOf(), -a)

    // the point under the fingers at the pivot's depth moves with them (s: world per pixel)
    fun pan(mx: Float, my: Float, s: Float): Orbit { sx -= mx * s; sy += my * s; return this }

    // dist over k, keeping the point at (x, y) world units from the view's
    // centre where it is; within 1/60 and 20 times the fitted distance (home),
    // so any model's size zooms alike
    fun zoom(k: Float, x: Float, y: Float): Orbit {
        val lo = if (home > 0f) home / 60f else 2_000f
        val hi = if (home > 0f) home * 20f else 5_000_000f
        val d = min(max(dist / k, lo), hi)
        val q = d / dist
        sx += x * (1 - q); sy += y * (1 - q); dist = d
        return this
    }

    // board micrometres to clip space
    fun mvp(aspect: Float): FloatArray {
        val p = FloatArray(16); val v = FloatArray(16); val pv = FloatArray(16); val m = FloatArray(16); val out = FloatArray(16)
        Matrix.perspectiveM(p, 0, fov, aspect, max(dist * 0.01f, 1f), dist * 20f + 1_000_000f)
        val e = eye(); val t = target()
        Matrix.setLookAtM(v, 0, e[0], e[1], e[2], t[0], t[1], t[2], u[0], u[1], u[2])
        Matrix.multiplyMM(pv, 0, p, 0, v, 0)
        Matrix.setIdentityM(m, 0)
        Matrix.scaleM(m, 0, 1f, -1f, 1f)
        Matrix.multiplyMM(out, 0, pv, 0, m, 0)
        return out
    }
}

class PlotRenderer : GLSurfaceView.Renderer {
    @Volatile var scale = 0.01f
    @Volatile var offX = 0f
    @Volatile var offY = 0f
    @Volatile var fade = 1f
    @Volatile var bg = floatArrayOf(0f, 0f, 0f)
    // each layer's colour on the viewer's light ground (empty: the chunks' own)
    @Volatile var look = IntArray(0)
    // the layers the user turned off, a bit each
    @Volatile var off = 0
    private fun shown(layer: Int) = layer !in 0..31 || (off shr layer) and 1 == 0
    // the layer focus (core/layers.bend): layers drawn at a quarter, a bit
    // each, and the active one, drawn after the rest (255 none)
    @Volatile var dim = 0
    @Volatile var lift = 255
    private fun dimmed(layer: Int) = layer in 0..31 && (dim shr layer) and 1 == 1
    @Volatile var three = false
    @Volatile var orbit = Orbit()
    @Volatile var thermal = floatArrayOf(0f, 25f, 0f)
    @Volatile var exposure = 0f
    @Volatile var thick = 1600f
    @Volatile var top = IntArray(0)
    @Volatile var bottom = IntArray(0)
    private lateinit var cap: Program
    private lateinit var fill: Program
    private lateinit var quad: Program
    private lateinit var copy: Program
    private lateinit var meshP: Program
    // capsules, fill vertices, the model, the slab, the highlight's capsules and fills
    private val bufs = IntArray(6)
    private val vao = IntArray(1)
    // the scene, coverage, and the model's multisampled target
    private val fbo = IntArray(3)
    private val tex = IntArray(2)
    // depth and stencil; the multisampled colour, depth and stencil
    private val rb = IntArray(3)
    private var samples = 0
    private var ms = 0
    private var msSize = 0L
    // the camera this frame is drawn with
    private var cam = Orbit()
    private var w = 0
    private var h = 0
    private var layers: List<Layer> = emptyList()
    private var meshCount = 0
    private var slabCount = 0
    private var hiCaps = 0
    private var hiTris = 0
    private var hiLayer = -1

    override fun onSurfaceCreated(gl: GL10?, config: EGLConfig?) {
        cap = Program(CAP_V, CAP_F)
        fill = Program(FILL_V, FILL_F)
        quad = Program(QUAD_V, QUAD_F)
        copy = Program(QUAD_V, COPY_F)
        meshP = Program(MESH_V, MESH_F)
        glGenBuffers(6, bufs, 0)
        glGenVertexArrays(1, vao, 0)
        glGenFramebuffers(3, fbo, 0)
        glGenTextures(2, tex, 0)
        glGenRenderbuffers(3, rb, 0)
        val n = IntArray(1)
        glGetIntegerv(GL_MAX_SAMPLES, n, 0)
        samples = if (n[0] >= 2) min(n[0], 4) else 0
        w = 0
        h = 0
        msSize = 0L
    }

    // the scene (colour) and coverage targets share one depth and stencil
    override fun onSurfaceChanged(gl: GL10?, width: Int, height: Int) {
        w = width
        h = height
        for ((i, fmt) in listOf(GL_RGBA8 to GL_RGBA, GL_R8 to GL_RED).withIndex()) {
            glBindTexture(GL_TEXTURE_2D, tex[i])
            glTexImage2D(GL_TEXTURE_2D, 0, fmt.first, w, h, 0, fmt.second, GL_UNSIGNED_BYTE, null)
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST)
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST)
        }
        glBindRenderbuffer(GL_RENDERBUFFER, rb[0])
        glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH24_STENCIL8, w, h)
        for (i in 0..1) {
            glBindFramebuffer(GL_FRAMEBUFFER, fbo[i])
            glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, tex[i], 0)
            glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_STENCIL_ATTACHMENT, GL_RENDERBUFFER, rb[0])
        }
        glBindFramebuffer(GL_FRAMEBUFFER, 0)
    }

    // the model's 4x multisampled target, made when 3D is first drawn at this size
    private fun multisample() {
        val size = (w.toLong() shl 32) or h.toLong()
        if (msSize == size) return
        msSize = size
        ms = samples
        if (ms == 0) return
        glBindRenderbuffer(GL_RENDERBUFFER, rb[1])
        glRenderbufferStorageMultisample(GL_RENDERBUFFER, ms, GL_RGBA8, w, h)
        glBindRenderbuffer(GL_RENDERBUFFER, rb[2])
        glRenderbufferStorageMultisample(GL_RENDERBUFFER, ms, GL_DEPTH24_STENCIL8, w, h)
        glBindFramebuffer(GL_FRAMEBUFFER, fbo[2])
        glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_RENDERBUFFER, rb[1])
        glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_STENCIL_ATTACHMENT, GL_RENDERBUFFER, rb[2])
        if (glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE) ms = 0
        glBindFramebuffer(GL_FRAMEBUFFER, fbo[0])
    }

    private fun put(buf: Int, data: FloatArray) {
        val b = ByteBuffer.allocateDirect(max(data.size, 1) * 4).order(ByteOrder.nativeOrder()).asFloatBuffer()
        b.put(data).position(0)
        glBindBuffer(GL_ARRAY_BUFFER, buf)
        glBufferData(GL_ARRAY_BUFFER, data.size * 4, b, GL_STATIC_DRAW)
    }

    // a plot's chunks into two buffers, layer by layer (the hub sends them
    // in paint order); runs on the GL thread
    fun load(chunks: List<PlotChunk>, fresh: Set<Int>) {
        val caps = Floats()
        val tris = Floats()
        val out = ArrayList<Layer>()
        var i = 0
        while (i < chunks.size) {
            val first = chunks[i]
            var j = i
            while (j < chunks.size && chunks[j].layer == first.layer && chunks[j].own == first.own &&
                (!first.own || (chunks[j].color == first.color && chunks[j].alpha == first.alpha))) j++
            val c0 = caps.size / 5
            val t0 = tris.size / 2
            for (k in i until j) if (k !in fresh) { caps.addAll(chunks[k].caps); tris.addAll(chunks[k].tris) }
            val c1 = caps.size / 5
            val t1 = tris.size / 2
            for (k in i until j) if (k in fresh) { caps.addAll(chunks[k].caps); tris.addAll(chunks[k].tris) }
            val rgb = if (!first.own && first.layer in look.indices) look[first.layer] else first.color
            out.add(Layer(first.layer, floatArrayOf(((rgb shr 16) and 255) / 255f, ((rgb shr 8) and 255) / 255f, (rgb and 255) / 255f, first.alpha),
                c0 until c1, c1 until caps.size / 5, t0 until t1, t1 until tris.size / 2))
            i = j
        }
        put(bufs[0], caps.array())
        put(bufs[1], tris.array())
        layers = out
    }

    private fun solid(verts: FloatArray, colors: IntArray): FloatArray {
        val n = colors.size
        val out = FloatArray(n * 9)
        for (k in 0 until n) {
            System.arraycopy(verts, k * 6, out, k * 9, 6)
            out[k * 9 + 6] = ((colors[k] shr 16) and 255) / 255f
            out[k * 9 + 7] = ((colors[k] shr 8) and 255) / 255f
            out[k * 9 + 8] = (colors[k] and 255) / 255f
        }
        return out
    }

    // the 3D model (GL thread)
    fun mesh(m: PlotMesh?) {
        if (m == null || m.colors.isEmpty()) { meshCount = 0; return }
        put(bufs[2], solid(m.verts, m.colors))
        meshCount = m.colors.size
    }

    // the board as a plain slab (before its model arrives; GL thread)
    fun slab(box: FloatArray, color: Int) {
        if (box.size < 4) { slabCount = 0; return }
        val x0 = box[0]; val y0 = box[1]; val x1 = box[2]; val y1 = box[3]; val z1 = thick
        val p = arrayOf(floatArrayOf(x0, y0, 0f), floatArrayOf(x1, y0, 0f), floatArrayOf(x1, y1, 0f), floatArrayOf(x0, y1, 0f),
            floatArrayOf(x0, y0, z1), floatArrayOf(x1, y0, z1), floatArrayOf(x1, y1, z1), floatArrayOf(x0, y1, z1))
        val faces = arrayOf(intArrayOf(0, 1, 2, 3), intArrayOf(4, 5, 6, 7), intArrayOf(0, 1, 5, 4), intArrayOf(1, 2, 6, 5), intArrayOf(2, 3, 7, 6), intArrayOf(3, 0, 4, 7))
        val v = Floats()
        val c = ArrayList<Int>()
        for (f in faces) {
            val a = p[f[0]]; val b = p[f[1]]; val d = p[f[2]]
            val ux = b[0] - a[0]; val uy = b[1] - a[1]; val uz = b[2] - a[2]
            val vx = d[0] - a[0]; val vy = d[1] - a[1]; val vz = d[2] - a[2]
            val nx = uy * vz - uz * vy; val ny = uz * vx - ux * vz; val nz = ux * vy - uy * vx
            val l = max(hypot(hypot(nx, ny), nz), 1e-6f)
            for (k in intArrayOf(0, 1, 2, 0, 2, 3)) { val q = p[f[k]]; v.add(q[0], q[1], q[2], nx / l, ny / l, nz / l); c.add(color) }
        }
        put(bufs[3], solid(v.array(), c.toIntArray()))
        slabCount = c.size
    }

    // the picked piece, drawn bright over everything (GL thread)
    fun highlight(c: PlotChunk?, p: PlotChunk.Piece?) {
        if (c == null || p == null) { hiCaps = 0; hiTris = 0; hiLayer = -1; return }
        put(bufs[4], c.caps.copyOfRange(p.caps.first * 5, (p.caps.last + 1) * 5))
        put(bufs[5], c.tris.copyOfRange(p.tris.first * 6, (p.tris.last + 1) * 6))
        hiCaps = p.caps.count()
        hiTris = p.tris.count() * 3
        hiLayer = c.layer
    }

    private var mvp = FloatArray(16)
    private var z = 0f

    private fun view(p: Program) {
        glUniform2f(p.at("size"), w.toFloat(), h.toFloat())
        glUniform2f(p.at("off"), offX, offY)
        glUniform1f(p.at("scale"), scale)
        glUniform1i(p.at("three"), if (three) 1 else 0)
        glUniformMatrix4fv(p.at("mvp"), 1, false, mvp, 0)
        glUniform1f(p.at("z"), z)
        glUniform1f(p.at("focal"), 1f / tan(orbit.fov * Math.PI.toFloat() / 360f) * h / 2f)
    }

    // one layer's coverage (fills by stencil, then capsules), laid over the scene
    private fun layer(color: FloatArray, capBuf: Int, caps: List<Pair<IntRange, Float>>, triBuf: Int, tris: List<Pair<IntRange, Float>>) {
        glBindFramebuffer(GL_FRAMEBUFFER, fbo[1])
        glClearColor(0f, 0f, 0f, 0f)
        glClearStencil(0)
        glStencilMask(0xFF)
        glColorMask(true, true, true, true)
        glClear(GL_COLOR_BUFFER_BIT or GL_STENCIL_BUFFER_BIT)
        glEnable(GL_BLEND)
        glBlendEquation(GL_MAX)
        glBlendFunc(GL_ONE, GL_ONE)
        if (three) { glEnable(GL_DEPTH_TEST); glDepthFunc(GL_LEQUAL); glDepthMask(false) } else glDisable(GL_DEPTH_TEST)
        glUseProgram(fill.id)
        view(fill)
        glBindBuffer(GL_ARRAY_BUFFER, triBuf)
        glVertexAttribDivisor(0, 0)
        glDisableVertexAttribArray(1)
        glDisableVertexAttribArray(2)
        glEnableVertexAttribArray(0)
        for ((r, f) in tris) {
            if (r.isEmpty()) continue
            glUniform1f(fill.at("fade"), f)
            glVertexAttribPointer(0, 2, GL_FLOAT, false, 8, r.first * 8)
            glEnable(GL_STENCIL_TEST)
            glColorMask(false, false, false, false)
            glStencilFunc(GL_ALWAYS, 0, 0xFF)
            glStencilOpSeparate(GL_FRONT, GL_KEEP, GL_KEEP, GL_INCR_WRAP)
            glStencilOpSeparate(GL_BACK, GL_KEEP, GL_KEEP, GL_DECR_WRAP)
            glDrawArrays(GL_TRIANGLES, 0, r.last - r.first + 1)
            glColorMask(true, true, true, true)
            glStencilFunc(GL_NOTEQUAL, 0, 0xFF)
            glStencilOp(GL_KEEP, GL_KEEP, GL_ZERO)
            glDrawArrays(GL_TRIANGLES, 0, r.last - r.first + 1)
            glDisable(GL_STENCIL_TEST)
        }
        glUseProgram(cap.id)
        view(cap)
        glBindBuffer(GL_ARRAY_BUFFER, capBuf)
        glEnableVertexAttribArray(0)
        glEnableVertexAttribArray(1)
        glVertexAttribDivisor(0, 1)
        glVertexAttribDivisor(1, 1)
        for ((r, f) in caps) {
            if (r.isEmpty()) continue
            glUniform1f(cap.at("fade"), f)
            glVertexAttribPointer(0, 4, GL_FLOAT, false, 20, r.first * 20)
            glVertexAttribPointer(1, 1, GL_FLOAT, false, 20, r.first * 20 + 16)
            glDrawArraysInstanced(GL_TRIANGLE_STRIP, 0, 4, r.last - r.first + 1)
        }
        glVertexAttribDivisor(0, 0)
        glVertexAttribDivisor(1, 0)
        glDisableVertexAttribArray(0)
        glDisableVertexAttribArray(1)
        glDisable(GL_DEPTH_TEST)
        // the layer over the scene
        glBindFramebuffer(GL_FRAMEBUFFER, fbo[0])
        glBlendEquation(GL_FUNC_ADD)
        glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA)
        glUseProgram(quad.id)
        glActiveTexture(GL_TEXTURE0)
        glBindTexture(GL_TEXTURE_2D, tex[1])
        glUniform1i(quad.at("cov"), 0)
        glUniform4f(quad.at("color"), color[0], color[1], color[2], color[3])
        glDrawArrays(GL_TRIANGLES, 0, 3)
    }

    private fun draw(l: Layer) =
        layer(if (dimmed(l.layer)) l.color.copyOf().also { it[3] *= 0.25f } else l.color,
            bufs[0], listOf(l.caps to 1f, l.freshCaps to fade), bufs[1], listOf(l.tris to 1f, l.freshTris to fade))

    private fun hi() {
        if (hiCaps + hiTris == 0) return
        layer(floatArrayOf(1f, 1f, 1f, 0.8f), bufs[4], listOf(0 until hiCaps to 1f), bufs[5], listOf(0 until hiTris to 1f))
    }

    private fun model(buf: Int, count: Int) {
        glUseProgram(meshP.id)
        glUniformMatrix4fv(meshP.at("mvp"), 1, false, mvp, 0)
        for ((name, v) in listOf("eye" to cam.eye(), "right" to cam.r, "up" to cam.u, "forward" to cam.f, "thermal" to thermal))
            glUniform3f(meshP.at(name), v[0], v[1], v[2])
        glUniform1f(meshP.at("exposure"), exposure)
        glUniform1f(meshP.at("orthographic"), 0f)
        glBindBuffer(GL_ARRAY_BUFFER, buf)
        for (k in 0..2) { glEnableVertexAttribArray(k); glVertexAttribDivisor(k, 0) }
        glVertexAttribPointer(0, 3, GL_FLOAT, false, 36, 0)
        glVertexAttribPointer(1, 3, GL_FLOAT, false, 36, 12)
        glVertexAttribPointer(2, 3, GL_FLOAT, false, 36, 24)
        glDrawArrays(GL_TRIANGLES, 0, count)
        for (k in 0..2) glDisableVertexAttribArray(k)
    }

    override fun onDrawFrame(gl: GL10?) {
        glBindVertexArray(vao[0])
        glViewport(0, 0, w, h)
        glDisable(GL_CULL_FACE)
        glBindFramebuffer(GL_FRAMEBUFFER, fbo[0])
        glClearColor(bg[0], bg[1], bg[2], 1f)
        glClearDepthf(1f)
        glDepthMask(true)
        glClear(GL_COLOR_BUFFER_BIT or GL_DEPTH_BUFFER_BIT or GL_STENCIL_BUFFER_BIT)
        if (three) {
            cam = orbit
            mvp = cam.mvp(w.toFloat() / max(h, 1))
            // the model (or a slab) with depth, then the faces' layers over it,
            // the model multisampled (resolved into the scene)
            val (mb, mc) = if (meshCount > 0) bufs[2] to meshCount else bufs[3] to slabCount
            multisample()
            if (ms > 0) {
                glBindFramebuffer(GL_FRAMEBUFFER, fbo[2])
                glClear(GL_COLOR_BUFFER_BIT or GL_DEPTH_BUFFER_BIT or GL_STENCIL_BUFFER_BIT)
            }
            glEnable(GL_DEPTH_TEST)
            glDepthFunc(GL_LESS)
            glDisable(GL_BLEND)
            if (mc > 0) model(mb, mc)
            if (ms > 0) {
                // resolve the colour; the layers test against depth drawn again
                // single-sampled (cheaper on tilers than storing and resolving
                // the multisampled depth, and exact)
                glBindFramebuffer(GL_READ_FRAMEBUFFER, fbo[2])
                glBindFramebuffer(GL_DRAW_FRAMEBUFFER, fbo[0])
                glBlitFramebuffer(0, 0, w, h, 0, 0, w, h, GL_COLOR_BUFFER_BIT, GL_NEAREST)
                glInvalidateFramebuffer(GL_READ_FRAMEBUFFER, 2, intArrayOf(GL_COLOR_ATTACHMENT0, GL_DEPTH_STENCIL_ATTACHMENT), 0)
                glBindFramebuffer(GL_FRAMEBUFFER, fbo[0])
                glColorMask(false, false, false, false)
                if (mc > 0) model(mb, mc)
                glColorMask(true, true, true, true)
            }
            glDisable(GL_DEPTH_TEST)
            // the board's layers lie on a plain slab until its model comes,
            // which has its own mask, silk and pads (a picked piece still shows)
            for ((face, zz) in listOf(top to thick + 40f, bottom to -40f)) {
                z = zz
                if (meshCount == 0) for (id in face) for (l in layers) if (l.layer == id && shown(l.layer)) draw(l)
                if (hiLayer in face) hi()
            }
        } else {
            // the active layer on top, as KiCad draws it
            for (l in layers) if (shown(l.layer) && l.layer != lift) draw(l)
            for (l in layers) if (shown(l.layer) && l.layer == lift) draw(l)
            hi()
        }
        // the scene to the screen
        glBindFramebuffer(GL_FRAMEBUFFER, 0)
        glDisable(GL_BLEND)
        glDisable(GL_DEPTH_TEST)
        glUseProgram(copy.id)
        glActiveTexture(GL_TEXTURE0)
        glBindTexture(GL_TEXTURE_2D, tex[0])
        glUniform1i(copy.at("scene"), 0)
        glDrawArrays(GL_TRIANGLES, 0, 3)
    }
}

@SuppressLint("ViewConstructor")
class PlotSurface(context: Context) : GLSurfaceView(context) {
    val renderer = PlotRenderer()
    var box = FloatArray(0)
    // the board's rectangle: the 3D view turns about its centre
    var edge = FloatArray(0)
    var margin = 0.9f
    var zmin = 0.5f
    var zmax = 0.5f
    var tap = 14f
    var fadeMs = 220f
    var chunks: List<PlotChunk> = emptyList()
    var onPick: (String) -> Unit = {}
    // a schematic's sheet symbols (micrometres: x0 y0 x1 y1) and what a tap on one opens
    var sheetHits: List<Pair<String, FloatArray>> = emptyList()
    var onSheet: (String) -> Unit = {}
    // review comments: a long press at a place (mm of the file) asks for a
    // note; the pins follow the camera (offX, offY, scale) via onCamera
    var onNote: (Float, Float) -> Unit = { _, _ -> }
    var onCamera: (Float, Float, Float) -> Unit = { _, _, _ -> }
    private var fitted = false
    private var shownFam = ""
    private var fadeFrom = 0L
    private var flingX = 0f
    private var flingY = 0f
    private var last = 0L
    private var ticking = false
    private var shown: PlotFrame? = null
    private var shownMesh: MeshFrame? = null
    private var picked = ""
    private val density = context.resources.displayMetrics.density

    private data class Pose(val scale: Float, val x: Float, val y: Float, val orbit: Orbit, val width: Int, val height: Int)
    private companion object { val poses = LinkedHashMap<String, Pose>() }
    private var stateKey = ""
    private var pendingPose: Pose? = null
    private fun savePose() {
        if (!fitted || stateKey.isEmpty()) return
        poses.remove(stateKey); poses[stateKey] = Pose(renderer.scale, renderer.offX, renderer.offY, renderer.orbit.copy(), width, height)
        while (poses.size > 64) poses.remove(poses.keys.first())
    }
    fun bindSource(key: String) {
        if (key == stateKey) return
        savePose(); stateKey = key; pendingPose = poses[key]
        fitted = false; box = FloatArray(0); edge = FloatArray(0); chunks = emptyList(); shown = null; shownMesh = null; picked = ""
        flingX = 0f; flingY = 0f
        queueEvent { renderer.load(emptyList(), emptySet()); renderer.mesh(null); renderer.highlight(null, null); renderer.slab(FloatArray(0), 0) }
    }
    private fun restorePose() {
        val pose = pendingPose ?: return
        if (width <= 0) return
        renderer.scale = pose.scale; renderer.offX = pose.x + (width - pose.width) / 2f; renderer.offY = pose.y + (height - pose.height) / 2f
        renderer.orbit = pose.orbit.copy(); fitted = true; pendingPose = null
    }
    override fun onDetachedFromWindow() {
        savePose(); Choreographer.getInstance().removeFrameCallback(frame); ticking = false
        super.onDetachedFromWindow()
    }

    init {
        setEGLContextClientVersion(3)
        setEGLConfigChooser(8, 8, 8, 8, 0, 0)
        preserveEGLContextOnPause = true
        isFocusable = true; isFocusableInTouchMode = true
        contentDescription = "Board and model viewer. Arrow keys orbit, shift arrows pan, plus and minus zoom, F fits."
        setRenderer(renderer)
        renderMode = RENDERMODE_WHEN_DIRTY
    }

    private val three get() = renderer.three

    override fun requestRender() {
        super.requestRender()
        onCamera(renderer.offX, renderer.offY, if (three) 0f else renderer.scale)
    }

    // the zoom that shows the whole box, and the range around it (pixels)
    private val fit: Float
        get() = if (box.size < 4 || width == 0) renderer.scale
        else margin * min(width / max(box[2] - box[0], 1f), height / max(box[3] - box[1], 1f))

    fun refit() {
        if (box.size < 4 || width == 0) return
        val s = fit
        renderer.scale = s
        renderer.offX = width / 2f - (box[0] + box[2]) / 2 * s
        renderer.offY = height / 2f - (box[1] + box[3]) / 2 * s
        val o = renderer.orbit
        val e = if (edge.size == 4) edge else box
        val cx = (e[0] + e[2]) / 2
        val cy = (e[1] + e[3]) / 2
        // far enough to show the whole box round the pivot
        val reach = hypot(max(abs(box[0] - cx), abs(box[2] - cx)), max(abs(box[1] - cy), abs(box[3] - cy)))
        // the narrower of the two fields of view takes the whole board
        val half = tan(o.fov * Math.PI.toFloat() / 360f)
        renderer.orbit = Orbit(floatArrayOf(cx, -cy, renderer.thick / 2),
            dist = reach / (half * min(width.toFloat() / max(height, 1), 1f)) * 1.1f, fov = o.fov).aim(0.5f, 0.75f).also { it.home = it.dist }
        fitted = true
        requestRender()
    }

    override fun onSizeChanged(w: Int, h: Int, ow: Int, oh: Int) {
        super.onSizeChanged(w, h, ow, oh)
        val b = shownMesh?.mesh?.box
        if (!fitted) { if (box.size < 4 && b != null && b.size == 6) fitSolid(b) else refit() }
        restorePose()
    }

    fun setThree(on: Boolean) {
        if (renderer.three == on) return
        renderer.three = on
        if (!fitted) refit()
    }

    fun show(f: PlotFrame, bg: Int, slab: Int, look: IntArray) {
        if (f === shown && look.contentEquals(renderer.look)) return
        shown = f
        chunks = f.chunks
        renderer.bg = floatArrayOf(((bg shr 16) and 255) / 255f, ((bg shr 8) and 255) / 255f, (bg and 255) / 255f)
        renderer.look = look
        renderer.thick = if (f.thick > 0) f.thick else 1600f
        val e = if (f.edge.size == 4 && f.edge[0] <= f.edge[2]) f.edge else f.box
        queueEvent { renderer.load(f.chunks, f.fresh); renderer.slab(e, slab) }
        // a new source (board to schematic, another sheet) fits anew; another
        // version of the same file keeps the view where the user left it
        val first = box.isEmpty() || !fitted || f.fam != shownFam
        shownFam = f.fam
        box = f.box
        edge = e
        if (first) { fitted = false; refit() }
        restorePose()
        val animate = f.fresh.isNotEmpty() && android.animation.ValueAnimator.areAnimatorsEnabled()
        fadeFrom = if (animate) f.at else 0
        renderer.fade = if (animate) 0f else 1f
        run()
        requestRender()
    }

    fun mesh(m: MeshFrame?) {
        if (m == null || m === shownMesh) return
        shownMesh = m
        queueEvent { renderer.mesh(m.mesh) }
        // a part alone (no board plot under it) is fitted by its own box
        val b = m.mesh?.box
        if (!fitted && box.size < 4 && b != null && b.size == 6 && width > 0) fitSolid(b)
        restorePose()
        requestRender()
    }

    // the camera round a model's 3D box (x0 y0 z0 x1 y1 z1, the mesh's own
    // axes; the pivot's y is flipped, as refit's is)
    private fun fitSolid(b: FloatArray) {
        val o = renderer.orbit
        val diag = sqrt((b[3] - b[0]) * (b[3] - b[0]) + (b[4] - b[1]) * (b[4] - b[1]) + (b[5] - b[2]) * (b[5] - b[2]))
        val half = tan(o.fov * Math.PI.toFloat() / 360f)
        renderer.orbit = Orbit(floatArrayOf((b[0] + b[3]) / 2, -(b[1] + b[4]) / 2, (b[2] + b[5]) / 2),
            dist = diag / 2 / (half * min(width.toFloat() / max(height, 1), 1f)) * 1.1f, fov = o.fov).aim(0.5f, 0.75f).also { it.home = it.dist }
        fitted = true
    }

    // the picked piece ("chunk,info" of the held chunks), drawn bright
    fun mark(p: String) {
        if (p == picked) return
        picked = p
        val ps = p.split(",").mapNotNull { it.toIntOrNull() }
        val c = if (ps.size == 2) chunks.getOrNull(ps[0]) else null
        val piece = c?.pieces?.firstOrNull { it.info == ps[1] }
        queueEvent { renderer.highlight(c, piece) }
        requestRender()
    }

    private fun zoom(by: Float, x: Float, y: Float) {
        if (three) {
            // toward the point under the fingers
            val o = renderer.orbit
            val s = o.unit(height)
            renderer.orbit = o.copy().zoom(by, (x - width / 2f) * s, (height / 2f - y) * s)
            return
        }
        val s0 = renderer.scale
        val s = min(max(s0 * by, fit * zmin), max(zmax, fit))
        val k = s / s0
        renderer.offX = x - (x - renderer.offX) * k
        renderer.offY = y - (y - renderer.offY) * k
        renderer.scale = s
    }

    // where on the board a point of the view lands (micrometres), and on
    // which face's layers in 3D (the face toward the viewer)
    private fun board(px: Float, py: Float): Pair<FloatArray, IntArray?>? {
        if (!three) return floatArrayOf((px - renderer.offX) / renderer.scale, (py - renderer.offY) / renderer.scale) to null
        val m = renderer.orbit.mvp(width.toFloat() / max(height, 1))
        val inv = FloatArray(16)
        if (!Matrix.invertM(inv, 0, m, 0)) return null
        val nx = px / width * 2 - 1
        val ny = 1 - py / height * 2
        fun at(z: Float): FloatArray {
            val q = FloatArray(4)
            Matrix.multiplyMV(q, 0, inv, 0, floatArrayOf(nx, ny, z, 1f), 0)
            return floatArrayOf(q[0] / q[3], q[1] / q[3], q[2] / q[3])
        }
        val a = at(-1f)
        val b = at(1f)
        val above = renderer.orbit.eye()[2] > renderer.thick / 2
        val z = if (above) renderer.thick + 40 else -40f
        if (kotlin.math.abs(b[2] - a[2]) < 1e-6f) return null
        val t = (z - a[2]) / (b[2] - a[2])
        if (t <= 0) return null
        return floatArrayOf(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t) to (if (above) renderer.top else renderer.bottom)
    }

    // what lies under a point, for Bend to pick from (View.pick)
    fun pick(px: Float, py: Float) {
        val (q, face) = board(px, py) ?: run { onPick("[]"); return }
        // a sheet symbol opens that sheet (the smallest when they nest)
        if (!three) {
            val hit = sheetHits.filter { (_, b) -> q[0] >= b[0] && q[0] <= b[2] && q[1] >= b[1] && q[1] <= b[3] }
                .minByOrNull { (_, b) -> (b[2] - b[0]) * (b[3] - b[1]) }
            if (hit != null) { onSheet(hit.first); return }
        }
        // the finger's reach, in micrometres
        val tol = if (three) tap * density * renderer.orbit.unit(height)
        else tap * density / renderer.scale
        val out = StringBuilder("[")
        for ((ci, c) in chunks.withIndex()) {
            if (face != null && c.layer !in face) continue
            for (p in c.pieces) {
                val b = p.box
                if (q[0] < b[0] - tol || q[0] > b[2] + tol || q[1] < b[1] - tol || q[1] > b[3] + tol) continue
                if (c.distance(p, q[0], q[1]) > tol) continue
                val area = min(max(b[2] - b[0], 1f) * max(b[3] - b[1], 1f) / 100f, 4e9f)
                if (out.length > 1) out.append(',')
                out.append("{\"c\":").append(ci).append(",\"p\":").append(p.info).append(",\"a\":").append(area.toLong()).append(",\"l\":").append(c.layer).append('}')
            }
        }
        onPick(out.append(']').toString())
    }

    private val scaler = ScaleGestureDetector(context, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
        override fun onScale(d: ScaleGestureDetector): Boolean {
            zoom(d.scaleFactor, d.focusX, d.focusY)
            requestRender()
            return true
        }
    })

    private val gestures = GestureDetector(context, object : GestureDetector.SimpleOnGestureListener() {
        override fun onDown(e: MotionEvent): Boolean {
            flingX = 0f
            flingY = 0f
            return true
        }

        override fun onScroll(a: MotionEvent?, b: MotionEvent, dx: Float, dy: Float): Boolean {
            if (three) {
                // one finger turns the model under it, two move it with them
                val o = renderer.orbit.copy()
                renderer.orbit = if (b.pointerCount >= 2) o.pan(-dx, -dy, o.unit(height)) else o.spin(-dx / density, -dy / density, 0.008f)
            } else {
                renderer.offX -= dx
                renderer.offY -= dy
            }
            requestRender()
            return true
        }

        override fun onFling(a: MotionEvent?, b: MotionEvent, vx: Float, vy: Float): Boolean {
            if (three) return false
            flingX = vx
            flingY = vy
            run()
            return true
        }

        override fun onLongPress(e: MotionEvent) {
            if (three || renderer.scale <= 0f) return
            if (AppSettings.haptics) performHapticFeedback(android.view.HapticFeedbackConstants.LONG_PRESS)
            onNote((e.x - renderer.offX) / renderer.scale / 1000f, (e.y - renderer.offY) / renderer.scale / 1000f)
        }

        override fun onSingleTapConfirmed(e: MotionEvent): Boolean {
            pick(e.x, e.y)
            return true
        }

        override fun onDoubleTap(e: MotionEvent): Boolean {
            if (three || renderer.scale > fit * 1.5f) refit() else zoom(3f, e.x, e.y)
            requestRender()
            return true
        }
    })

    // two fingers twisting roll the model about the view axis
    private var twist = Float.NaN

    private fun twisted(e: MotionEvent) {
        if (!three || e.pointerCount != 2 || e.actionMasked != MotionEvent.ACTION_MOVE) { twist = Float.NaN; return }
        val a = atan2(e.getY(1) - e.getY(0), e.getX(1) - e.getX(0))
        val t = twist
        twist = a
        if (t.isNaN()) return
        var d = a - t
        if (d > Math.PI) d -= 2 * Math.PI.toFloat() else if (d < -Math.PI) d += 2 * Math.PI.toFloat()
        renderer.orbit = renderer.orbit.copy().roll(d)
        requestRender()
    }

    override fun onKeyDown(keyCode: Int, event: android.view.KeyEvent): Boolean {
        val delta = when (keyCode) {
            android.view.KeyEvent.KEYCODE_DPAD_LEFT -> -12f to 0f
            android.view.KeyEvent.KEYCODE_DPAD_RIGHT -> 12f to 0f
            android.view.KeyEvent.KEYCODE_DPAD_UP -> 0f to -12f
            android.view.KeyEvent.KEYCODE_DPAD_DOWN -> 0f to 12f
            else -> null
        }
        if (delta != null) {
            if (three) renderer.orbit = renderer.orbit.copy().let {
                if (event.isShiftPressed) it.pan(delta.first, delta.second, it.unit(height)) else it.spin(delta.first, delta.second, .008f)
            } else { renderer.offX += delta.first; renderer.offY += delta.second }
        } else when (keyCode) {
            android.view.KeyEvent.KEYCODE_PLUS, android.view.KeyEvent.KEYCODE_EQUALS -> zoom(1.15f, width / 2f, height / 2f)
            android.view.KeyEvent.KEYCODE_MINUS -> zoom(1 / 1.15f, width / 2f, height / 2f)
            android.view.KeyEvent.KEYCODE_F, android.view.KeyEvent.KEYCODE_MOVE_HOME -> {
                val b = shownMesh?.mesh?.box
                if (box.size < 4 && b != null && b.size == 6) fitSolid(b) else refit()
            }
            else -> return super.onKeyDown(keyCode, event)
        }
        requestRender(); return true
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(e: MotionEvent): Boolean {
        if (e.actionMasked == MotionEvent.ACTION_DOWN) requestFocus()
        twisted(e)
        scaler.onTouchEvent(e)
        gestures.onTouchEvent(e)
        return true
    }

    private val frame = object : Choreographer.FrameCallback {
        override fun doFrame(now: Long) {
            val dt = if (last == 0L) 0f else (now - last) / 1e9f
            last = now
            var busy = false
            if (fadeFrom != 0L) {
                val f = min(1f, (System.currentTimeMillis() - fadeFrom) / fadeMs)
                renderer.fade = f
                if (f >= 1f) fadeFrom = 0 else busy = true
            }
            if (android.animation.ValueAnimator.areAnimatorsEnabled() && hypot(flingX, flingY) > 8f) {
                renderer.offX += flingX * dt
                renderer.offY += flingY * dt
                val k = exp(-5.5f * dt)
                flingX *= k
                flingY *= k
                busy = true
            } else {
                flingX = 0f
                flingY = 0f
            }
            requestRender()
            if (busy) Choreographer.getInstance().postFrameCallback(this) else ticking = false
        }
    }

    // frames by themselves only while a fade or a fling runs
    private fun run() {
        if (ticking) return
        ticking = true
        last = 0L
        Choreographer.getInstance().postFrameCallback(frame)
    }
}
