package dev.backplane.mobile

import android.provider.Settings
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameMillis
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Dp
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.max

// A bot's cat, drawn from its rig. The
// rig decides everything: parts back to front in a 64 x 64 box, path data
// (absolute M L C Q Z), even-odd fill and/or a round outline, opacity, and
// animations applied outer first. The math is the rig's, as the window and
// the web do it:
//   t = ((ms - delay) mod per) / per; w(x) = (1 - cos(2 pi x)) / 2
//   io: w(t); blink: 0 below 0.94, else w((t - 0.94) / 0.06); saw: t
//   value = from + (to - from) * e

class CatAnim(
    val k: String, val from: Float, val to: Float,
    val px: Float, val py: Float, val per: Int, val delay: Int, val ease: String,
)

class CatPart(
    val name: String, val d: String, val fill: Int?, val stroke: Int?,
    val sw: Float, val alpha: Int, val anims: List<CatAnim>,
) {
    val path: Path by lazy { catPath(d) }
}

fun catRig(json: JSONArray): List<CatPart> = (0 until json.length()).mapNotNull { i ->
    val o = json.optJSONObject(i) ?: return@mapNotNull null
    val a = o.optJSONArray("anims") ?: JSONArray()
    CatPart(
        o.optString("name"), o.optString("d"), colorOf(o, "fill"), colorOf(o, "stroke"),
        o.optDouble("sw", 0.0).toFloat(), o.optInt("alpha", 255),
        (0 until a.length()).mapNotNull { j -> a.optJSONObject(j)?.let(::catAnim) },
    )
}

private fun colorOf(o: JSONObject, key: String): Int? =
    if (!o.has(key) || o.isNull(key)) null else o.optInt(key)

private fun catAnim(o: JSONObject) = CatAnim(
    o.optString("k"), o.optDouble("from").toFloat(), o.optDouble("to").toFloat(),
    o.optDouble("px").toFloat(), o.optDouble("py").toFloat(),
    o.optInt("per", 1), o.optInt("delay", 0), o.optString("ease", "io"),
)

// path data: absolute M L C Q Z only (quadratics become cubics)
fun catPath(d: String): Path {
    val p = Path().apply { fillType = PathFillType.EvenOdd }
    val toks = Regex("[MLCQZ]|-?[0-9]*\\.?[0-9]+").findAll(d).map { it.value }.toList()
    var i = 0
    var cmd = 'M'
    var x = 0f
    var y = 0f
    fun num(): Float = toks.getOrNull(i++)?.toFloatOrNull() ?: 0f
    while (i < toks.size) {
        val t = toks[i]
        if (t[0].isLetter()) { cmd = t[0]; i++ }
        when (cmd) {
            'M' -> { x = num(); y = num(); p.moveTo(x, y); cmd = 'L' }
            'L' -> { x = num(); y = num(); p.lineTo(x, y) }
            'C' -> {
                val x1 = num(); val y1 = num(); val x2 = num(); val y2 = num()
                x = num(); y = num(); p.cubicTo(x1, y1, x2, y2, x, y)
            }
            'Q' -> {
                val qx = num(); val qy = num(); val ex = num(); val ey = num()
                p.cubicTo(x + 2f / 3f * (qx - x), y + 2f / 3f * (qy - y), ex + 2f / 3f * (qx - ex), ey + 2f / 3f * (qy - ey), ex, ey)
                x = ex; y = ey
            }
            'Z' -> { p.close(); cmd = ' ' }
            else -> i++
        }
    }
    return p
}

fun catValue(a: CatAnim, ms: Long): Float {
    val per = max(a.per, 1)
    val off = ((per - a.delay % per) % per).toLong()
    val t = ((ms + off) % per).toFloat() / per
    fun w(x: Float) = ((1.0 - cos(2.0 * PI * x)) / 2.0).toFloat()
    val e = when (a.ease) {
        "saw" -> t
        "blink" -> if (t < 0.94f) 0f else w((t - 0.94f) / 0.06f)
        else -> w(t)
    }
    return a.from + (a.to - a.from) * e
}

// the parts at ms, onto a canvas already scaled to the 64 box
fun DrawScope.drawCat(rig: List<CatPart>, ms: Long) {
    for (p in rig) {
        var op = p.alpha / 255f
        for (a in p.anims) if (a.k == "op") op *= catValue(a, ms)
        if (op <= 0f) continue
        withTransform({
            for (a in p.anims) {
                val v = catValue(a, ms)
                when (a.k) {
                    "rot" -> rotate(v, Offset(a.px, a.py))
                    "tx" -> translate(v, 0f)
                    "ty" -> translate(0f, v)
                    "sx" -> scale(v, 1f, Offset(a.px, a.py))
                    "sy" -> scale(1f, v, Offset(a.px, a.py))
                }
            }
        }) {
            val alpha = op.coerceIn(0f, 1f)
            p.fill?.let { drawPath(p.path, Color(0xFF000000.toInt() or it), alpha = alpha) }
            p.stroke?.let {
                drawPath(p.path, Color(0xFF000000.toInt() or it), alpha = alpha,
                    style = Stroke(width = p.sw, cap = StrokeCap.Round, join = StrokeJoin.Round))
            }
        }
    }
}

// A cat of size x size. It animates while any part moves, unless the
// system asks for no animation (then it holds its first pose).
@Composable
fun CatView(rig: List<CatPart>, size: Dp, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val still = remember {
        Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
    }
    val moving = !still && rig.any { it.anims.isNotEmpty() }
    var ms by remember(rig) { mutableLongStateOf(0L) }
    if (moving) {
        LaunchedEffect(rig) {
            val t0 = withFrameMillis { it }
            while (true) withFrameMillis { ms = it - t0 }
        }
    }
    Canvas(modifier.size(size)) {
        val s = this.size.minDimension / 64f
        withTransform({ scale(s, s, Offset.Zero) }) { drawCat(rig, ms) }
    }
}
