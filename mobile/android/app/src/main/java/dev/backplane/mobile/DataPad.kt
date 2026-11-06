package dev.backplane.mobile

import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

// The drawing pad's model: pure, no view. The desktop's native/gpui/src/screens/pad.rs, gesture for gesture, and iOS's DataPad.swift:
// boxes dragged out, moved, resized and given typed lengths, each new one taking the next colour; a pen and an eraser.
// Coordinates are the paper's own pixels (PAD_W x PAD_H); the screen scales them. A finger is wider than a mouse, so the view sets `edge` and
// `eraser` from its scale (a side is about a fingertip wide on any phone).

const val PAD_W = 960f
const val PAD_H = 600f
const val PAD_UNIT = 4f // paper pixels per unit of a typed length
val PAD_COLORS = intArrayOf(0xe5484d, 0xf5a524, 0x3e9b4f, 0x2f80ed, 0x8e4ec6, 0x12a594, 0x1f2328)
private const val MIN_SIZE = 6f // a smaller drag is a tap, not a box
private const val STEP = 3f // the largest gap between a stroke's points, so the eraser can cut it anywhere
private const val UNDO_MAX = 100

enum class PadTool { Pen, Boxes, Eraser }
/** Which part of an item a point is on: a side, or its middle. */
enum class PadPart { L, R, T, B, In }
data class PadPt(val x: Float, val y: Float)

sealed class PadItem {
    data class Box(val x: Float, val y: Float, val w: Float, val h: Float, val color: Int, val dimW: String = "", val dimH: String = "") : PadItem()
    data class Stroke(val pts: List<PadPt>, val color: Int) : PadItem()
}
/** A typed length: 40 and "mm". */
data class PadLength(val n: Float, val unit: String)

object PadMath {
    /** Which part of `r` a point is on, if any. */
    fun hit(r: PadItem.Box, x: Float, y: Float, edge: Float): PadPart? {
        if (x < r.x - edge || x > r.x + r.w + edge || y < r.y - edge || y > r.y + r.h + edge) return null
        val inX = x >= r.x && x <= r.x + r.w; val inY = y >= r.y && y <= r.y + r.h
        val best = listOf(Triple(PadPart.L, inY, abs(x - r.x)), Triple(PadPart.R, inY, abs(x - r.x - r.w)), Triple(PadPart.T, inX, abs(y - r.y)), Triple(PadPart.B, inX, abs(y - r.y - r.h)))
            .filter { it.second }.minByOrNull { it.third }
        if (best != null && best.third <= edge) return best.first
        return if (x > r.x && x < r.x + r.w && y > r.y && y < r.y + r.h) PadPart.In else null
    }
    private fun onStroke(s: PadItem.Stroke, x: Float, y: Float, edge: Float) = s.pts.any { abs(it.x - x) <= edge && abs(it.y - y) <= edge }
    /** The topmost item under a point, and the part of it. */
    fun pick(items: List<PadItem>, x: Float, y: Float, edge: Float): Pair<Int, PadPart>? {
        for (i in items.indices.reversed()) when (val it = items[i]) {
            is PadItem.Box -> hit(it, x, y, edge)?.let { p -> return i to p }
            is PadItem.Stroke -> if (onStroke(it, x, y, edge)) return i to PadPart.In
        }
        return null
    }
    /** "40", "40 mm", "12.5cm"; anything else is none. */
    fun parseLength(text: String): PadLength? {
        val t = text.trim()
        val end = t.indexOfFirst { !it.isDigit() && it != '.' }.let { if (it < 0) t.length else it }
        val num = t.substring(0, end); val unit = t.substring(end).trimStart()
        if (num.isEmpty() || num.startsWith('.') || num.endsWith('.') || unit.length > 4 || !unit.all { (it in 'a'..'z') || (it in 'A'..'Z') || it == 'µ' || it == '"' || it == '\'' }) return null
        val n = num.toFloatOrNull() ?: return null
        return if (n > 0f) PadLength(n, unit) else null
    }
    private fun label(n: Float) = if (n == n.toInt().toFloat()) n.toInt().toString() else n.toString()
    /** A box with one side's length set: the side's axis takes n units, the opposite side stays, and the label is kept for the picture. */
    fun withLength(r: PadItem.Box, side: PadPart, len: PadLength): PadItem.Box {
        val px = max((len.n * PAD_UNIT).roundToInt().toFloat(), MIN_SIZE)
        val l = if (len.unit.isEmpty()) label(len.n) else "${label(len.n)} ${len.unit}"
        return when (side) {
            PadPart.L -> r.copy(x = r.x + r.w - px, w = px, dimW = l)
            PadPart.R -> r.copy(w = px, dimW = l)
            PadPart.T -> r.copy(y = r.y + r.h - px, h = px, dimH = l)
            PadPart.B -> r.copy(h = px, dimH = l)
            PadPart.In -> r
        }
    }
    /** A side dragged to (x, y): the box with that side there, never flipped inside out; a typed length on that axis no longer holds. */
    fun resize(r: PadItem.Box, side: PadPart, x: Float, y: Float): PadItem.Box = when (side) {
        PadPart.L -> { val right = r.x + r.w; val nx = min(x, right - MIN_SIZE); r.copy(x = nx, w = right - nx, dimW = "") }
        PadPart.R -> r.copy(w = max(x - r.x, MIN_SIZE), dimW = "")
        PadPart.T -> { val bottom = r.y + r.h; val ny = min(y, bottom - MIN_SIZE); r.copy(y = ny, h = bottom - ny, dimH = "") }
        PadPart.B -> r.copy(h = max(y - r.y, MIN_SIZE), dimH = "")
        PadPart.In -> r
    }
    fun bounds(it: PadItem): FloatArray = when (it) {
        is PadItem.Box -> floatArrayOf(it.x, it.y, it.w, it.h)
        is PadItem.Stroke -> { val a = it.pts.minOf { p -> p.x }; val b = it.pts.minOf { p -> p.y }; floatArrayOf(a, b, it.pts.maxOf { p -> p.x } - a, it.pts.maxOf { p -> p.y } - b) }
    }
    /** The points from `a` to `b`, `b` included, no further apart than STEP. */
    fun walk(a: PadPt, b: PadPt): List<PadPt> {
        val n = max(ceil(hypot(b.x - a.x, b.y - a.y) / STEP).toInt(), 1)
        return (1..n).map { val t = it.toFloat() / n; PadPt(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t) }
    }
    /** What is left after the eraser touched (x, y): a box it touches goes, a stroke is cut around it. Null when nothing changed. */
    fun erased(items: List<PadItem>, x: Float, y: Float, radius: Float): List<PadItem>? {
        val touched = { p: PadPt -> hypot(p.x - x, p.y - y) <= radius }
        val out = ArrayList<PadItem>(items.size); var changed = false
        for (it in items) when {
            it is PadItem.Box && x >= it.x - radius && x <= it.x + it.w + radius && y >= it.y - radius && y <= it.y + it.h + radius -> changed = true
            it is PadItem.Stroke && it.pts.any(touched) -> {
                changed = true
                // what stays is the runs of points the eraser missed; a lone point is a crumb, not a stroke
                var run = ArrayList<PadPt>()
                for (p in it.pts + PadPt(Float.NaN, Float.NaN)) {
                    if (p.x.isNaN() || touched(p)) { if (run.size > 1) out.add(PadItem.Stroke(run, it.color)); run = ArrayList() } else run.add(p)
                }
            }
            else -> out.add(it)
        }
        return if (changed) out else null
    }
}

private sealed class PadDrag {
    data class New(val x0: Float, val y0: Float) : PadDrag()
    object Pen : PadDrag()
    data class Erase(var kept: Boolean, var at: PadPt) : PadDrag()
    data class Item(val i: Int, val part: PadPart, val x0: Float, val y0: Float, val from: PadItem, var moved: Boolean) : PadDrag()
}

class DataPad {
    var items: List<PadItem> = emptyList(); private set
    var tool = PadTool.Pen; private set
    /** The colour the next stroke or box takes, an index into PAD_COLORS that may run past its end. */
    var next = 0; private set
    /** A colour was chosen by hand: new boxes keep it instead of taking the next. */
    private var pinned = false
    var picked: Int? = null; private set
    /** The side being given a length. */
    var editing: Pair<Int, PadPart>? = null; private set
    /** The box being dragged out, and the stroke being drawn, for the screen only. */
    var draft: PadItem.Box? = null; private set
    var ink: PadItem.Stroke? = null; private set
    var pointer: PadPt? = null; private set
    /** How near a side counts as on it, and the eraser's radius, in paper pixels. */
    var edge = 7f
    var eraser = 12f
    private val undoStack = ArrayList<Pair<List<PadItem>, Int>>()
    private var drag: PadDrag? = null

    val color get() = PAD_COLORS[next % PAD_COLORS.size]
    val isEmpty get() = items.isEmpty()
    val dragging get() = drag != null
    val canUndo get() = undoStack.isNotEmpty()
    private fun keep() { undoStack.add(items to next); if (undoStack.size > UNDO_MAX) undoStack.removeAt(0) }
    private fun clamp(x: Float, y: Float) = PadPt(x.coerceIn(0f, PAD_W), y.coerceIn(0f, PAD_H))
    fun setTool(t: PadTool) { tool = t; picked = null; pointer = null }
    fun setColor(i: Int) { next = i % PAD_COLORS.size; pinned = true }

    /** A finger went down on the paper at (x, y). */
    fun down(x: Float, y: Float) {
        val p = clamp(x, y)
        picked = null
        drag = when (tool) {
            PadTool.Pen -> { ink = PadItem.Stroke(listOf(p), color); PadDrag.Pen }
            PadTool.Eraser -> { pointer = p; PadDrag.Erase(false, p).also { eraseTo(it, p) } }
            PadTool.Boxes -> PadMath.pick(items, p.x, p.y, edge)?.let { (i, part) -> picked = i; PadDrag.Item(i, part, p.x, p.y, items[i], false) } ?: PadDrag.New(p.x, p.y)
        }
    }
    private fun eraseTo(d: PadDrag.Erase, to: PadPt) {
        for (p in listOf(d.at) + PadMath.walk(d.at, to)) {
            val left = PadMath.erased(items, p.x, p.y, eraser) ?: continue
            if (!d.kept) { d.kept = true; keep() } // one sweep is one undo
            items = left
        }
        d.at = to
    }
    /** The finger moved to (x, y), on the paper or off it. */
    fun moved(x: Float, y: Float) {
        val c = clamp(x, y)
        when (val d = drag ?: return) {
            PadDrag.Pen -> ink?.let { s -> val last = s.pts.last(); if (last != c) ink = s.copy(pts = s.pts + PadMath.walk(last, c)) }
            is PadDrag.Erase -> { pointer = c; eraseTo(d, c) }
            is PadDrag.New -> if (hypot(c.x - d.x0, c.y - d.y0) >= 3f || draft != null) draft = PadItem.Box(min(d.x0, c.x), min(d.y0, c.y), abs(c.x - d.x0), abs(c.y - d.y0), color)
            is PadDrag.Item -> if (d.moved || hypot(x - d.x0, y - d.y0) >= 3f) {
                if (!d.moved) { d.moved = true; keep() }
                val f = d.from
                items = items.toMutableList().also { it[d.i] = when {
                    f is PadItem.Box && d.part == PadPart.In -> f.copy(x = (f.x + x - d.x0).coerceIn(0f, PAD_W - f.w).roundToInt().toFloat(), y = (f.y + y - d.y0).coerceIn(0f, PAD_H - f.h).roundToInt().toFloat())
                    f is PadItem.Box -> PadMath.resize(f, d.part, c.x.roundToInt().toFloat(), c.y.roundToInt().toFloat())
                    else -> { val s = f as PadItem.Stroke; val b = PadMath.bounds(f)
                        val dx = (x - d.x0).coerceIn(-b[0], PAD_W - b[2] - b[0]); val dy = (y - d.y0).coerceIn(-b[1], PAD_H - b[3] - b[1])
                        PadItem.Stroke(s.pts.map { p -> PadPt(p.x + dx, p.y + dy) }, s.color) }
                } }
            }
        }
    }
    /** The finger came up. True when it was a tap on a box's side (no drag): the view asks for its length, and `editing` says which side. */
    fun up(): Boolean {
        val d = drag ?: return false
        drag = null; pointer = null
        return when (d) {
            PadDrag.Pen -> { ink?.let { keep(); items = items + it }; ink = null; false }
            is PadDrag.Erase -> false
            is PadDrag.New -> {
                draft?.takeIf { it.w >= MIN_SIZE && it.h >= MIN_SIZE }?.let { r ->
                    keep()
                    items = items + r.copy(x = r.x.roundToInt().toFloat(), y = r.y.roundToInt().toFloat(), w = r.w.roundToInt().toFloat(), h = r.h.roundToInt().toFloat())
                    picked = items.size - 1
                    if (!pinned) next += 1
                }
                draft = null; false
            }
            is PadDrag.Item -> { val side = !d.moved && d.part != PadPart.In && d.from is PadItem.Box; if (side) editing = d.i to d.part; side }
        }
    }
    /** The touch was taken away (a system gesture): what was being drawn is dropped. */
    fun cancel() { drag = null; draft = null; ink = null; pointer = null }
    /** What the length box starts with: the side's typed length, else its length in units. */
    fun lengthText(): String {
        val (i, side) = editing ?: return ""
        val r = items.getOrNull(i) as? PadItem.Box ?: return ""
        val horizontal = side == PadPart.L || side == PadPart.R
        val label = if (horizontal) r.dimW else r.dimH
        return label.ifEmpty { ((if (horizontal) r.w else r.h) / PAD_UNIT).roundToInt().toString() }
    }
    /** The length box closed with `text`; text that is no length changes nothing. */
    fun commitLength(text: String) {
        val (i, side) = editing ?: return
        editing = null
        val len = PadMath.parseLength(text) ?: return
        val r = items.getOrNull(i) as? PadItem.Box ?: return
        keep(); items = items.toMutableList().also { it[i] = PadMath.withLength(r, side, len) }
    }
    fun cancelLength() { editing = null }
    /** Take back the last change. */
    fun undo() {
        if (dragging) return
        val (was, n) = undoStack.removeLastOrNull() ?: return
        items = was; next = n; picked = null
    }
    /** Delete the selection; with none, clear the paper. Either is one undo. */
    fun delete() {
        if (dragging || items.isEmpty()) return
        keep()
        val p = picked; picked = null
        items = if (p != null && p < items.size) items.toMutableList().also { it.removeAt(p) } else emptyList()
    }
}
