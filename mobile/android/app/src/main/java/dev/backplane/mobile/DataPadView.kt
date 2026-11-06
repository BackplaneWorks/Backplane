package dev.backplane.mobile

import android.graphics.Bitmap
import android.graphics.DashPathEffect
import android.graphics.Paint
import android.graphics.Path
import android.graphics.Typeface
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.*
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlin.math.max

// The drawing pad on the phone: the composer's pen button opens it over the thread. The paper is DataPad's model drawn by `padPaint`, which also
// draws the attachment, so what is attached is what was drawn (the desktop's padview.rs; iOS's DataPadView.swift). Attach hands a JPEG to the composer's
// pictures, like one chosen from the gallery. Back or Cancel drops it; a tap beside the paper does not, so a stray touch loses no drawing.

private const val INK = 0x1f2328

private fun hint(t: PadTool) = when (t) {
    PadTool.Pen -> "Drag to draw"
    PadTool.Boxes -> "Drag to draw a box · drag its middle to move it, a side to resize · tap a side to type its length"
    PadTool.Eraser -> "Drag over what to erase"
}

/** The pad's picture on `c`, in paper pixels (the caller scales). `marks`: the selection and what is being drawn, for the screen; without, for the attachment. */
fun padPaint(c: android.graphics.Canvas, pad: DataPad, marks: Boolean) {
    val p = Paint(Paint.ANTI_ALIAS_FLAG)
    fun box(r: PadItem.Box, dashed: Boolean) {
        if (dashed) { p.style = Paint.Style.STROKE; p.strokeWidth = 1.5f; p.color = 0xff000000.toInt() or r.color; p.pathEffect = DashPathEffect(floatArrayOf(6f, 4f), 0f); c.drawRect(r.x, r.y, r.x + r.w, r.y + r.h, p); p.pathEffect = null; return }
        p.style = Paint.Style.FILL; p.color = 0x33000000 or r.color; c.drawRect(r.x, r.y, r.x + r.w, r.y + r.h, p)
        p.style = Paint.Style.STROKE; p.strokeWidth = 3f; p.color = 0xff000000.toInt() or r.color; c.drawRect(r.x + 1.5f, r.y + 1.5f, r.x + r.w - 1.5f, r.y + r.h - 1.5f, p)
        p.style = Paint.Style.FILL; p.textSize = 26f; p.typeface = Typeface.MONOSPACE; p.textAlign = Paint.Align.CENTER
        if (r.dimW.isNotEmpty()) c.drawText(r.dimW, r.x + r.w / 2, r.y - 6f, p)
        if (r.dimH.isNotEmpty()) { c.save(); c.rotate(-90f, r.x - 8f, r.y + r.h / 2); c.drawText(r.dimH, r.x - 8f, r.y + r.h / 2, p); c.restore() }
    }
    fun stroke(s: PadItem.Stroke) {
        p.color = 0xff000000.toInt() or s.color; p.strokeWidth = 3f; p.strokeCap = Paint.Cap.ROUND; p.strokeJoin = Paint.Join.ROUND
        if (s.pts.size == 1) { p.style = Paint.Style.FILL; c.drawCircle(s.pts[0].x, s.pts[0].y, 1.5f, p); return }
        p.style = Paint.Style.STROKE
        c.drawPath(Path().apply { moveTo(s.pts[0].x, s.pts[0].y); s.pts.drop(1).forEach { lineTo(it.x, it.y) } }, p)
    }
    pad.items.forEachIndexed { i, it ->
        when (it) { is PadItem.Box -> box(it, false); is PadItem.Stroke -> stroke(it) }
        if (marks && pad.picked == i) { val b = PadMath.bounds(it); box(PadItem.Box(b[0] - 4, b[1] - 4, b[2] + 8, b[3] + 8, INK), true) }
    }
    if (!marks) return
    pad.draft?.let { box(it, true) }
    pad.ink?.let { stroke(it) }
    if (pad.tool == PadTool.Eraser) pad.pointer?.let { p.style = Paint.Style.STROKE; p.strokeWidth = 1.5f; p.color = 0xff000000.toInt() or INK; c.drawCircle(it.x, it.y, pad.eraser, p) }
}

/** The attachment: the paper at its own size, white, as a JPEG. */
fun padPicture(pad: DataPad): ByteArray {
    val bmp = Bitmap.createBitmap(PAD_W.toInt(), PAD_H.toInt(), Bitmap.Config.ARGB_8888)
    val c = android.graphics.Canvas(bmp); c.drawColor(android.graphics.Color.WHITE); padPaint(c, pad, false)
    return java.io.ByteArrayOutputStream().also { bmp.compress(Bitmap.CompressFormat.JPEG, 92, it) }.toByteArray()
}

@Composable
fun DrawPadSheet(close: () -> Unit, attach: (ByteArray) -> Unit) {
    val pad = remember { DataPad() }
    var tick by remember { mutableIntStateOf(0) } // the model is plain; this is what tells Compose it changed
    var asking by remember { mutableStateOf(false) }
    val touched = { tick++ }
    Dialog(onDismissRequest = close, properties = DialogProperties(usePlatformDefaultWidth = false, dismissOnClickOutside = false)) {
        tick.let { } // read, so the sheet recomposes on each change
        Column(Modifier.fillMaxWidth().padding(10.dp).background(dcs.surfaceVariant).border(BorderStroke(1.dp, dcs.outlineVariant)).padding(10.dp).testTag("pad"), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("Draw", color = dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp, fontWeight = FontWeight.Bold)
            Text(hint(pad.tool), Modifier.testTag("pad.hint"), color = dfaint, fontFamily = BpMono, fontSize = 11.sp) // wraps: the Box tool's hint is long
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                listOf(PadTool.Pen to "Pen", PadTool.Boxes to "Box", PadTool.Eraser to "Eraser").forEach { (t, label) ->
                    DBtn(label, pad.tool == t, modifier = Modifier.testTag("pad.${label.lowercase()}")) { pad.setTool(t); touched() }
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                PAD_COLORS.forEachIndexed { i, col ->
                    val on = pad.next % PAD_COLORS.size == i
                    Box(Modifier.size(30.dp).border(BorderStroke(if (on) 3.dp else 1.dp, if (on) dcs.onSurface else dcs.outlineVariant)).padding(if (on) 3.dp else 1.dp)
                        .background(Color(0xff000000.toInt() or col)).clickable { pad.setColor(i); touched() }.testTag("pad.color.$i"))
                }
            }
            Canvas(Modifier.fillMaxWidth().aspectRatio(PAD_W / PAD_H).background(Color.White).border(BorderStroke(1.dp, dcs.outlineVariant)).testTag("pad.paper").semantics { contentDescription = "Drawing paper" }
                .pointerInput(Unit) {
                    val k = PAD_W / max(size.width, 1).toFloat()
                    // a side is a fingertip wide (about 14 dp) and so is the eraser, whatever the phone's size; never less than the desktop's
                    pad.edge = max(7f, 14.dp.toPx() * k); pad.eraser = max(12f, 14.dp.toPx() * k)
                    awaitEachGesture {
                        val d = awaitFirstDown(); d.consume()
                        pad.down(d.position.x * k, d.position.y * k); touched()
                        var ended = false
                        try {
                            while (true) {
                                val ch = awaitPointerEvent().changes.firstOrNull { it.id == d.id } ?: break
                                if (!ch.pressed) { ended = true; break }
                                ch.consume(); pad.moved(ch.position.x * k, ch.position.y * k); touched()
                            }
                        } finally {
                            if (ended) { if (pad.up()) asking = true } else pad.cancel()
                            touched()
                        }
                    }
                }) {
                tick.let { }
                drawIntoCanvas { c -> val n = c.nativeCanvas; n.save(); n.scale(size.width / PAD_W, size.width / PAD_W); padPaint(n, pad, true); n.restore() }
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                DBtn("Undo", false, pad.canUndo, Modifier.testTag("pad.undo")) { pad.undo(); touched() }
                DBtn(if (pad.picked != null) "Delete" else "Clear", false, !pad.isEmpty, Modifier.testTag("pad.delete")) { pad.delete(); touched() }
                Spacer(Modifier.weight(1f))
                DBtn("Cancel", false, modifier = Modifier.testTag("pad.cancel"), onClick = close)
                DBtn("Attach", true, !pad.isEmpty, Modifier.testTag("pad.attach")) { attach(padPicture(pad)); close() }
            }
        }
    }
    if (asking) TextPrompt("Length, e.g. 40 mm", pad.lengthText(), "Set", { asking = false; touched() }) { pad.commitLength(it); touched() }
}
