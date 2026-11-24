package dev.backplane.mobile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.acos
import kotlin.math.ceil
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin
import kotlin.math.sqrt

/**
 * Vector marks for every built-in AI provider, matching the bundled provider SVGs and desktop GPUI (`art.rs` / `picker.rs`).
 * Pure path parsing and canonicalization live in [ProviderMarks] so JVM unit tests can exercise them without Android UI.
 */
object ProviderMarks {
    data class Spec(val id: String, val color: Long, val letter: String, val vbW: Float, val vbH: Float, val d: String)

    sealed class Seg {
        data class Move(val x: Float, val y: Float) : Seg()
        data class Line(val x: Float, val y: Float) : Seg()
        data class Cubic(val x1: Float, val y1: Float, val x2: Float, val y2: Float, val x: Float, val y: Float) : Seg()
        data class Quad(val x1: Float, val y1: Float, val x: Float, val y: Float) : Seg()
        data object Close : Seg()
    }

    private const val CLAUDE_D = "m4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z"
    private const val CODEX_D = "M239.184 106.203a64.716 64.716 0 0 0-5.576-53.103C219.452 28.459 191 15.784 163.213 21.74A65.586 65.586 0 0 0 52.096 45.22a64.716 64.716 0 0 0-43.23 31.36c-14.31 24.602-11.061 55.634 8.033 76.74a64.665 64.665 0 0 0 5.525 53.102c14.174 24.65 42.644 37.324 70.446 31.36a64.72 64.72 0 0 0 48.754 21.744c28.481.025 53.714-18.361 62.414-45.481a64.767 64.767 0 0 0 43.229-31.36c14.137-24.558 10.875-55.423-8.083-76.483Zm-97.56 136.338a48.397 48.397 0 0 1-31.105-11.255l1.535-.87l51.67-29.825a8.595 8.595 0 0 0 4.247-7.367v-72.85l21.845 12.636c.218.111.37.32.409.563v60.367c-.056 26.818-21.783 48.545-48.601 48.601Zm-104.466-44.61a48.345 48.345 0 0 1-5.781-32.589l1.534.921l51.722 29.826a8.339 8.339 0 0 0 8.441 0l63.181-36.425v25.221a.87.87 0 0 1-.358.665l-52.335 30.184c-23.257 13.398-52.97 5.431-66.404-17.803ZM23.549 85.38a48.499 48.499 0 0 1 25.58-21.333v61.39a8.288 8.288 0 0 0 4.195 7.316l62.874 36.272l-21.845 12.636a.819.819 0 0 1-.767 0L41.353 151.53c-23.211-13.454-31.171-43.144-17.804-66.405v.256Zm179.466 41.695l-63.08-36.63L161.73 77.86a.819.819 0 0 1 .768 0l52.233 30.184a48.6 48.6 0 0 1-7.316 87.635v-61.391a8.544 8.544 0 0 0-4.4-7.213Zm21.742-32.69l-1.535-.922l-51.619-30.081a8.39 8.39 0 0 0-8.492 0L99.98 99.808V74.587a.716.716 0 0 1 .307-.665l52.233-30.133a48.652 48.652 0 0 1 72.236 50.391v.205ZM88.061 139.097l-21.845-12.585a.87.87 0 0 1-.41-.614V65.685a48.652 48.652 0 0 1 79.757-37.346l-1.535.87l-51.67 29.825a8.595 8.595 0 0 0-4.246 7.367l-.051 72.697Zm11.868-25.58L128.067 97.3l28.188 16.218v32.434l-28.086 16.218l-28.188-16.218l-.052-32.434Z"
    private const val GROK_D = "M9.497 15.122 L16.811 9.491 C17.169 9.215 17.682 9.323 17.852 9.751 C18.752 12.013 18.35 14.73 16.561 16.596 C14.772 18.462 12.283 18.871 10.007 17.939 L7.522 19.139 C11.087 21.68 15.416 21.052 18.121 18.229 C20.267 15.991 20.931 12.941 20.31 10.19 L20.315 10.196 C19.414 6.155 20.537 4.54 22.837 1.237 C22.891 1.159 22.946 1.08 23 1 L19.974 4.156 L19.974 4.146 L9.495 15.124 M7.987 16.49 C5.429 13.941 5.87 9.996 8.053 7.721 C9.667 6.037 12.313 5.35 14.622 6.361 L17.102 5.166 C16.655 4.829 16.082 4.467 15.425 4.213 C12.455 2.938 8.9 3.573 6.485 6.088 C4.163 8.51 3.433 12.234 4.687 15.411 C5.624 17.786 4.088 19.466 2.541 21.162 C1.993 21.763 1.443 22.364 1 23 L7.985 16.492"
    private const val MUSE_D = "M10.73.032c-1.333 0-2.032 1.016-2.032 2.285 0 2.223 2.127 4.953 4.318 4.953 1.301 0 2-.953 2-2.254 0-2.254-2.095-4.984-4.286-4.984m8.413 2.984c-1.968 0-3.397 2.73-3.397 4.825 0 1.556.794 3.016 2.254 3.016 2 0 3.365-2.762 3.365-4.794 0-1.523-.762-3.047-2.222-3.047M4.857 4.159c-1.968 0-3.778 1.016-3.778 2.54 0 1.65 2.16 2.793 4.254 2.793 1.873 0 3.778-.92 3.778-2.508 0-1.682-2.159-2.825-4.254-2.825m-.222 6.476C2.413 10.635 0 13.175 0 15.397c0 1.301.825 2.159 2.095 2.159 2.19 0 4.603-2.508 4.603-4.794 0-1.27-.761-2.127-2.063-2.127m16.667.698c-2.223 0-5.016 1.62-5.016 3.651 0 1.238 1.047 2.19 2.698 2.19 2.159 0 5.016-1.587 5.016-3.682 0-1.302-1.08-2.159-2.698-2.159M7.619 16c-1.524 0-2.38 1.746-2.38 3.429 0 2.063 1.269 4.54 3.047 4.54 1.524 0 2.38-1.81 2.38-3.461C10.667 18.57 9.46 16 7.62 16m6.571 1.016c-1.333 0-2.476.667-2.476 2.032 0 2.095 2.667 4.063 4.92 4.063 1.366 0 2.54-.73 2.54-2.063 0-2.032-2.698-4.032-4.984-4.032"
    private const val ANTIGRAVITY_D = "M20.197 20.618 C21.318 21.459 23 20.898 21.459 19.357 C16.834 14.873 17.815 2.541 12.07 2.541 C6.325 2.541 7.306 14.873 2.682 19.357 C1 21.038 2.822 21.459 3.943 20.618 C8.287 17.675 8.006 12.49 12.07 12.49 C16.134 12.49 15.853 17.675 20.197 20.618 Z"
    private const val OPENCODE_D = "M22 24H2V0h20zM17 4.8H7v14.4h10z"
    private const val PI_D = "M0 0v24h6v-6h6v-6H6V6h6v6h6V0Zm18 12v12h6V12Z"
    private const val OMP_D = "M1.44 3.86 L22.56 3.86 C23 3.86 23 3.86 23 4.3 L23 6.06 C23 6.5 23 6.5 22.56 6.5 L1.44 6.5 C1 6.5 1 6.5 1 6.06 L1 4.3 C1 3.86 1 3.86 1.44 3.86 Z M4.74 6.5 L6.5 6.5 C6.94 6.5 6.94 6.5 6.94 6.94 L6.94 19.7 C6.94 20.14 6.94 20.14 6.5 20.14 L4.74 20.14 C4.3 20.14 4.3 20.14 4.3 19.7 L4.3 6.94 C4.3 6.5 4.3 6.5 4.74 6.5 Z M15.74 6.5 L17.5 6.5 C17.94 6.5 17.94 6.5 17.94 6.94 L17.94 13.32 C17.94 13.76 17.94 13.76 17.5 13.76 L15.74 13.76 C15.3 13.76 15.3 13.76 15.3 13.32 L15.3 6.94 C15.3 6.5 15.3 6.5 15.74 6.5 Z M15.08 14.64 L18.16 14.64 C18.82 14.64 18.82 14.64 18.82 15.3 L18.82 17.5 C18.82 18.16 18.82 18.16 18.16 18.16 L15.08 18.16 C14.42 18.16 14.42 18.16 14.42 17.5 L14.42 15.3 C14.42 14.64 14.42 14.64 15.08 14.64 Z"

    val ALL: List<Spec> = listOf(
        Spec("claude", 0xD97757L, "A", 24f, 24f, CLAUDE_D),
        Spec("codex", 0x10A37FL, "O", 256f, 260f, CODEX_D),
        Spec("grok", 0x6B7280L, "X", 24f, 24f, GROK_D),
        Spec("muse", 0x8B5CF6L, "M", 24f, 24f, MUSE_D),
        Spec("antigravity", 0x4285F4L, "G", 24f, 24f, ANTIGRAVITY_D),
        Spec("opencode", 0x2563EBL, "C", 24f, 24f, OPENCODE_D),
        Spec("pi", 0xDB2777L, "π", 24f, 24f, PI_D),
        Spec("omp", 0xC026D3L, "Ω", 24f, 24f, OMP_D),
    )

    private val byId = ALL.associateBy { it.id }

    fun canonical(raw: String): String {
        val k = raw.trim().removePrefix("@").lowercase()
        return when (k) {
            "anthropic" -> "claude"
            "openai" -> "codex"
            "agy", "gemini" -> "antigravity"
            else -> k
        }
    }

    fun spec(raw: String): Spec? = byId[canonical(raw)]

    fun color(raw: String): Long = spec(raw)?.color ?: 0x64748BL

    fun letter(raw: String): String = spec(raw)?.letter ?: "?"

    fun providerOfChip(chip: String): String {
        val head = chip.substringBefore(" / ").substringBefore(" · ").trim()
        return if (spec(head) != null) canonical(head) else ""
    }

    private val parsedCache: Map<String, List<Seg>> = ALL.associate { it.id to parsePath(it.d) }

    fun segments(raw: String): List<Seg>? = spec(raw)?.let { parsedCache[it.id] }

    /**
     * Parses SVG path `d` supporting M/m, L/l, H/h, V/v, C/c, S/s, Q/q, T/t, A/a, Z/z.
     * Handles comma/whitespace delimiters, glued signs/decimals, and compact SVG arc flags.
     */
    fun parsePath(d: String): List<Seg> {
        val n = d.length
        var idx = 0

        fun skipSep() {
            while (idx < n && (d[idx].isWhitespace() || d[idx] == ',')) idx++
        }

        fun hasNumber(): Boolean {
            skipSep()
            if (idx >= n) return false
            val c = d[idx]
            return c == '-' || c == '+' || c == '.' || c in '0'..'9'
        }

        fun nextNum(): Float {
            skipSep()
            val start = idx
            if (idx < n && (d[idx] == '-' || d[idx] == '+')) idx++
            var sawDot = false
            while (idx < n) {
                val c = d[idx]
                if (c in '0'..'9') {
                    idx++
                } else if (c == '.' && !sawDot) {
                    sawDot = true
                    idx++
                } else if ((c == 'e' || c == 'E') && idx > start) {
                    idx++
                    if (idx < n && (d[idx] == '-' || d[idx] == '+')) idx++
                } else {
                    break
                }
            }
            return d.substring(start, idx).toFloatOrNull() ?: 0f
        }

        fun nextFlag(): Boolean {
            skipSep()
            if (idx >= n) return false
            val c = d[idx]
            idx++
            return c == '1'
        }

        val out = ArrayList<Seg>()
        var cmd = 'M'
        var x = 0f
        var y = 0f
        var sx = 0f
        var sy = 0f
        var lastC: Pair<Float, Float>? = null
        var lastQ: Pair<Float, Float>? = null

        while (true) {
            skipSep()
            if (idx >= n) break
            val before = idx
            val ch = d[idx]
            if (ch.isLetter()) {
                cmd = ch
                idx++
                if (cmd == 'Z' || cmd == 'z') {
                    out.add(Seg.Close)
                    x = sx
                    y = sy
                    lastC = null
                    lastQ = null
                    continue
                }
            } else if (!hasNumber()) {
                idx++
                continue
            }

            val rel = cmd.isLowerCase()
            val ox = if (rel) x else 0f
            val oy = if (rel) y else 0f
            var nextC: Pair<Float, Float>? = null
            var nextQ: Pair<Float, Float>? = null

            when (cmd.uppercaseChar()) {
                'M' -> {
                    x = ox + nextNum(); y = oy + nextNum()
                    sx = x; sy = y
                    out.add(Seg.Move(x, y))
                    cmd = if (rel) 'l' else 'L'
                }
                'L' -> {
                    x = ox + nextNum(); y = oy + nextNum()
                    out.add(Seg.Line(x, y))
                }
                'H' -> {
                    x = ox + nextNum()
                    out.add(Seg.Line(x, y))
                }
                'V' -> {
                    y = oy + nextNum()
                    out.add(Seg.Line(x, y))
                }
                'C' -> {
                    val x1 = ox + nextNum(); val y1 = oy + nextNum()
                    val x2 = ox + nextNum(); val y2 = oy + nextNum()
                    x = ox + nextNum(); y = oy + nextNum()
                    out.add(Seg.Cubic(x1, y1, x2, y2, x, y))
                    nextC = x2 to y2
                }
                'S' -> {
                    val (x1, y1) = lastC?.let { (2f * x - it.first) to (2f * y - it.second) } ?: (x to y)
                    val x2 = ox + nextNum(); val y2 = oy + nextNum()
                    x = ox + nextNum(); y = oy + nextNum()
                    out.add(Seg.Cubic(x1, y1, x2, y2, x, y))
                    nextC = x2 to y2
                }
                'Q' -> {
                    val qx = ox + nextNum(); val qy = oy + nextNum()
                    x = ox + nextNum(); y = oy + nextNum()
                    out.add(Seg.Quad(qx, qy, x, y))
                    nextQ = qx to qy
                }
                'T' -> {
                    val (qx, qy) = lastQ?.let { (2f * x - it.first) to (2f * y - it.second) } ?: (x to y)
                    x = ox + nextNum(); y = oy + nextNum()
                    out.add(Seg.Quad(qx, qy, x, y))
                    nextQ = qx to qy
                }
                'A' -> {
                    val rx = nextNum(); val ry = nextNum(); val rot = nextNum()
                    val large = nextFlag(); val sweep = nextFlag()
                    val ex = ox + nextNum(); val ey = oy + nextNum()
                    arcToCubics(x.toDouble(), y.toDouble(), rx.toDouble(), ry.toDouble(), rot.toDouble(), large, sweep, ex.toDouble(), ey.toDouble(), out)
                    x = ex; y = ey
                }
                else -> if (idx == before) idx++
            }
            if (idx == before) idx++
            lastC = nextC
            lastQ = nextQ
        }
        return out
    }

    /** W3C SVG 1.1 Implementation Notes F.6.5 / F.6.6 elliptical arc to cubic Bezier conversion. */
    private fun arcToCubics(
        x0: Double, y0: Double,
        rxIn: Double, ryIn: Double,
        xAxisRotationDeg: Double,
        largeArc: Boolean, sweep: Boolean,
        x: Double, y: Double,
        out: MutableList<Seg>,
    ) {
        if (x0 == x && y0 == y) return
        var rx = abs(rxIn)
        var ry = abs(ryIn)
        if (rx == 0.0 || ry == 0.0) {
            out.add(Seg.Line(x.toFloat(), y.toFloat()))
            return
        }
        val phi = xAxisRotationDeg * PI / 180.0
        val cosPhi = cos(phi)
        val sinPhi = sin(phi)

        val dx2 = (x0 - x) / 2.0
        val dy2 = (y0 - y) / 2.0
        val x1p = cosPhi * dx2 + sinPhi * dy2
        val y1p = -sinPhi * dx2 + cosPhi * dy2

        val lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry)
        if (lambda > 1.0) {
            val s = sqrt(lambda)
            rx *= s
            ry *= s
        }

        val rx2 = rx * rx
        val ry2 = ry * ry
        val x1p2 = x1p * x1p
        val y1p2 = y1p * y1p

        val num = max(0.0, rx2 * ry2 - rx2 * y1p2 - ry2 * x1p2)
        val den = rx2 * y1p2 + ry2 * x1p2
        val factor = if (den == 0.0) 0.0 else sqrt(num / den) * if (largeArc == sweep) -1.0 else 1.0
        val cxp = factor * (rx * y1p / ry)
        val cyp = factor * (-ry * x1p / rx)

        val cx = cosPhi * cxp - sinPhi * cyp + (x0 + x) / 2.0
        val cy = sinPhi * cxp + cosPhi * cyp + (y0 + y) / 2.0

        fun vecAngle(ux: Double, uy: Double, vx: Double, vy: Double): Double {
            val sign = if (ux * vy - uy * vx < 0.0) -1.0 else 1.0
            val mag = sqrt((ux * ux + uy * uy) * (vx * vx + vy * vy))
            if (mag == 0.0) return 0.0
            val dot = ((ux * vx + uy * vy) / mag).coerceIn(-1.0, 1.0)
            return sign * acos(dot)
        }

        val ux = (x1p - cxp) / rx
        val uy = (y1p - cyp) / ry
        val vx = (-x1p - cxp) / rx
        val vy = (-y1p - cyp) / ry

        val theta1 = vecAngle(1.0, 0.0, ux, uy)
        var dTheta = vecAngle(ux, uy, vx, vy)
        if (!sweep && dTheta > 0.0) dTheta -= 2.0 * PI
        else if (sweep && dTheta < 0.0) dTheta += 2.0 * PI

        val segs = max(1, ceil(abs(dTheta) / (PI / 2.0)).toInt())
        val delta = dTheta / segs
        val t = (4.0 / 3.0) * kotlin.math.tan(delta / 4.0)

        for (i in 0 until segs) {
            val t1 = theta1 + i * delta
            val t2 = theta1 + (i + 1) * delta
            val cos1 = cos(t1); val sin1 = sin(t1)
            val cos2 = cos(t2); val sin2 = sin(t2)

            val p1x = cos1 - t * sin1
            val p1y = sin1 + t * cos1
            val p2x = cos2 + t * sin2
            val p2y = sin2 - t * cos2

            fun mapX(px: Double, py: Double) = (cosPhi * rx * px - sinPhi * ry * py + cx).toFloat()
            fun mapY(px: Double, py: Double) = (sinPhi * rx * px + cosPhi * ry * py + cy).toFloat()

            out.add(
                Seg.Cubic(
                    mapX(p1x, p1y), mapY(p1x, p1y),
                    mapX(p2x, p2y), mapY(p2x, p2y),
                    if (i == segs - 1) x.toFloat() else mapX(cos2, sin2),
                    if (i == segs - 1) y.toFloat() else mapY(cos2, sin2),
                )
            )
        }
    }
}

private val providerComposePaths: Map<String, Path> by lazy {
    ProviderMarks.ALL.associate { spec ->
        val p = Path().apply { fillType = PathFillType.NonZero }
        for (s in ProviderMarks.segments(spec.id).orEmpty()) {
            when (s) {
                is ProviderMarks.Seg.Move -> p.moveTo(s.x, s.y)
                is ProviderMarks.Seg.Line -> p.lineTo(s.x, s.y)
                is ProviderMarks.Seg.Cubic -> p.cubicTo(s.x1, s.y1, s.x2, s.y2, s.x, s.y)
                is ProviderMarks.Seg.Quad -> p.quadraticTo(s.x1, s.y1, s.x, s.y)
                ProviderMarks.Seg.Close -> p.close()
            }
        }
        spec.id to p
    }
}

/**
 * Draws the provider's vector mark tinted in its brand color, or a colored square with "?" for unknown providers.
 */
@Composable
fun ProviderMark(provider: String, size: Dp = 14.dp, modifier: Modifier = Modifier) {
    val spec = remember(provider) { ProviderMarks.spec(provider) }
    if (spec != null) {
        val path = remember(spec.id) { providerComposePaths[spec.id] }
        val tint = remember(spec.color) { Color(0xFF000000L or spec.color) }
        if (path != null) {
            Canvas(modifier.size(size)) {
                val s = min(this.size.width / spec.vbW, this.size.height / spec.vbH)
                val ox = (this.size.width - spec.vbW * s) * 0.5f
                val oy = (this.size.height - spec.vbH * s) * 0.5f
                withTransform({
                    translate(ox, oy)
                    scale(s, s, androidx.compose.ui.geometry.Offset.Zero)
                }) {
                    drawPath(path, color = tint)
                }
            }
            return
        }
    }
    Box(modifier.size(size).background(Color(0xFF64748BL)), contentAlignment = Alignment.Center) {
        Text("?", color = Color.White, fontFamily = BpMono, fontSize = (size.value * 0.62f).coerceAtLeast(8f).sp, fontWeight = FontWeight.Bold, lineHeight = (size.value * 0.7f).coerceAtLeast(9f).sp)
    }
}
