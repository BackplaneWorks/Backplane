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

// Plots from the hub as GPU-ready geometry. Nothing
// is decided here: which chunks, their order, colour and opacity come from
// the hub, which piece a tap picks from , the fade and zoom range from
// the screen. A plot lists each chunk as the index of one this app holds
// (the last plot applied) or the chunk itself; applying it is Plot.apply
// (law plot_delta_exact), so `held` always mirrors what the hub thinks
// this connection holds, index for index (a tap names a chunk by it).

// CBOR, as much as plots use:
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
        /** A part frame's model placed at each instance: `at` is 12 little-endian f32 an instance, a row-major 3x4 affine from the
         *  mesh's integer coordinates to the board model's, applied to the integers before the 10 µm scale (as the board's mesh goes). */
        fun decodePart(n: Int, bytes: ByteArray, at: ByteArray, cancelled: () -> Boolean = { false }): PlotMesh {
            require(n > 0 && n <= (64 shl 20) / 84 && bytes.size >= n * 10) { "Invalid or oversized part" }
            require(at.isNotEmpty() && at.size % 48 == 0) { "Invalid part placement" }
            val k = at.size / 48
            require(n.toLong() * k <= (64 shl 20) / 84) { "Too many part triangles" }
            val v = Varints(bytes)
            val raw = IntArray(n * 9)
            val colors = IntArray(n)
            var color = 0
            val p = IntArray(3)
            for (t in 0 until n) {
                color = color xor v.next()
                for (c in 0 until 3) for (a in 0 until 3) { p[a] += v.signed(); raw[t * 9 + 3 * c + a] = p[a] }
                require(v.valid) { "Incomplete part" }
                colors[t] = color
            }
            require(!v.more && v.valid) { "Unexpected part bytes" }
            val verts = FloatArray(n * k * 18)
            val cols = IntArray(n * k * 3)
            val bb = java.nio.ByteBuffer.wrap(at).order(java.nio.ByteOrder.LITTLE_ENDIAN)
            val a = FloatArray(12)
            var w = 0
            for (inst in 0 until k) {
                if (cancelled()) throw java.util.concurrent.CancellationException()
                for (i in 0 until 12) { a[i] = bb.getFloat(inst * 48 + i * 4); require(a[i].isFinite()) { "Invalid part placement" } }
                val q = FloatArray(9)
                for (t in 0 until n) {
                    for (c in 0 until 3) {
                        val x = raw[t * 9 + 3 * c].toFloat(); val y = raw[t * 9 + 3 * c + 1].toFloat(); val z = raw[t * 9 + 3 * c + 2].toFloat()
                        q[3 * c] = (a[0] * x + a[1] * y + a[2] * z + a[3]) * 10f
                        q[3 * c + 1] = (a[4] * x + a[5] * y + a[6] * z + a[7]) * 10f
                        q[3 * c + 2] = (a[8] * x + a[9] * y + a[10] * z + a[11]) * 10f
                    }
                    val ux = q[3] - q[0]; val uy = q[4] - q[1]; val uz = q[5] - q[2]
                    val vx = q[6] - q[0]; val vy = q[7] - q[1]; val vz = q[8] - q[2]
                    var nx = uy * vz - uz * vy; var ny = uz * vx - ux * vz; var nz = ux * vy - uy * vx
                    val l = sqrt(nx * nx + ny * ny + nz * nz)
                    if (l > 0) { nx /= l; ny /= l; nz /= l } else { nx = 0f; ny = 0f; nz = 1f }
                    for (c in 0 until 3) {
                        verts[w * 6] = q[3 * c]; verts[w * 6 + 1] = q[3 * c + 1]; verts[w * 6 + 2] = q[3 * c + 2]
                        verts[w * 6 + 3] = nx; verts[w * 6 + 4] = ny; verts[w * 6 + 5] = nz
                        cols[w] = colors[t]
                        w++
                    }
                }
            }
            return PlotMesh(verts, cols, FloatArray(0))
        }

        /** A part group's opacity `ms` after it showed (the desktop's fade_at). */
        const val FADE_MS = 300f
        fun fadeAt(ms: Float) = (ms / FADE_MS).coerceIn(0f, 1f)
        /** "parts n/N" while parts stream, "" when there are none to wait for. */
        fun progress(done: Int, of: Int) = if (of <= 0 || done >= of) "" else "parts $done/$of"

        /** A model's id: FNV-1a, 64 bits, of its wire bytes, as 16 hex digits (the hub's plot::mesh_id); `viewer.watch {kept}` names the cached model with it. */
        fun id(d: ByteArray): String {
            var h = -0x340d631b7bdddcdbL // 0xcbf29ce484222325
            for (b in d) { h = (h xor (b.toLong() and 255)) * 0x100000001b3L }
            return java.lang.Long.toUnsignedString(h, 16).padStart(16, '0')
        }
    }
}

/** The last model each view showed (hub, thread, kind, path), as the hub sent it, in the app's cache folder: shown at once when the view opens
 *  again, on another launch or offline, and not downloaded again while the hub's model is the same. The newest few are kept. */
object MeshDisk {
    private const val MOST = 8
    var dir: java.io.File? = null
    private fun file(slot: String) = dir?.let { java.io.File(it, PlotMesh.id(slot.toByteArray()) + ".bin") }
    /** {key, n, box3, mesh}: the frame's own fields, so PlotMesh.decode reads it as it reads the wire */
    fun load(slot: String): Map<String, Any?>? = runCatching {
        val f = file(slot)?.takeIf { slot.isNotEmpty() && it.isFile } ?: return null
        java.io.DataInputStream(f.inputStream().buffered()).use { i ->
            val key = i.readUTF(); val n = i.readInt(); val box = List(i.readInt()) { i.readLong() }; val mesh = ByteArray(i.readInt()).also { i.readFully(it) }
            f.setLastModified(System.currentTimeMillis())
            mapOf("key" to key, "n" to n.toLong(), "box3" to box, "mesh" to mesh)
        }
    }.getOrNull()
    fun save(slot: String, key: String, n: Int, box3: List<Long>, mesh: ByteArray) {
        val f = file(slot)?.takeIf { slot.isNotEmpty() } ?: return
        runCatching {
            f.parentFile?.mkdirs()
            val tmp = java.io.File(f.path + ".tmp")
            java.io.DataOutputStream(tmp.outputStream().buffered()).use { o -> o.writeUTF(key); o.writeInt(n); o.writeInt(box3.size); box3.forEach { o.writeLong(it) }; o.writeInt(mesh.size); o.write(mesh) }
            tmp.renameTo(f)
            f.parentFile?.listFiles { x -> x.name.endsWith(".bin") }?.sortedByDescending { it.lastModified() }?.drop(MOST)?.forEach { it.delete() }
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

class MeshFrame(val key: String, val mesh: PlotMesh?, val none: String, val at: Long, val decoding: Boolean = false)

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
        serial++; tickets.clear(); tickets[key] = serial; return serial
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

/** Which plots reach the view on show, pure so it is tested (PlotStoreGateTest): after `begin` (a viewer switches) what comes waits
 *  until `shown` names the view's keys, then what waited for them is released and the rest dropped; a plot for a key the view does not
 *  show (late, for the view that was left) is dropped. `same`: a newer parked item replaces an older one of the key. */
class ViewGate<T>(private val same: (T, T) -> Boolean = { _, _ -> true }) {
    var accept: Set<String>? = emptySet(); private set
    private val parked = ArrayList<Pair<String, T>>()
    fun begin() { accept = null; parked.clear() }
    fun stop() { accept = emptySet(); parked.clear() }
    /** true: show it; false: drop it; null: park it (the view's keys are not known yet) */
    fun wanted(key: String): Boolean? = accept?.contains(key)
    fun park(key: String, item: T) { parked.removeAll { it.first == key && same(it.second, item) }; parked.add(key to item) }
    fun shown(keys: Collection<String>): List<T> { val a = keys.toSet(); accept = a; val out = parked.filter { it.first in a }.map { it.second }; parked.clear(); return out }
}

class PlotStore {
    var frame by mutableStateOf<PlotFrame?>(null)
        private set
    var mesh by mutableStateOf<MeshFrame?>(null)
        private set
    private var held: List<PlotChunk> = emptyList()
    val parts = PartsStore()
    /** A 3D model's frames other than the model (`parts`, `part`: kicad's native board mesher streams the parts after the board), already
     *  gated to the view on show (route), to the parts store. A `parts` frame may come alone when the phone's kept model was the board. */
    var more: ((String, Map<String, Any?>) -> Unit)? = { k, o -> parts.receive(k, o) }
    private val splitFrames = LinkedHashMap<String, PlotFrame>()
    private var generation = 0
    private val assets = PlotMeshAssets()
    private val meshQueue = Executors.newSingleThreadExecutor()
    fun cachedMesh(key: String): MeshFrame? = assets.frame(key)
    /** The view the models now arriving are for (MeshDisk's slot), set by `cached` */
    @Volatile private var slot = ""

    // The view on show: the hub's keys of the plots it shows (`viewer.watch` answers them), null while its watch is on its way.
    // A plot for another key came late for a view that was left and is dropped; one that comes before the answer waits for it. Main thread.
    private class Parked(val o: Map<String, Any?>?, val f: PlotFrame?)
    // a newer plot or model replaces the parked one of its key; a model's parts all wait, in order
    private val gate = ViewGate<Parked> { a, b -> (a.f != null && b.f != null) || (a.o?.get("mesh") != null && b.o?.get("mesh") != null) }
    /** This view's cached model (MeshDisk), drawn once the hub names its key among the view's */
    private var disk: Map<String, Any?>? = null
    private fun wanted(key: String): Boolean? = gate.wanted(key)
    private fun park(key: String, o: Map<String, Any?>?, f: PlotFrame?) { gate.park(key, Parked(o, f)) }

    /** A viewer switches (a tab, a file, a thread): what the last one showed goes at once, plots for it are dropped from now on, and the
     *  view's own cached model (`slot`: hub, thread, kind, path) waits to be named. Returns `viewer.watch {kept}` ("" for no cached model). */
    fun begin(slot: String): String {
        this.slot = slot
        gate.begin(); frame = null; mesh = null; parts.reset() // the last view's parts go with its model
        assets.issue("", assets.epoch()) // a model still decoding for the last view is not shown
        disk = if (slot.isEmpty()) null else MeshDisk.load(slot)
        val o = disk ?: return ""
        return (o["key"] as String) + "\t" + PlotMesh.id(o["mesh"] as ByteArray)
    }
    /** The hub named the view's plots: what waited for them is shown, the rest dropped; the cached model shows if it is of this view's file. */
    fun shown(keys: List<String>) {
        for (p in gate.shown(keys)) { if (p.f != null) { frame = p.f; keepFrame() } else if (p.o != null) route(p.o) }
        disk?.let { o -> val key = o["key"] as String; if (gate.wanted(key) == true && mesh?.key != key) showDisk(o, key) }
        disk = null
    }
    /** The watch failed (no hub): the view's own cached model is all there is to show. */
    fun failed() {
        gate.stop()
        disk?.let { o -> val key = o["key"] as String; gate.shown(listOf(key)); showDisk(o, key) }
        disk = null
    }
    private fun showDisk(o: Map<String, Any?>, key: String) {
        val bytes = o["mesh"] as ByteArray
        val ticket = assets.issue(key, assets.epoch()) ?: return
        val gen = generation
        meshQueue.execute {
            if (!assets.current(key, ticket)) return@execute
            val m = runCatching { PlotMesh.decode(o) { !assets.current(key, ticket) } }.getOrNull() ?: return@execute
            assets.put(key, o.int("n"), bytes, m, ticket)
            main.post { if (gen == generation && assets.current(key, ticket)) mesh = MeshFrame(key, m, "", System.currentTimeMillis()) }
        }
    }
    /** A plot as JSON (the data socket's own frames: `{t:"plot", key, none}`, why a view has nothing to show). Main thread. */
    fun receiveJSON(o: org.json.JSONObject) { if (o.has("none")) route(mapOf("key" to o.optString("key"), "none" to o.optString("none"), "fam" to o.optString("fam"))) }
    fun cachedFrame(key: String): PlotFrame? = frame?.takeIf { it.key == key } ?: splitFrames[key]
    private fun keepFrame() {
        val f = frame ?: return
        splitFrames.remove(f.key)
        splitFrames[f.key] = f
        while (splitFrames.size > 3) splitFrames.remove(splitFrames.keys.first())
    }
    private val main = Handler(Looper.getMainLooper())
    // one plot at a time, in arrival order; its new chunks decode side by side
    private val queue = Executors.newSingleThreadExecutor()
    private val pool = Executors.newFixedThreadPool(maxOf(2, Runtime.getRuntime().availableProcessors() - 1))

    // a new connection holds nothing
    fun reset() {
        generation += 1
        assets.reset()
        held = emptyList()
        parts.reset()
        splitFrames.clear()
        frame = null
        mesh = null
        gate.stop()
    }

    @Suppress("UNCHECKED_CAST")
    fun receive(b: ByteArray) {
        val gen = generation
        queue.execute {
            val o = runCatching { Cbor.decode(b) as? Map<String, Any?> }.getOrNull() ?: return@execute
            val key = o["key"] as? String ?: ""
            val fam = (o["fam"] as? String)?.takeIf { it.isNotEmpty() } ?: key
            // a 3D model: with its parts (2) or the board alone (3)
            val solid = key.startsWith("2|") || key.startsWith("3|") || key.startsWith("4|")
            // a note about the model (parts with no 3D model) is not a model
            if (o["none"] is String || solid) {
                // a model's other frames (its parts streaming in after it, `parts`/`part`) pass the same gate to `more`; a bare note does not
                if (solid && o["none"] == null && o["mesh"] == null && o["parts"] == null && o["part"] == null) return@execute
                main.post { if (gen == generation) route(o) }
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
                held = all // the hub's deltas count on what it sent, shown or not
                val f = PlotFrame(key, box, edge, thick, all, decoded.keys, "", System.currentTimeMillis(), fam)
                when (wanted(key)) { true -> { frame = f; keepFrame() }; null -> park(key, null, f); false -> {} }
            }
        }
    }

    /** A model or a plot's absence, on the main thread: for the view on show, waiting for it, or dropped */
    private fun route(o: Map<String, Any?>) {
        val key = o["key"] as? String ?: ""
        when (wanted(key)) { null -> { park(key, o, null); return }; false -> return; true -> {} }
        val fam = (o["fam"] as? String)?.takeIf { it.isNotEmpty() } ?: key
        val solid = key.startsWith("2|") || key.startsWith("3|") || key.startsWith("4|")
        if (solid && o["none"] == null && o["mesh"] == null) { more?.invoke(key, o); return } // parts after (or without) the model: for the view on show only
        val gen = generation; val slot = slot
        val ticket = if (solid) assets.issue(key, assets.epoch()) ?: return else 0L
        val previous = if (solid) assets.frame(key)?.mesh else null
        val why = o["none"] as? String
        if (why != null) {
            if (solid) { mesh = MeshFrame(key, previous, why + if (previous == null) "" else " — previous saved revision shown", System.currentTimeMillis()); return }
            held = emptyList()
            frame = PlotFrame(key, FloatArray(0), FloatArray(0), 0f, emptyList(), emptySet(), why, System.currentTimeMillis(), fam)
            keepFrame(); return
        }
        parts.reset() // a new board mesh: its parts come again after it
        mesh = MeshFrame(key, previous, "Received model; decoding…" + if (previous == null) "" else " — previous saved revision shown", System.currentTimeMillis(), decoding = true)
        meshQueue.execute meshJob@ {
            if (!assets.current(key, ticket)) return@meshJob
            val bytes = o["mesh"] as? ByteArray ?: ByteArray(0)
            val n = o.int("n")
            val result = runCatching { assets.same(key, n, bytes) ?: PlotMesh.decode(o) { !assets.current(key, ticket) } }
            if (!assets.current(key, ticket)) return@meshJob
            val m = result.getOrNull()
            if (m != null) { assets.put(key, n, bytes, m, ticket); MeshDisk.save(slot, key, n, (o["box3"] as? List<*>)?.map { (it as? Long) ?: 0L }.orEmpty(), bytes) }
            main.post {
                if (gen != generation || !assets.current(key, ticket)) return@post
                mesh = MeshFrame(key, m ?: previous,
                    if (m != null) "" else "Unable to decode model" + if (previous == null) "" else " — previous saved revision shown", System.currentTimeMillis())
            }
        }
    }

    companion object {
        /** The keys a view shows by the hub's rule (kicad/watch.rs `key`), for a hub that does not answer them. */
        fun keys(kind: String, root: String, path: String): List<String> {
            val p = if (path.isEmpty() || path.startsWith("/")) path else "$root/$path"
            fun k(n: Int, q: String) = "$n|$root|$q"
            return when (kind) {
                "board" -> listOf(k(0, p)); "schematic" -> listOf(k(1, p))
                "3d" -> listOf(k(2, p), k(0, p)); "3d-bare" -> listOf(k(3, p), k(0, p)); "3d-full" -> listOf(k(4, p), k(0, p))
                "split" -> listOf(k(1, p), k(0, ""), k(2, "")); else -> emptyList()
            }
        }
        // a plot frame: a map whose first key is 1 ("t") and value "plot"
        fun isPlot(b: ByteArray) = b.size > 7 && (b[0].toInt() and 255) in 0xA0..0xB7 && b[1].toInt() == 1 &&
            b[2].toInt() == 0x64 && b[3].toInt() == 0x70 && b[4].toInt() == 0x6C && b[5].toInt() == 0x6F && b[6].toInt() == 0x74
    }
}
