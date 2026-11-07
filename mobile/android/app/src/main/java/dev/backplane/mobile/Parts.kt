package dev.backplane.mobile

import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.util.concurrent.Executors

// The parts of a board's 3D model (PHONE-DATA.md): after the bare board, one
// frame a part model with all its instances, placed by `at`. Each is a
// group; the renderer fades a group in over FADE_MS from its arrival.

/** One part frame's instances as triangles in the board model's axes (as PlotMesh), and how many instances it brought. */
class PartGroup(mesh: PlotMesh) {
    /** pos, normal, colour a vertex, ready for the GL buffer (built on the decoding thread) */
    val packed: FloatArray = packSolid(mesh.verts, mesh.colors)
    val count = mesh.colors.size
}

fun packSolid(verts: FloatArray, colors: IntArray): FloatArray {
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

/** The parts held for a model's key. `gen` changes when they start again; within one `gen` `groups` only grows. */
class PartsFrame(val key: String, val gen: Int, val of: Int, val done: Int, val groups: List<PartGroup>) {
    val label get() = PlotMesh.progress(done, of)
}

class PartsStore {
    var parts by mutableStateOf<PartsFrame?>(null)
        private set
    private val main = Handler(Looper.getMainLooper())
    // one thread, in arrival order, off the main and GL threads
    private val queue = Executors.newSingleThreadExecutor { r -> Thread(r, "parts").also { it.priority = Thread.MIN_PRIORITY } }
    private var epoch = 0   // main thread: bumps on every reset, so a decode of the old parts is dropped
    private var gen = 0

    /** A new connection, a view switch, or a new board mesh: nothing held. Main thread. */
    fun reset() {
        epoch++
        parts = null
    }

    /** `{t:"plot", key, parts:{of, rev}}` or `{key, part, n, mesh, at, refs, done}`, decoded; any thread. */
    fun receive(key: String, o: Map<String, Any?>) {
        val e = epoch
        queue.execute {
            val start = o["parts"] as? Map<*, *>
            if (start != null) {
                val of = ((start["of"] as? Long) ?: 0L).toInt()
                main.post { if (e == epoch) { gen++; parts = PartsFrame(key, gen, of, 0, emptyList()) } }
                return@execute
            }
            val n = ((o["n"] as? Long) ?: 0L).toInt()
            val g = runCatching {
                PartGroup(PlotMesh.decodePart(n, o["mesh"] as? ByteArray ?: ByteArray(0), o["at"] as? ByteArray ?: ByteArray(0)) { e != epoch })
            }.getOrNull()
            val done = ((o["done"] as? Long) ?: 0L).toInt()
            main.post {
                if (e != epoch) return@post
                val p = parts?.takeIf { it.key == key } ?: PartsFrame(key, ++gen, done, 0, emptyList())
                parts = PartsFrame(key, p.gen, p.of, maxOf(done, p.done), if (g != null) p.groups + g else p.groups)
            }
        }
    }
}
