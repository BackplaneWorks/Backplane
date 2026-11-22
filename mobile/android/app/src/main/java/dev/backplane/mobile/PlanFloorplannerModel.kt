package dev.backplane.mobile

import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sqrt
import org.json.JSONArray
import org.json.JSONObject

data class PlanCopperLayer(
    val layer: String,
    val index: Int = 0,
    val role: String = "",
    val title: String = "",
    val subtitle: String = "",
)

data class PlanItem(
    val id: String,
    val kind: String = "corridor",
    val name: String = "",
    val color: String = "",
    val state: String = "proposed",
    val nets: Int = 0,
    val routed: Int = 0,
    val total: Int = 0,
    val width: Double? = null,
    val impedance: String = "",
    val reference: String = "",
    val locked: Boolean = false,
    val edited: Boolean = false,
    val problems: Int = 0,
    val problemText: String = "",
    val layer: String = "",
)

data class PlanLayer(
    val layer: String,
    val index: Int = 0,
    val title: String = "",
    val subtitle: String = "",
    val role: String = "",
    val status: String = "",
    val done: Int = 0,
    val approved: Int = 0,
    val total: Int = 0,
    val stackup: String = "",
    val items: List<PlanItem> = emptyList(),
)

data class PlanPartBox(
    val r: String,
    val s: String = "",
    val b: List<Double> = emptyList(),
    val o: Boolean = false,
)

data class PlanBoardGeom(
    val bbox: List<Double> = listOf(0.0, 0.0, 100.0, 70.0),
    val outline: List<List<Pair<Double, Double>>> = emptyList(),
    val parts: List<PlanPartBox> = emptyList(),
)

data class PlanBand(
    val id: String,
    val layer: String,
    val name: String = "",
    val color: String = "",
    val state: String = "proposed",
    val width: Double = 1.5,
    val nets: Int = 0,
    val locked: Boolean = false,
    val edited: Boolean = false,
    val path: List<Pair<Double, Double>> = emptyList(),
    val vias: List<Pair<Double, Double>> = emptyList(),
    val guides: List<List<Double>> = emptyList(),
    val from: List<String> = emptyList(),
    val to: List<String> = emptyList(),
)

data class PlanRegion(
    val id: String,
    val layer: String,
    val net: String = "",
    val color: String = "",
    val state: String = "proposed",
    val locked: Boolean = false,
    val edited: Boolean = false,
    val outline: List<Pair<Double, Double>> = emptyList(),
    val holes: List<List<Pair<Double, Double>>> = emptyList(),
)

data class PlanDraw(
    val bands: List<PlanBand> = emptyList(),
    val regions: List<PlanRegion> = emptyList(),
)

data class PlanAskNote(val what: String = "", val note: String = "")
data class PlanElsewhere(val name: String = "", val pcb: String = "", val rel: String = "")

data class PlanPanel(
    val empty: Boolean = false,
    val on: Boolean = false,
    val live: Boolean = false,
    val pro: String = "",
    val boardPath: String = "",
    val boardRel: String = "",
    val routing: Boolean = false,
    val awaiting: Int = 0,
    val approved: Int = 0,
    val done: Int = 0,
    val blocked: Int = 0,
    val total: Int = 0,
    val revision: Int = 0,
    val status: String = "",
    val boardTrimmed: Boolean = false,
    val stackupSummary: String = "",
    val impedances: List<String> = emptyList(),
    val keysNote: String = "",
    val asks: List<PlanAskNote> = emptyList(),
    val elsewhere: List<PlanElsewhere> = emptyList(),
    val layers: List<PlanLayer> = emptyList(),
    val copperLayers: List<PlanCopperLayer> = emptyList(),
    val board: PlanBoardGeom? = null,
    val draw: PlanDraw? = null,
)

data class PlanCamera(val scale: Double, val cx: Double, val cy: Double) {
    fun toPx(x: Double, y: Double, w: Double, h: Double): Pair<Double, Double> =
        (w * 0.5 + (x - cx) * scale) to (h * 0.5 + (y - cy) * scale)

    fun toWorld(px: Double, py: Double, w: Double, h: Double): Pair<Double, Double> =
        (cx + (px - w * 0.5) / max(scale, 0.01)) to (cy + (py - h * 0.5) / max(scale, 0.01))

    fun pan(dxPx: Double, dyPx: Double): PlanCamera =
        copy(cx = cx - dxPx / max(scale, 0.01), cy = cy - dyPx / max(scale, 0.01))

    fun zoom(factor: Double, ax: Double, ay: Double, w: Double, h: Double): PlanCamera {
        val next = (scale * factor).coerceIn(0.8, 64.0)
        val ratio = scale / next
        return PlanCamera(
            scale = next,
            cx = cx + (ax - w * 0.5) / max(scale, 0.01) * (1.0 - ratio),
            cy = cy + (ay - h * 0.5) / max(scale, 0.01) * (1.0 - ratio),
        )
    }

    companion object {
        fun fit(bbox: List<Double>, w: Double, h: Double, pad: Double = 22.0): PlanCamera {
            val minx = bbox.getOrElse(0) { 0.0 }
            val miny = bbox.getOrElse(1) { 0.0 }
            val maxx = bbox.getOrElse(2) { 100.0 }
            val maxy = bbox.getOrElse(3) { 70.0 }
            val spanX = max(1.0, maxx - minx)
            val spanY = max(1.0, maxy - miny)
            val aw = max(24.0, w - pad * 2.0)
            val ah = max(24.0, h - pad * 2.0)
            return PlanCamera(min(aw / spanX, ah / spanY), (minx + maxx) * 0.5, (miny + maxy) * 0.5)
        }
    }
}

sealed class PlanHit(open val id: String) {
    data class Vertex(override val id: String, val isPlane: Boolean, val idx: Int) : PlanHit(id)
    data class Midpoint(override val id: String, val isPlane: Boolean, val segIdx: Int) : PlanHit(id)
    data class Band(override val id: String) : PlanHit(id)
    data class Region(override val id: String) : PlanHit(id)
}

data class PlanBoxRect(val x0: Double, val y0: Double, val x1: Double, val y1: Double) {
    fun overlaps(o: PlanBoxRect): Boolean = !(x1 <= o.x0 || x0 >= o.x1 || y1 <= o.y0 || y0 >= o.y1)
    fun pad(p: Double): PlanBoxRect = PlanBoxRect(x0 - p, y0 - p, x1 + p, y1 + p)
}

data class PlanPlacedLabel(val text: String, val cx: Double, val cy: Double)
data class PlanCorridorPill(
    val id: String,
    val text: String,
    val cx: Double,
    val cy: Double,
    val pw: Double,
    val ph: Double,
    val colorRgb: Int,
    val selected: Boolean,
) {
    val rect: PlanBoxRect get() = PlanBoxRect(cx - pw * 0.5, cy - ph * 0.5, cx + pw * 0.5, cy + ph * 0.5)
}

data class PlanOptimisticPatch(
    val path: List<Pair<Double, Double>>? = null,
    val outline: List<Pair<Double, Double>>? = null,
    val width: Double? = null,
    val layer: String? = null,
    val locked: Boolean? = null,
)

object PlanFloorplannerModel {
    private const val DEFAULT_STATE = "proposed"
    private val railPalette = intArrayOf(0xF59E0B, 0xC084FC, 0xFB7185, 0x34D399, 0x38BDF8, 0xFACC15, 0x94A3B8)
    private val busPalette = intArrayOf(0x14B8A6, 0x38BDF8, 0xF59E0B, 0xA78BFA, 0xF472B6, 0x4ADE80, 0xFB923C, 0xEAB308)

    fun parseHexRgb(hex: String, fallback: Int): Int {
        val clean = hex.trim().removePrefix("#")
        if (clean.length != 6) return fallback
        return clean.toIntOrNull(16) ?: fallback
    }

    private fun stableHash(s: String): Long {
        var h = 0x811c9dc5L
        for (b in s.toByteArray(Charsets.UTF_8)) {
            h = h xor (b.toLong() and 0xFFL)
            h = (h * 0x01000193L) and 0xFFFFFFFFL
        }
        return h
    }

    fun railColorRgb(net: String, fallback: String = ""): Int {
        val up = net.uppercase()
        if ("3V3" in up || "3.3" in up) return 0xF59E0B
        if ("1V8" in up || "1.8" in up || "1V2" in up || "0V" in up) return 0xC084FC
        if ("VBUS" in up || "5V" in up) return 0xFB7185
        if ("VBAT" in up || "VIN" in up || "12V" in up || "24V" in up) return 0xFACC15
        if ("GND" in up) return 0x94A3B8
        val fb = fallback.lowercase()
        if (fb.isNotEmpty() && fb !in setOf("#5a7a9a", "#4c6a88", "#52667a")) {
            return parseHexRgb(fallback, 0xF59E0B)
        }
        return railPalette[(stableHash(net) % railPalette.size).toInt()]
    }

    fun busColorRgb(name: String, fallback: String = ""): Int {
        if (fallback.isNotBlank()) return parseHexRgb(fallback, 0x14B8A6)
        return busPalette[(stableHash(name) % busPalette.size).toInt()]
    }

    private fun parsePts(a: JSONArray?): List<Pair<Double, Double>> {
        if (a == null) return emptyList()
        val out = ArrayList<Pair<Double, Double>>(a.length())
        for (i in 0 until a.length()) {
            val pt = a.optJSONArray(i) ?: continue
            if (pt.length() >= 2) out.add(pt.optDouble(0, 0.0) to pt.optDouble(1, 0.0))
        }
        return out
    }

    private fun parseNums(a: JSONArray?): List<Double> {
        if (a == null) return emptyList()
        return (0 until a.length()).map { a.optDouble(it, 0.0) }
    }

    private fun parseStrs(a: JSONArray?): List<String> {
        if (a == null) return emptyList()
        return (0 until a.length()).mapNotNull { a.optString(it).takeIf(String::isNotEmpty) }
    }

    fun unwrapPanel(json: JSONObject?): PlanPanel? {
        if (json == null) return null
        val target = json.optJSONObject("panel")
            ?: json.optJSONObject("result")?.optJSONObject("panel")
            ?: if (json.has("layers") || json.has("revision") || json.optBoolean("empty", false)) json else null
        return target?.let(::parsePanel)
    }

    fun parsePanel(o: JSONObject): PlanPanel {
        val layersArr = o.optJSONArray("layers")
        val layers = (0 until (layersArr?.length() ?: 0)).mapNotNull { i ->
            val l = layersArr?.optJSONObject(i) ?: return@mapNotNull null
            val itemsArr = l.optJSONArray("items")
            val items = (0 until (itemsArr?.length() ?: 0)).mapNotNull { j ->
                val it = itemsArr?.optJSONObject(j) ?: return@mapNotNull null
                PlanItem(
                    id = it.optString("id"),
                    kind = it.optString("kind", "corridor"),
                    name = it.optString("name"),
                    color = it.optString("color"),
                    state = it.optString("state", "proposed"),
                    nets = it.optInt("nets", 0),
                    routed = it.optInt("routed", 0),
                    total = it.optInt("total", 0),
                    width = if (it.has("width") && !it.isNull("width")) it.optDouble("width") else null,
                    impedance = it.optString("impedance"),
                    reference = it.optString("reference"),
                    locked = it.optBoolean("locked", false),
                    edited = it.optBoolean("edited", false),
                    problems = it.optInt("problems", 0),
                    problemText = it.optString("problem_text"),
                    layer = it.optString("layer"),
                )
            }
            PlanLayer(
                layer = l.optString("layer"),
                index = l.optInt("index", 0),
                title = l.optString("title"),
                subtitle = l.optString("subtitle"),
                role = l.optString("role"),
                status = l.optString("status"),
                done = l.optInt("done", 0),
                approved = l.optInt("approved", 0),
                total = l.optInt("total", 0),
                stackup = l.optString("stackup"),
                items = items,
            )
        }
        val clArr = o.optJSONArray("copper_layers")
        val copper = (0 until (clArr?.length() ?: 0)).mapNotNull { i ->
            val c = clArr?.optJSONObject(i) ?: return@mapNotNull null
            PlanCopperLayer(c.optString("layer"), c.optInt("index", 0), c.optString("role"), c.optString("title"), c.optString("subtitle"))
        }
        val boardObj = o.optJSONObject("board")?.let { b ->
            val bb = parseNums(b.optJSONArray("bbox"))
            val outArr = b.optJSONArray("outline")
            val outline = (0 until (outArr?.length() ?: 0)).map { i -> parsePts(outArr?.optJSONArray(i)) }
            val partsArr = b.optJSONArray("parts")
            val parts = (0 until (partsArr?.length() ?: 0)).mapNotNull { i ->
                val p = partsArr?.optJSONObject(i) ?: return@mapNotNull null
                PlanPartBox(p.optString("r"), p.optString("s"), parseNums(p.optJSONArray("b")), p.optBoolean("o", false))
            }
            PlanBoardGeom(if (bb.size == 4) bb else listOf(0.0, 0.0, 100.0, 70.0), outline, parts)
        }
        val drawObj = o.optJSONObject("draw")?.let { d ->
            val bArr = d.optJSONArray("bands")
            val bands = (0 until (bArr?.length() ?: 0)).mapNotNull { i ->
                val b = bArr?.optJSONObject(i) ?: return@mapNotNull null
                val gArr = b.optJSONArray("guides")
                val guides = (0 until (gArr?.length() ?: 0)).map { j -> parseNums(gArr?.optJSONArray(j)) }
                PlanBand(
                    id = b.optString("id"),
                    layer = b.optString("layer"),
                    name = b.optString("name"),
                    color = b.optString("color"),
                    state = b.optString("state", "proposed"),
                    width = b.optDouble("width", 1.5),
                    nets = b.optInt("nets", 0),
                    locked = b.optBoolean("locked", false),
                    edited = b.optBoolean("edited", false),
                    path = parsePts(b.optJSONArray("path")),
                    vias = parsePts(b.optJSONArray("vias")),
                    guides = guides,
                    from = parseStrs(b.optJSONArray("from")),
                    to = parseStrs(b.optJSONArray("to")),
                )
            }
            val rArr = d.optJSONArray("regions")
            val regions = (0 until (rArr?.length() ?: 0)).mapNotNull { i ->
                val r = rArr?.optJSONObject(i) ?: return@mapNotNull null
                val hArr = r.optJSONArray("holes")
                val holes = (0 until (hArr?.length() ?: 0)).map { j -> parsePts(hArr?.optJSONArray(j)) }
                PlanRegion(
                    id = r.optString("id"),
                    layer = r.optString("layer"),
                    net = r.optString("net"),
                    color = r.optString("color"),
                    state = r.optString("state", "proposed"),
                    locked = r.optBoolean("locked", false),
                    edited = r.optBoolean("edited", false),
                    outline = parsePts(r.optJSONArray("outline")),
                    holes = holes,
                )
            }
            PlanDraw(bands, regions)
        }
        val asksArr = o.optJSONArray("asks")
        val asks = (0 until (asksArr?.length() ?: 0)).mapNotNull { i ->
            asksArr?.optJSONObject(i)?.let { PlanAskNote(it.optString("what"), it.optString("note")) }
        }
        val elseArr = o.optJSONArray("elsewhere")
        val elsewhere = (0 until (elseArr?.length() ?: 0)).mapNotNull { i ->
            elseArr?.optJSONObject(i)?.let { PlanElsewhere(it.optString("name"), it.optString("pcb"), it.optString("rel")) }
        }
        return PlanPanel(
            empty = o.optBoolean("empty", false),
            on = o.optBoolean("on", false),
            live = o.optBoolean("live", false),
            pro = o.optString("pro"),
            boardPath = o.optString("board_path"),
            boardRel = o.optString("board_rel"),
            routing = o.optBoolean("routing", false),
            awaiting = o.optInt("awaiting", 0),
            approved = o.optInt("approved", 0),
            done = o.optInt("done", 0),
            blocked = o.optInt("blocked", 0),
            total = o.optInt("total", 0),
            revision = o.optInt("revision", 0),
            status = o.optString("status"),
            boardTrimmed = o.optBoolean("board_trimmed", false),
            stackupSummary = o.optString("stackup_summary"),
            impedances = parseStrs(o.optJSONArray("impedances")),
            keysNote = o.optString("keys_note"),
            asks = asks,
            elsewhere = elsewhere,
            layers = layers,
            copperLayers = copper,
            board = boardObj,
            draw = drawObj,
        )
    }

    fun allCanvasLayers(planned: List<PlanLayer>, copper: List<PlanCopperLayer>): List<PlanLayer> {
        if (copper.isEmpty()) return planned
        val byName = planned.associateBy { it.layer }
        val out = mutableListOf<PlanLayer>()
        val seen = mutableSetOf<String>()
        for (cl in copper) {
            if (cl.layer.isEmpty()) continue
            seen.add(cl.layer)
            val ex = byName[cl.layer]
            if (ex != null) {
                out.add(ex.copy(
                    title = ex.title.ifEmpty { cl.title },
                    subtitle = ex.subtitle.ifEmpty { cl.subtitle },
                    role = ex.role.ifEmpty { cl.role },
                ))
            } else {
                out.add(PlanLayer(
                    layer = cl.layer,
                    index = cl.index,
                    title = cl.title.ifEmpty { "L${cl.index} · ${cl.layer}" },
                    subtitle = cl.subtitle,
                    role = cl.role,
                    status = DEFAULT_STATE,
                ))
            }
        }
        for (l in planned) {
            if (l.layer.isNotEmpty() && seen.add(l.layer)) out.add(l)
        }
        return out
    }

    fun defaultActiveLayer(layers: List<PlanLayer>): String {
        var bestSig: Pair<Int, String>? = null
        var bestPwr: Pair<Int, String>? = null
        for (l in layers) {
            if (l.layer.isEmpty()) continue
            val corN = l.items.count { it.kind == "corridor" }
            val plnN = l.items.count { it.kind == "plane" }
            if (corN > 0 || l.role == "signal") {
                if (bestSig == null || corN > bestSig.first) bestSig = corN to l.layer
            }
            if (plnN > 0 || l.role == "power" || l.role == "plane") {
                if (bestPwr == null || plnN > bestPwr.first) bestPwr = plnN to l.layer
            }
        }
        return bestSig?.second ?: bestPwr?.second ?: layers.firstOrNull()?.layer ?: ""
    }

    fun layerDisplayTitle(layer: String, copper: List<PlanCopperLayer>, allLayers: List<PlanLayer>): String {
        copper.firstOrNull { it.layer == layer && it.title.isNotEmpty() }?.let { return it.title }
        allLayers.firstOrNull { it.layer == layer && it.title.isNotEmpty() }?.let { return it.title }
        return layer
    }

    fun bandStrokePx(widthMm: Double, scale: Double): Double = (widthMm * scale).coerceIn(6.0, 42.0)

    fun polylinePointAt(pts: List<Pair<Double, Double>>, frac: Double): Pair<Double, Double>? {
        if (pts.isEmpty()) return null
        if (pts.size == 1) return pts[0]
        val f = frac.coerceIn(0.0, 1.0)
        var total = 0.0
        for (i in 1 until pts.size) total += hypot(pts[i].first - pts[i - 1].first, pts[i].second - pts[i - 1].second)
        if (total < 1e-6) return pts[0]
        val target = total * f
        var acc = 0.0
        for (i in 1 until pts.size) {
            val dx = pts[i].first - pts[i - 1].first
            val dy = pts[i].second - pts[i - 1].second
            val len = hypot(dx, dy)
            if (acc + len >= target && len > 1e-9) {
                val t = (target - acc) / len
                return (pts[i - 1].first + dx * t) to (pts[i - 1].second + dy * t)
            }
            acc += len
        }
        return pts.last()
    }

    fun polylineMidpoint(pts: List<Pair<Double, Double>>): Pair<Double, Double>? = polylinePointAt(pts, 0.5)

    fun insertVertex(pts: List<Pair<Double, Double>>, segIdx: Int, pos: Pair<Double, Double>, closed: Boolean): List<Pair<Double, Double>> {
        if (pts.isEmpty()) return listOf(pos)
        val out = pts.toMutableList()
        val at = if (closed && segIdx + 1 >= out.size) out.size else min(segIdx + 1, out.size)
        out.add(at, pos)
        return out
    }

    fun removeVertex(pts: List<Pair<Double, Double>>, idx: Int, closed: Boolean): List<Pair<Double, Double>>? {
        val minCount = if (closed) 3 else 2
        if (pts.size <= minCount || idx !in pts.indices) return null
        return pts.toMutableList().apply { removeAt(idx) }
    }

    fun pointInRing(x: Double, y: Double, ring: List<Pair<Double, Double>>): Boolean {
        if (ring.size < 3) return false
        var inside = false
        var j = ring.size - 1
        for (i in ring.indices) {
            val xi = ring[i].first; val yi = ring[i].second
            val xj = ring[j].first; val yj = ring[j].second
            if ((yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / ((yj - yi) + 1e-12) + xi) {
                inside = !inside
            }
            j = i
        }
        return inside
    }

    fun pourLabelPoint(outer: List<Pair<Double, Double>>, holes: List<List<Pair<Double, Double>>>): Pair<Double, Double>? {
        if (outer.size < 3) return null
        var minx = Double.POSITIVE_INFINITY; var miny = Double.POSITIVE_INFINITY
        var maxx = Double.NEGATIVE_INFINITY; var maxy = Double.NEGATIVE_INFINITY
        var sx = 0.0; var sy = 0.0
        for (p in outer) {
            minx = min(minx, p.first); miny = min(miny, p.second)
            maxx = max(maxx, p.first); maxy = max(maxy, p.second)
            sx += p.first; sy += p.second
        }
        val cands = mutableListOf(((minx + maxx) * 0.5) to ((miny + maxy) * 0.5), (sx / outer.size) to (sy / outer.size))
        for (fy in doubleArrayOf(0.5, 0.25, 0.75)) {
            for (fx in doubleArrayOf(0.5, 0.25, 0.75)) {
                cands.add((minx + (maxx - minx) * fx) to (miny + (maxy - miny) * fy))
            }
        }
        for (c in cands) {
            if (pointInRing(c.first, c.second, outer) && holes.none { pointInRing(c.first, c.second, it) }) return c
        }
        return ((minx + maxx) * 0.5) to ((miny + maxy) * 0.5)
    }

    private fun distSqSeg(px: Double, py: Double, ax: Double, ay: Double, bx: Double, by: Double): Double {
        val vx = bx - ax; val vy = by - ay
        val wx = px - ax; val wy = py - ay
        val c1 = vx * wx + vy * wy
        if (c1 <= 0) return wx * wx + wy * wy
        val c2 = vx * vx + vy * vy
        if (c2 <= c1) return (px - bx) * (px - bx) + (py - by) * (py - by)
        val t = c1 / c2
        val qx = ax + vx * t; val qy = ay + vy * t
        return (px - qx) * (px - qx) + (py - qy) * (py - qy)
    }

    fun hitTest(
        bands: List<PlanBand>,
        regions: List<PlanRegion>,
        layer: String,
        selectedId: String,
        cam: PlanCamera,
        w: Double,
        h: Double,
        x: Double,
        y: Double,
        vertexSlop: Double = 22.0,
        midpointSlop: Double = 18.0,
    ): PlanHit? {
        if (selectedId.isNotEmpty()) {
            bands.firstOrNull { it.id == selectedId && it.layer == layer && !it.locked }?.let { b ->
                for (i in b.path.indices) {
                    val (px, py) = cam.toPx(b.path[i].first, b.path[i].second, w, h)
                    if (hypot(px - x, py - y) <= vertexSlop) return PlanHit.Vertex(b.id, false, i)
                }
                for (i in 0 until max(0, b.path.size - 1)) {
                    val (px, py) = cam.toPx((b.path[i].first + b.path[i + 1].first) * 0.5, (b.path[i].second + b.path[i + 1].second) * 0.5, w, h)
                    if (hypot(px - x, py - y) <= midpointSlop) return PlanHit.Midpoint(b.id, false, i)
                }
            }
            regions.firstOrNull { it.id == selectedId && it.layer == layer && !it.locked }?.let { r ->
                for (i in r.outline.indices) {
                    val (px, py) = cam.toPx(r.outline[i].first, r.outline[i].second, w, h)
                    if (hypot(px - x, py - y) <= vertexSlop) return PlanHit.Vertex(r.id, true, i)
                }
                if (r.outline.size >= 3) {
                    for (i in r.outline.indices) {
                        val nxt = r.outline[(i + 1) % r.outline.size]
                        val (px, py) = cam.toPx((r.outline[i].first + nxt.first) * 0.5, (r.outline[i].second + nxt.second) * 0.5, w, h)
                        if (hypot(px - x, py - y) <= midpointSlop) return PlanHit.Midpoint(r.id, true, i)
                    }
                }
            }
        }
        var best: Pair<Double, String>? = null
        for (b in bands) {
            if (b.layer != layer || b.path.size < 2) continue
            val thresh = max(14.0, bandStrokePx(b.width, cam.scale) * 0.5 + 6.0)
            var minD2 = Double.POSITIVE_INFINITY
            for (i in 0 until b.path.size - 1) {
                val (ax, ay) = cam.toPx(b.path[i].first, b.path[i].second, w, h)
                val (bx, by) = cam.toPx(b.path[i + 1].first, b.path[i + 1].second, w, h)
                minD2 = min(minD2, distSqSeg(x, y, ax, ay, bx, by))
            }
            val d = sqrt(minD2)
            if (d <= thresh && (best == null || d <= best.first)) best = d to b.id
        }
        if (best != null) return PlanHit.Band(best.second)

        val (wx, wy) = cam.toWorld(x, y, w, h)
        for (r in regions.asReversed()) {
            if (r.layer != layer) continue
            if (pointInRing(wx, wy, r.outline) && r.holes.none { pointInRing(wx, wy, it) }) return PlanHit.Region(r.id)
        }
        return null
    }

    fun placeCorridorPills(
        bands: List<PlanBand>,
        cam: PlanCamera,
        w: Double,
        h: Double,
        selectedId: String,
    ): List<PlanCorridorPill> {
        data class Cand(val b: PlanBand, val pxLen: Double, val sel: Boolean, val col: Int)
        val cands = bands.mapNotNull { b ->
            if (b.nets <= 0 || b.path.size < 2) return@mapNotNull null
            var len = 0.0
            for (i in 1 until b.path.size) {
                val (ax, ay) = cam.toPx(b.path[i - 1].first, b.path[i - 1].second, w, h)
                val (bx, by) = cam.toPx(b.path[i].first, b.path[i].second, w, h)
                len += hypot(bx - ax, by - ay)
            }
            Cand(b, len, selectedId == b.id, busColorRgb(b.name.ifEmpty { b.id }, b.color))
        }.sortedWith(compareByDescending<Cand> { it.sel }.thenByDescending { it.b.nets }.thenBy { it.b.id })

        val placed = mutableListOf<PlanBoxRect>()
        val out = mutableListOf<PlanCorridorPill>()
        val clampX = { cx: Double, pw: Double -> val lo = pw * 0.5 + 3.0; cx.coerceIn(lo, max(lo, w - pw * 0.5 - 3.0)) }
        val clampY = { cy: Double, ph: Double -> val lo = ph * 0.5 + 3.0; cy.coerceIn(lo, max(lo, h - ph * 0.5 - 3.0)) }
        for (c in cands) {
            val shortTxt = c.b.nets.toString()
            val fullTxt = if (c.b.name.isNotEmpty()) "${c.b.name} · ${c.b.nets}" else shortTxt
            val texts = if ((c.sel || c.pxLen >= 95.0) && c.b.name.isNotEmpty()) listOf(fullTxt, shortTxt) else listOf(shortTxt)
            var chosen: PlanCorridorPill? = null
            outer@ for (txt in texts) {
                val pw = max(16.0, txt.length * 5.9 + 10.0)
                val ph = 15.0
                for (frac in doubleArrayOf(0.5, 0.35, 0.65, 0.22, 0.78)) {
                    val wp = polylinePointAt(c.b.path, frac) ?: continue
                    val (rx, ry) = cam.toPx(wp.first, wp.second, w, h)
                    val cx = clampX(rx, pw); val cy = clampY(ry, ph)
                    val rect = PlanBoxRect(cx - pw * 0.5, cy - ph * 0.5, cx + pw * 0.5, cy + ph * 0.5).pad(2.0)
                    if (placed.none { it.overlaps(rect) }) {
                        chosen = PlanCorridorPill(c.b.id, txt, cx, cy, pw, ph, c.col, c.sel)
                        break@outer
                    }
                }
            }
            if (chosen == null) {
                val wp = polylinePointAt(c.b.path, 0.5)
                if (wp != null) {
                    val (rx, ry) = cam.toPx(wp.first, wp.second, w, h)
                    val pw = max(16.0, shortTxt.length * 6.5 + 8.0); val ph = 15.0
                    val cx = clampX(rx, pw)
                    for (dy in doubleArrayOf(-(ph + 3.0), ph + 3.0)) {
                        val cy = clampY(ry + dy, ph)
                        val cand = PlanCorridorPill(c.b.id, shortTxt, cx, cy, pw, ph, c.col, c.sel)
                        if (placed.none { it.overlaps(cand.rect.pad(2.0)) }) { chosen = cand; break }
                    }
                    if (chosen == null) chosen = PlanCorridorPill(c.b.id, shortTxt, cx, clampY(ry, ph), pw, ph, c.col, c.sel)
                }
            }
            if (chosen != null) {
                placed.add(chosen.rect.pad(2.0))
                out.add(chosen)
            }
        }
        return out
    }

    fun layoutLabels(
        parts: List<PlanPartBox>,
        cam: PlanCamera,
        w: Double,
        h: Double,
        pinned: List<String>,
        blocked: List<PlanBoxRect> = emptyList(),
    ): List<PlanPlacedLabel> {
        val pinSet = pinned.toSet()
        data class Cand(val text: String, val cx: Double, val cy: Double, val bw: Double, val bh: Double, val score: Double)
        val cands = parts.mapNotNull { p ->
            if (p.r.isEmpty() || p.b.size != 4) return@mapNotNull null
            val (x0, y0) = cam.toPx(p.b[0], p.b[1], w, h)
            val (x1, y1) = cam.toPx(p.b[2], p.b[3], w, h)
            val pw = abs(x1 - x0); val ph = abs(y1 - y0)
            if (pw < 13.0 || ph < 9.0) return@mapNotNull null
            val cx = (x0 + x1) * 0.5; val cy = (y0 + y1) * 0.5
            if (cx < 8.0 || cx > w - 8.0 || cy < 8.0 || cy > h - 8.0) return@mapNotNull null
            Cand(p.r, cx, cy, p.r.length * 5.8 + 4.0, 11.0, pw * ph + if (p.r in pinSet) 10000.0 else 0.0)
        }.sortedByDescending { it.score }

        val placed = blocked.toMutableList()
        val out = mutableListOf<PlanPlacedLabel>()
        for (c in cands) {
            val rect = PlanBoxRect(c.cx - c.bw * 0.5, c.cy - c.bh * 0.5, c.cx + c.bw * 0.5, c.cy + c.bh * 0.5)
            if (placed.none { it.overlaps(rect) }) {
                placed.add(rect)
                out.add(PlanPlacedLabel(c.text, c.cx, c.cy))
            }
        }
        return out
    }

    fun mergeOptimistic(panel: PlanPanel, patches: Map<String, PlanOptimisticPatch>): PlanPanel {
        if (patches.isEmpty()) return panel
        val draw = panel.draw ?: PlanDraw()
        val newBands = draw.bands.map { b ->
            val p = patches[b.id] ?: return@map b
            b.copy(
                path = p.path ?: b.path,
                width = p.width ?: b.width,
                layer = p.layer ?: b.layer,
                locked = p.locked ?: b.locked,
                edited = true,
                state = if (b.state == "proposed") "edited" else b.state,
            )
        }
        val newRegions = draw.regions.map { r ->
            val p = patches[r.id] ?: return@map r
            r.copy(
                outline = p.outline ?: r.outline,
                layer = p.layer ?: r.layer,
                locked = p.locked ?: r.locked,
                edited = true,
                state = if (r.state == "proposed") "edited" else r.state,
            )
        }
        val moved = mutableListOf<Pair<String, PlanItem>>()
        val newLayers = panel.layers.map { l ->
            val kept = mutableListOf<PlanItem>()
            for (it in l.items) {
                val p = patches[it.id]
                if (p == null) { kept.add(it); continue }
                val next = it.copy(
                    width = p.width ?: it.width,
                    locked = p.locked ?: it.locked,
                    edited = true,
                    state = if (it.state == "proposed") "edited" else it.state,
                    layer = p.layer ?: it.layer,
                )
                if (p.layer != null && p.layer != l.layer) moved.add(p.layer to next) else kept.add(next)
            }
            l.copy(items = kept)
        }.toMutableList()
        for ((target, item) in moved) {
            val idx = newLayers.indexOfFirst { it.layer == target }
            if (idx >= 0) newLayers[idx] = newLayers[idx].copy(items = newLayers[idx].items + item)
        }
        return panel.copy(layers = newLayers, draw = draw.copy(bands = newBands, regions = newRegions))
    }

    fun round2(v: Double): Double = (v * 100.0).roundToInt() / 100.0
    fun fmt2(v: Double): String = String.format(java.util.Locale.US, "%.2f", v)
}
