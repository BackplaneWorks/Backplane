package dev.backplane.mobile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import java.util.Locale
import kotlin.math.ceil
import kotlin.math.floor

private data class ReflowAlloy(val id: String, val composition: String, val solidus: Double, val liquidus: Double)

// All temperatures and fractions come from Backplane.reflow on the engine's
// worker. Playback only interpolates samples; there is no second thermal model.
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun ReflowSheet(model: AppModel, dismiss: () -> Unit) {
    val scope = rememberCoroutineScope()
    var profile by remember { mutableStateOf("") }
    var defaults by remember { mutableStateOf("") }
    var alloy by remember { mutableStateOf("SAC305") }
    var alloys by remember { mutableStateOf(emptyList<ReflowAlloy>()) }
    var samples by remember { mutableStateOf(emptyList<DoubleArray>()) }
    var duration by remember { mutableStateOf(0f) }
    var time by remember { mutableStateOf(0f) }
    var speed by remember { mutableStateOf(10f) }
    var playing by remember { mutableStateOf(false) }
    var overlay by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf("") }
    var revision by remember { mutableStateOf(0) }
    var active by remember { mutableStateOf(true) }
    fun sample(): DoubleArray {
        if (samples.isEmpty()) return doubleArrayOf(0.0, 25.0, 25.0, 0.0)
        val t = time.coerceIn(0f, duration).toDouble()
        val a = samples[t.toInt().coerceAtMost(samples.lastIndex)]
        val b = samples[ceil(t).toInt().coerceAtMost(samples.lastIndex)]
        return DoubleArray(4) { a[it] + (b[it] - a[it]) * (t - floor(t)) }
    }
    fun paint() {
        val s = sample(); model.reflowThermal = floatArrayOf(if (overlay) 1f else 0f, s[2].toFloat(), s[3].toFloat())
    }
    suspend fun rebuild() {
        playing = false; revision++; val ticket = revision
        val result = runCatching { JSONObject(model.reflow(profile, alloy)) }.getOrNull()
        if (!active || ticket != revision) return
        if (result == null) { error = "Reflow planner unavailable"; return }
        if (result.has("error")) { error = result.optString("error"); return }
        val rows = result.optJSONArray("samples")
        if (rows == null || rows.length() == 0) { error = "Invalid planner reply"; return }
        samples = (0 until rows.length()).map { i -> DoubleArray(4) { k -> rows.getJSONArray(i).getDouble(k) } }
        duration = result.getDouble("duration").toFloat(); time = time.coerceAtMost(duration); error = ""; paint()
    }
    LaunchedEffect(Unit) {
        defaults = model.reflowDefaults(); profile = defaults
        val list = runCatching { JSONArray(model.reflowAlloys()) }.getOrNull()
        alloys = if (list == null) emptyList() else (0 until list.length()).map {
            val a = list.getJSONObject(it); ReflowAlloy(a.getString("id"), a.getString("composition"), a.getDouble("solidus"), a.getDouble("liquidus"))
        }
        rebuild()
    }
    LaunchedEffect(playing) {
        if (!playing) return@LaunchedEffect
        var last = android.os.SystemClock.elapsedRealtime()
        while (playing) {
            delay(100)
            val now = android.os.SystemClock.elapsedRealtime()
            time = (time + ((now - last) / 1000f).coerceAtMost(.25f) * speed).coerceAtMost(duration); last = now; paint()
            if (time >= duration) playing = false
        }
    }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    DisposableEffect(lifecycle) {
        val observer = LifecycleEventObserver { _, event -> if (event == Lifecycle.Event.ON_STOP) playing = false }
        lifecycle.addObserver(observer)
        onDispose { active = false; revision++; playing = false; lifecycle.removeObserver(observer); model.reflowThermal = floatArrayOf(0f, 25f, 0f) }
    }
    ModalBottomSheet(onDismissRequest = dismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(16.dp).navigationBarsPadding(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text("Illustrative reflow preview", style = MaterialTheme.typography.titleLarge)
            Text("Editable example, not a qualified process. CAD geometry stays unchanged.", style = MaterialTheme.typography.bodySmall)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                for (a in alloys) FilterChip(selected = a.id == alloy, onClick = { alloy = a.id; scope.launch { rebuild() } }, label = { Text(a.id) })
            }
            alloys.firstOrNull { it.id == alloy }?.let { Text("${it.composition} · solidus ${it.solidus} °C / liquidus ${it.liquidus} °C", style = MaterialTheme.typography.bodySmall) }
            val context = LocalContext.current
            TextButton(onClick = { context.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse("https://www.indium.com/products/alloys/solder-alloys/"))) }) { Text("Alloy data · Indium Corporation") }
            val s = sample()
            Text(String.format(Locale.ROOT, "Furnace %.1f °C · board mean %.1f °C · liquid %.1f%%", s[1], s[2], s[3] * 100), style = MaterialTheme.typography.bodySmall)
            ReflowGraph(samples, duration, time, false)
            ReflowGraph(samples, duration, time, true)
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                TextButton(onClick = { playing = false; time = 0f; paint() }) { Text("Restart") }
                Button(enabled = samples.isNotEmpty(), onClick = { if (time >= duration) time = 0f; playing = !playing }) { Text(if (playing) "Pause" else "Play") }
            }
            FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) { for (n in listOf(1f, 10f, 30f, 60f)) FilterChip(selected = speed == n, onClick = { speed = n }, label = { Text("${n.toInt()}×") }) }
            Text(String.format(Locale.ROOT, "%.1f / %.0f s", time, duration))
            Slider(value = time, valueRange = 0f..maxOf(duration, 1f), onValueChange = { playing = false; time = it; paint() })
            Text(if (s[3] >= .999) "Liquid · illustrative" else if (s[3] > 0) "Partially molten · illustrative" else "Solid · illustrative")
            Row { Checkbox(checked = overlay, onCheckedChange = { overlay = it; paint() }); Text("Illustrative heat overlay", Modifier.padding(top = 12.dp)) }
            OutlinedTextField(value = profile, onValueChange = { profile = it }, modifier = Modifier.fillMaxWidth(), minLines = 4,
                label = { Text("Profile points · [seconds, furnace °C]") }, supportingText = { Text("JSON pairs. Apply validates time order and limits in the shared planner.") })
            Row { TextButton(onClick = { scope.launch { rebuild() } }) { Text("Apply") }; TextButton(onClick = { profile = defaults; scope.launch { rebuild() } }) { Text("Reset") } }
            if (error.isNotEmpty()) Text(error, color = MaterialTheme.colorScheme.error)
            Text("Model: 24 s first-order lag, 1 s steps; linear fraction between solidus and liquidus. No latent heat, spatial gradients, flux, wetting or settling model.", style = MaterialTheme.typography.bodySmall)
        }
    }
}

@Composable
private fun ReflowGraph(samples: List<DoubleArray>, duration: Float, time: Float, liquid: Boolean) {
    Text(if (liquid) "Liquid fraction · %" else "Temperature °C · furnace dashed / board solid", style = MaterialTheme.typography.bodySmall)
    Canvas(Modifier.fillMaxWidth().height(120.dp).border(1.dp, Color.Gray)) {
        for (index in if (liquid) listOf(3) else listOf(1, 2)) {
            val path = Path()
            samples.forEachIndexed { n, row ->
                val x = (row[0] / maxOf(duration, 1f) * size.width).toFloat()
                val maximum = if (liquid) 1.0 else 350.0
                val y = ((1 - row[index] / maximum) * size.height).toFloat()
                if (n == 0) path.moveTo(x, y) else path.lineTo(x, y)
            }
            drawPath(path, if (index == 1) Color(0xffffa040) else Color.Cyan, style = Stroke(width = 2.dp.toPx(), pathEffect = if (index == 1) PathEffect.dashPathEffect(floatArrayOf(5.dp.toPx(), 4.dp.toPx())) else null))
        }
        val x = time / maxOf(duration, 1f) * size.width
        drawLine(Color.Gray, androidx.compose.ui.geometry.Offset(x, 0f), androidx.compose.ui.geometry.Offset(x, size.height))
    }
}
