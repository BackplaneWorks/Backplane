package dev.backplane.mobile

import android.provider.Settings
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.DesktopWindows
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.MotionDurationScale
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.PI
import kotlin.math.sin

data class BubMove(val id: String, val from: String, val to: String, val rig: List<CatPart>, val duration: Int, val expires: Long, val title: String = "Bub’s moving")

fun parseBubMove(o: JSONObject) = BubMove(o.optString("id"), o.optString("from"), o.optString("to"),
    catRig(o.optJSONArray("rig") ?: JSONArray()), o.optInt("duration", 2000), o.optLong("expires"), o.optString("title", "Bub’s moving"))

@Composable
fun BubMoveOverlay(move: BubMove?) {
    var seen by remember { mutableStateOf("") }
    var active by remember { mutableStateOf<BubMove?>(null) }
    LaunchedEffect(move?.id) {
        if (move != null && move.id != seen) {
            seen = move.id
            if (move.expires > System.currentTimeMillis()) active = move
        }
    }
    LaunchedEffect(active?.id) {
        val event = active ?: return@LaunchedEffect
        delay((event.expires - System.currentTimeMillis()).coerceIn(0L, event.duration.toLong()))
        if (active?.id == event.id) active = null
    }
    Box(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top + WindowInsetsSides.Horizontal)).padding(top = 12.dp),
        contentAlignment = Alignment.TopCenter) {
        active?.let { event -> key(event.id) { BubMoveCard(event) } }
    }
}

@Composable
private fun BubMoveMachine(name: String) {
    Column(Modifier.width(88.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Icon(Icons.Default.DesktopWindows, contentDescription = null, modifier = Modifier.size(32.dp, 28.dp))
        Text(name, fontSize = 11.sp, lineHeight = 14.sp, textAlign = TextAlign.Center, maxLines = 2, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun BubMoveCard(move: BubMove) {
    val context = LocalContext.current
    val reduced = Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
    val progress = remember { Animatable(0f) }
    LaunchedEffect(move.id) {
        // Keep the soft fade visible even when Android disables motion;
        // reduced motion changes only the cat's static placement below.
        withContext(object : MotionDurationScale { override val scaleFactor = 1f }) {
            progress.animateTo(1f, tween(move.duration, easing = LinearEasing))
        }
    }
    // Read animated values only inside the layer blocks: no per-frame layout
    // or app-state updates, no pointer input, focus targets, or modal Surface.
    Column(Modifier.graphicsLayer {
        val p = progress.value
        alpha = when { p < 0.12f -> p / 0.12f; p > 0.85f -> (1f - p) / 0.15f; else -> 1f }
    }.background(MaterialTheme.colorScheme.surface).border(1.dp, MaterialTheme.colorScheme.outlineVariant)
        .padding(horizontal = 16.dp, vertical = 12.dp).clearAndSetSemantics {
            contentDescription = "${move.title}: ${move.from} to ${move.to}"
            liveRegion = LiveRegionMode.Polite
        }, horizontalAlignment = Alignment.CenterHorizontally) {
        Text(move.title, fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis)
        Box(Modifier.width(248.dp).height(96.dp)) {
            Row(Modifier.width(248.dp).padding(top = 36.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                BubMoveMachine(move.from)
                BubMoveMachine(move.to)
            }
            CatView(move.rig, 48.dp, Modifier.graphicsLayer {
                val p = ((progress.value - 0.15f) / 0.6f).coerceIn(0f, 1f)
                translationX = (20f + if (reduced) 160f else 160f * p) * density
                translationY = (10f - if (reduced) 0f else 20f * sin(PI.toFloat() * p)) * density
            })
        }
    }
}
