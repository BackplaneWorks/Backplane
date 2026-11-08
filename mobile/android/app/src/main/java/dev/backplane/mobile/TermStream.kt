package dev.backplane.mobile

// A terminal's output as a stream with offsets (docs/PHONE-DATA.md, Terminals; the hub's term/stream.rs). A run of a terminal has a name `g` and counts the
// bytes it printed, `o`. The phone holds a screen up to an offset of one run: a live frame says where its bytes start, and the phone takes from it only
// what it does not hold yet (`merge`), asking again (`Gap`) when a frame starts past what it holds. Pure: no Android, no socket (TermStreamTest).

/** What a phone does with a live frame of the run it holds. */
sealed interface TMerge {
    /** it holds all of it already */
    object Skip : TMerge
    /** feed the frame's bytes from this index on; it then holds up to `o + len` */
    data class Take(val at: Int) : TMerge
    /** bytes between are missing: `term.read {from: held}` */
    object Gap : TMerge
}

object TermStream {
    /** stream::merge: it holds up to `held`; the frame's bytes start at `o` and are `len` long. */
    fun merge(held: Long, o: Long, len: Int): TMerge {
        val end = o + len
        return if (o > held) TMerge.Gap else if (end <= held) TMerge.Skip else TMerge.Take((held - o).toInt())
    }
}

/** What a frame asks the phone to do next. */
sealed interface TNext {
    object Nothing : TNext
    /** `term.read {key, g, from}`: a gap (from = the offset held). */
    data class Resume(val g: Long, val from: Long) : TNext
    /** `term.read {key}`: another run, or no run named: read it again. */
    object Reread : TNext
}

/** One reply of term.read / term.watch, or one push, as plain values. `reset` is the (cols, rows) of a snapshot. */
class TFrame(val g: Long?, val o: Long?, val b: ByteArray, val from: Long?, val reset: Pair<Int, Int>?, val state: String?, val title: String?, val running: Boolean?)

/** The screen a phone holds for one terminal: the emulator, the run and offset it is at, and the state the hub last said. */
class TermScreen(cols: Int = 80, rows: Int = 24) {
    var vt = Vt(cols, rows); private set
    /** the run held (0: none yet) and the bytes of it held (-1: none) */
    var g = 0L; private set
    var held = -1L; private set
    var state = ""; private set
    var running = false; private set
    var title = ""; private set
    /** bumped whenever the screen changed: a view redraws on it */
    var rev = 0; private set

    /** The answer of `term.read` / `term.watch`: with `from` echoed the bytes come after what is held, otherwise a snapshot on a blank terminal. */
    fun read(f: TFrame) {
        meta(f)
        val gg = f.g ?: return
        if (f.reset != null) { vt = Vt(f.reset.first, f.reset.second); vt.feed(f.b); g = gg; held = f.o ?: 0 }
        else if (f.from != null && gg == g && held >= 0) {
            // what came after `from`: skip what is held already (the same rule as a live frame)
            val o = f.from
            when (val m = TermStream.merge(held, o, f.b.size)) { is TMerge.Take -> { vt.feed(f.b.copyOfRange(m.at, f.b.size)); held = o + f.b.size }; else -> {} }
        }
        rev++
    }

    private fun meta(f: TFrame) {
        f.state?.let { state = it; running = it == "running" || it.startsWith("done ") }
        f.running?.let { running = it }
        f.title?.let { if (it.isNotEmpty()) title = it }
    }

    /** A live push `{b, g?, o?, reset?, state?}`. */
    fun push(f: TFrame): TNext {
        meta(f)
        if (f.reset != null) { // a snapshot at `o`: replace the screen
            vt = Vt(f.reset.first, f.reset.second); vt.feed(f.b); f.g?.let { g = it }; held = f.o ?: held; rev++; return TNext.Nothing
        }
        if (f.b.isEmpty()) { rev++; return TNext.Nothing } // a state only
        val gg = f.g; val o = f.o
        if (gg == null || o == null || gg != g || held < 0) return TNext.Reread
        return when (val m = TermStream.merge(held, o, f.b.size)) {
            TMerge.Skip -> TNext.Nothing
            is TMerge.Take -> { vt.feed(if (m.at == 0) f.b else f.b.copyOfRange(m.at, f.b.size)); held = o + f.b.size; rev++; TNext.Nothing }
            TMerge.Gap -> TNext.Resume(g, held)
        }
    }

    /** The phone cuts the screen to a size locally (its own full-screen view): the hub then sends a snapshot at the new size. */
    fun resized(cols: Int, rows: Int) { if (cols != vt.cols() || rows != vt.rows()) { vt.resize(cols, rows); rev++ } }

    /** The state words: running, done N, exited N, stopped. */
    val stateLine get() = if (state.isEmpty()) (if (running) "running" else "") else state
}
