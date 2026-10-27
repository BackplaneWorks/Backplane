package dev.backplane.mobile

import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.util.concurrent.Callable
import java.util.concurrent.Executors
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sqrt

// Plots from the hub (src/core/plot.bend) as GPU-ready geometry. Nothing
// is decided here: which chunks, their order, colour and opacity come from
// the hub, which piece a tap picks from Bend, the fade and zoom range from
// the screen. A plot lists each chunk as the index of one this app holds
// (the last plot applied) or the chunk itself; applying it is Plot.apply
// (law plot_delta_exact), so `held` always mirrors what the hub thinks
// this connection holds, index for index (a tap names a chunk by it).

// CBOR, as much as plots use (core/plot.bend writes them straight):
// unsigned and negative ints, byte and text strings, arrays, maps. Keys 1
// and 31 are the hub's dictionary words "t" and "key".
object Cbor {
    fun decode(b: ByteArray): Any? {
        var i = 0
        fun u(k: Int) = b[k].toInt() and 255
        fun arg(ai: Int): Long? = when {
            ai < 24 -> ai.toLong()
            ai == 24 -> if (i < b.size) u(i++).toLong() else null
            ai == 25 -> if (i + 1 < b.size) { i += 2; ((u(i - 2) shl 8) or u(i - 1)).toLong() } else null
            ai == 26 -> if (i + 3 < b.size) { i += 4; (u(i - 4).toLong() shl 24) or (u(i - 3).toLong() shl 16) or (u(i - 2).toLong() shl 8) or u(i - 1).toLong() } else null
            else -> null
        }
        fun item(): Any? {
            if (i >= b.size) return null
            val h = u(i++)
            val n = arg(h and 31) ?: return null
            return when (h shr 5) {
                0 -> n
                1 -> -1 - n
                2 -> { val k = n.toInt(); if (i + k > b.size) null else b.copyOfRange(i, i + k).also { i += k } }
                3 -> { val k = n.toInt(); if (i + k > b.size) null else String(b, i, k, Charsets.UTF_8).also { i += k } }
                4 -> ArrayList<Any?>(n.toInt()).apply { repeat(n.toInt()) { add(item()) } }
                5 -> HashMap<String, Any?>().apply {
                    repeat(n.toInt()) {
                        val k = item()
                        val key = when (k) { 1L -> "t"; 31L -> "key"; is Long -> k.toString(); else -> k as? String ?: "" }
                        put(key, item())
                    }
                }
                else -> null
            }
        }
        return item()
    }
}

// Varints (7 bits a byte, low first) and zigzag, as Enc writes them.
class Varints(private val b: ByteArray) {
    private var i = 0
    var valid = true
        private set
    val more get() = valid && i < b.size

    fun next(): Int {
        var acc = 0
        for (sh in 0..28 step 7) {
            if (i >= b.size) { valid = false; return 0 }
            val v = b[i++].toInt() and 255
            if (sh == 28 && v and 0xF0 != 0) { valid = false; return 0 }
            acc = acc or ((v and 127) shl sh)
            if (v < 128) return acc
        }
        valid = false
        return 0
    }

    fun signed(): Int { val z = next(); return (z ushr 1) xor -(z and 1) }
}

private fun Map<String, Any?>.int(k: String, d: Int = 0) = (this[k] as? Long)?.toInt() ?: d

// One chunk, decoded once: capsules (ax ay bx by r, micrometres; a dot is
// a capsule of no length) and fill triangles (x y, three per triangle);
// and its pieces, for tap hit tests: each piece's info index (what the hub
// is asked), its capsules and triangles, and its box.
// own: a tint (fills with a width), drawn in its own colour, not its layer's
class PlotChunk(val layer: Int, val color: Int, val alpha: Float, val own: Boolean = false) {
    class Piece(val info: Int, val caps: IntRange, val tris: IntRange, val box: FloatArray)

    var caps = FloatArray(0)
    var tris = FloatArray(0)
    val pieces = ArrayList<Piece>()

    // how close a piece comes to (x, y), micrometres (0 inside a fill)
    fun distance(p: Piece, x: Float, y: Float): Float {
        var best = Float.MAX_VALUE
        for (k in p.caps) {
            val o = k * 5
            val ax = caps[o]; val ay = caps[o + 1]; val bx = caps[o + 2]; val by = caps[o + 3]
            val dx = bx - ax; val dy = by - ay
            val h = ((x - ax) * dx + (y - ay) * dy) / max(dx * dx + dy * dy, 1e-6f)
            val t = min(max(h, 0f), 1f)
            val ex = x - ax - dx * t; val ey = y - ay - dy * t
            best = min(best, sqrt(ex * ex + ey * ey) - caps[o + 4])
        }
        // inside a polygon: its fan's signed triangles containing the point sum to non-zero
        var wind = 0
        for (k in p.tris) {
            val o = k * 6
            val ax = tris[o]; val ay = tris[o + 1]; val bx = tris[o + 2]; val by = tris[o + 3]; val cx = tris[o + 4]; val cy = tris[o + 5]
            fun side(ux: Float, uy: Float, vx: Float, vy: Float) = (vx - ux) * (y - uy) - (vy - uy) * (x - ux)
            val s0 = side(ax, ay, bx, by); val s1 = side(bx, by, cx, cy); val s2 = side(cx, cy, ax, ay)
            if ((s0 >= 0 && s1 >= 0 && s2 >= 0) || (s0 <= 0 && s1 <= 0 && s2 <= 0)) {
                wind += if ((bx - ax) * (cy - ay) - (by - ay) * (cx - ax) >= 0) 1 else -1
            }
        }
        return if (wind != 0) 0f else best
    }

    companion object {
        fun decode(o: Map<String, Any?>): PlotChunk {
            val mode = o.int("m", 1)
            val c = PlotChunk(o.int("l"), o.int("c"), o.int("a", 255) / 255f, mode == 0 && o.int("w") != 0)
            val r = o.int("w") / 2f
            val v = Varints(o["d"] as? ByteArray ?: ByteArray(0))
            val caps = Floats()
            val tris = Floats()
            var px = 0
            var py = 0
            while (v.more) {
                val info = v.next()
                val c0 = caps.size / 5
                val t0 = tris.size / 6
                val pts = Floats()
                if (mode == 2) {
                    px += v.signed(); py += v.signed()
                    pts.add(px.toFloat(), py.toFloat())
                    caps.add(px.toFloat(), py.toFloat(), px.toFloat(), py.toFloat(), r)
                } else {
                    val n = v.next()
                    var k = 0
                    while (k < n && v.more) { px += v.signed(); py += v.signed(); pts.add(px.toFloat(), py.toFloat()); k++ }
                    val p = pts.array()
                    if (mode == 0) fan(p, p.size / 2, tris) else path(p, p.size / 2, r, caps)
                }
                val p = pts.array()
                var lx = Float.MAX_VALUE; var ly = Float.MAX_VALUE; var hx = -Float.MAX_VALUE; var hy = -Float.MAX_VALUE
                for (k in 0 until p.size / 2) { lx = min(lx, p[2 * k]); ly = min(ly, p[2 * k + 1]); hx = max(hx, p[2 * k]); hy = max(hy, p[2 * k + 1]) }
                val pad = if (mode == 0) 0f else r
                c.pieces.add(Piece(info, c0 until caps.size / 5, t0 until tris.size / 6, floatArrayOf(lx - pad, ly - pad, hx + pad, hy + pad)))
            }
            c.caps = caps.array()
            c.tris = tris.array()
            return c
        }

        private fun path(p: FloatArray, n: Int, r: Float, caps: Floats) {
            if (n == 1) { caps.add(p[0], p[1], p[0], p[1], r); return }
            for (k in 1 until n) caps.add(p[2 * k - 2], p[2 * k - 1], p[2 * k], p[2 * k + 1], r)
        }

        // a polygon as a triangle fan, turned counter-clockwise, so the
        // stencil's winding count fills every polygon of a layer as one union
        private fun fan(p: FloatArray, n: Int, tris: Floats) {
            if (n < 3) return
            var area = 0f
            for (k in 0 until n) {
                val j = (k + 1) % n
                area += p[2 * k] * p[2 * j + 1] - p[2 * j] * p[2 * k + 1]
            }
            for (k in 1 until n - 1) {
                val a = if (area >= 0) k else k + 1
                val b = if (area >= 0) k + 1 else k
                tris.add(p[0], p[1], p[2 * a], p[2 * a + 1], p[2 * b], p[2 * b + 1])
            }
        }
    }
}

class Floats {
    private var a = FloatArray(64)
    var size = 0
        private set

    fun add(vararg xs: Float) {
        if (size + xs.size > a.size) a = a.copyOf(maxOf(a.size * 2, size + xs.size))
        for (x in xs) a[size++] = x
    }

    fun addAll(xs: FloatArray) {
        if (size + xs.size > a.size) a = a.copyOf(maxOf(a.size * 2, size + xs.size))
        System.arraycopy(xs, 0, a, size, xs.size)
        size += xs.size
    }

    fun array(): FloatArray = a.copyOf(size)
}

// A 3D model (Solid): triangles in board axes, micrometres, with a colour
// each; normals are the triangles' own.
class PlotMesh(val verts: FloatArray, val colors: IntArray, val box: FloatArray) {
    companion object {
        fun decode(o: Map<String, Any?>, cancelled: () -> Boolean = { false }): PlotMesh {
            val n = o.int("n")
            val bytes = o["mesh"] as? ByteArray ?: ByteArray(0)
            require(n > 0 && n <= (64 shl 20) / 84 && bytes.size >= n * 10) { "Invalid or oversized model" }
            val b = (o["box3"] as? List<*>)?.map { ((it as? Long) ?: 0L).toInt() * 10f } ?: emptyList()
            val v = Varints(o["mesh"] as? ByteArray ?: ByteArray(0))
            val verts = FloatArray(n * 18)
            val colors = IntArray(n * 3)
            var color = 0
            var px = 0; var py = 0; var pz = 0
            var w = 0
            val p = FloatArray(9)
            for (t in 0 until n) {
                if (t % 256 == 0 && cancelled()) throw java.util.concurrent.CancellationException()
                color = color xor v.next()
                for (k in 0 until 3) {
                    px += v.signed(); py += v.signed(); pz += v.signed()
                    p[3 * k] = px * 10f; p[3 * k + 1] = py * 10f; p[3 * k + 2] = pz * 10f
                }
                require(v.valid) { "Incomplete model" }
                val ux = p[3] - p[0]; val uy = p[4] - p[1]; val uz = p[5] - p[2]
                val vx = p[6] - p[0]; val vy = p[7] - p[1]; val vz = p[8] - p[2]
                var nx = uy * vz - uz * vy; var ny = uz * vx - ux * vz; var nz = ux * vy - uy * vx
                val l = sqrt(nx * nx + ny * ny + nz * nz)
                if (l > 0) { nx /= l; ny /= l; nz /= l } else { nx = 0f; ny = 0f; nz = 1f }
                for (k in 0 until 3) {
                    verts[w * 6] = p[3 * k]; verts[w * 6 + 1] = p[3 * k + 1]; verts[w * 6 + 2] = p[3 * k + 2]
                    verts[w * 6 + 3] = nx; verts[w * 6 + 4] = ny; verts[w * 6 + 5] = nz
                    colors[w] = color
                    w++
                }
            }
            require(!v.more && v.valid) { "Unexpected model bytes" }
            return PlotMesh(verts, colors, if (b.size == 6) b.toFloatArray() else FloatArray(0))
        }
    }
}

// What the renderer draws: the chunks in paint order and which of them
// just arrived (they fade in), the box, the board's rectangle (empty
// from an older hub, or x0 > x1 for a schematic) and thickness, or why
// there is nothing to draw. fam: the file the source is a version of
// (versions of one file share it; the key itself when the hub names none).
class PlotFrame(
    val key: String, val box: FloatArray, val edge: FloatArray, val thick: Float, val chunks: List<PlotChunk>, val fresh: Set<Int>,
    val none: String, val at: Long, val fam: String = key,
)

class MeshFrame(val key: String, val mesh: PlotMesh?, val none: String, val at: Long)

// Exact bytes identify a revision; no hash collision can reuse old geometry.
// The demultiplexer stays free to cancel work while one mesh is being built.
private class PlotMeshAssets {
    private data class Entry(val n: Int, val bytes: ByteArray, val frame: MeshFrame)
    private val entries = LinkedHashMap<String, Entry>(8, 0.75f, true)
    private val tickets = HashMap<String, Long>()
    private var serial = 0L
    private var generation = 0
    @Synchronized fun epoch() = generation
    @Synchronized fun reset() { generation++; serial++; entries.clear(); tickets.clear() }
    @Synchronized fun issue(key: String, epoch: Int): Long? {
        if (generation != epoch) return null
        serial++; tickets[key] = serial; return serial
    }
    @Synchronized fun current(key: String, ticket: Long) = tickets[key] == ticket
    @Synchronized fun frame(key: String) = entries[key]?.frame
    @Synchronized fun same(key: String, n: Int, bytes: ByteArray): PlotMesh? =
        entries[key]?.takeIf { it.n == n && it.bytes.contentEquals(bytes) }?.frame?.mesh
    @Synchronized fun put(key: String, n: Int, bytes: ByteArray, mesh: PlotMesh, ticket: Long) {
        if (!current(key, ticket)) return
        entries[key] = Entry(n, bytes, MeshFrame(key, mesh, "", System.currentTimeMillis()))
        fun size() = entries.values.sumOf { it.bytes.size.toLong() + (it.frame.mesh?.verts?.size ?: 0) * 4L + (it.frame.mesh?.colors?.size ?: 0) * 4L }
        while (entries.size > 4 || (size() > (64L shl 20) && entries.size > 1)) entries.remove(entries.keys.first())
    }
}

class PlotStore {
    var frame by mutableStateOf<PlotFrame?>(null)
        private set
    var mesh by mutableStateOf<MeshFrame?>(null)
        private set
    private var held: List<PlotChunk> = emptyList()
    private var generation = 0
    private val assets = PlotMeshAssets()
    private val meshQueue = Executors.newSingleThreadExecutor()
    fun cachedMesh(key: String): MeshFrame? = assets.frame(key)
    private val main = Handler(Looper.getMainLooper())
    // one plot at a time, in arrival order; its new chunks decode side by side
    private val queue = Executors.newSingleThreadExecutor()
    private val pool = Executors.newFixedThreadPool(maxOf(2, Runtime.getRuntime().availableProcessors() - 1))

    // a new connection holds nothing
    fun reset() {
        generation += 1
        assets.reset()
        held = emptyList()
        frame = null
        mesh = null
    }

    @Suppress("UNCHECKED_CAST")
    fun receive(b: ByteArray) {
        val gen = generation
        val epoch = assets.epoch()
        queue.execute {
            val o = runCatching { Cbor.decode(b) as? Map<String, Any?> }.getOrNull() ?: return@execute
            val key = o["key"] as? String ?: ""
            val fam = (o["fam"] as? String)?.takeIf { it.isNotEmpty() } ?: key
            // a 3D model: with its parts (2) or the board alone (3)
            val solid = key.startsWith("2|") || key.startsWith("3|") || key.startsWith("4|")
            val why = o["none"] as? String
            if (why != null) {
                val ticket = if (solid) assets.issue(key, epoch) ?: return@execute else 0L
                main.post {
                    if (gen != generation) return@post
                    if (solid) {
                        if (!assets.current(key, ticket)) return@post
                        val previous = assets.frame(key)?.mesh
                        mesh = MeshFrame(key, previous, why + if (previous == null) "" else " — previous saved revision shown", System.currentTimeMillis()); return@post
                    }
                    held = emptyList()
                    frame = PlotFrame(key, FloatArray(0), FloatArray(0), 0f, emptyList(), emptySet(), why, System.currentTimeMillis(), fam)
                }
                return@execute
            }
            if (solid) {
                // a note about the model (parts with no 3D model) is not a model
                if (o["mesh"] == null) return@execute
                val ticket = assets.issue(key, epoch) ?: return@execute
                val previous = assets.frame(key)?.mesh
                main.post {
                    if (gen == generation && assets.current(key, ticket))
                        mesh = MeshFrame(key, previous, "Received model; decoding…" + if (previous == null) "" else " — previous saved revision shown", System.currentTimeMillis())
                }
                meshQueue.execute meshJob@ {
                    if (!assets.current(key, ticket)) return@meshJob
                    val bytes = o["mesh"] as? ByteArray ?: ByteArray(0)
                    val n = o.int("n")
                    val result = runCatching { assets.same(key, n, bytes) ?: PlotMesh.decode(o) { !assets.current(key, ticket) } }
                    if (!assets.current(key, ticket)) return@meshJob
                    val m = result.getOrNull()
                    if (m != null) assets.put(key, n, bytes, m, ticket)
                    main.post {
                        if (gen != generation || !assets.current(key, ticket)) return@post
                        mesh = MeshFrame(key, m ?: previous,
                            if (m != null) "" else "Unable to decode model" + if (previous == null) "" else " — previous saved revision shown", System.currentTimeMillis())
                    }
                }
                return@execute
            }
            val refs = o["cs"] as? List<Any?> ?: emptyList()
            val jobs = refs.mapIndexedNotNull { i, r -> (r as? Map<String, Any?>)?.let { obj -> i to Callable { PlotChunk.decode(obj) } } }
            val done = pool.invokeAll(jobs.map { it.second })
            val decoded = HashMap<Int, PlotChunk>()
            jobs.forEachIndexed { k, (i, _) -> decoded[i] = done[k].get() }
            val box = (o["box"] as? List<*>)?.map { ((it as? Long) ?: 0L).toFloat() }?.toFloatArray() ?: FloatArray(0)
            val edge = (o["edge"] as? List<*>)?.map { ((it as? Long) ?: 0L).toFloat() }?.toFloatArray() ?: FloatArray(0)
            val thick = ((o["thick"] as? Long) ?: 1600L).toFloat()
            // the main thread keeps arrival order: each plot applies to the one before
            main.post {
                if (gen != generation) return@post
                val all = ArrayList<PlotChunk>(refs.size)
                for (i in refs.indices) {
                    val c = decoded[i] ?: (refs[i] as? Long)?.let { k -> held.getOrNull(k.toInt()) }
                    if (c != null) all.add(c)
                }
                held = all
                frame = PlotFrame(key, box, edge, thick, all, decoded.keys, "", System.currentTimeMillis(), fam)
            }
        }
    }

    companion object {
        // a plot frame: a map whose first key is 1 ("t") and value "plot"
        fun isPlot(b: ByteArray) = b.size > 7 && (b[0].toInt() and 255) in 0xA0..0xB7 && b[1].toInt() == 1 &&
            b[2].toInt() == 0x64 && b[3].toInt() == 0x70 && b[4].toInt() == 0x6C && b[5].toInt() == 0x6F && b[6].toInt() == 0x74
    }
}
