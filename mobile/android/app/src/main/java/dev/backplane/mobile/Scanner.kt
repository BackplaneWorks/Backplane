package dev.backplane.mobile

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.Settings
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.Camera
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.FlashlightOff
import androidx.compose.material.icons.filled.FlashlightOn
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.LuminanceSource
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

// A pairing code read by the app's own camera (CameraX and ZXing): no Google Play services.
// start() asks for the camera at the tap (Android's own prompt, nothing before it) and opens the scanner the moment it is allowed.
// note: why the camera is not there (refused, no camera, it did not start); the pairing screen shows it beside its other ways to pair
// (a photo of the code, the link pasted). blocked: refused for good, only Settings can allow it now.
class PairScanner(val open: Boolean, val note: String, val blocked: Boolean, val start: () -> Unit, val close: () -> Unit, val failed: () -> Unit)

@Composable
fun pairScanner(): PairScanner {
    val ctx = LocalContext.current
    var open by remember { mutableStateOf(false) }
    var note by remember { mutableStateOf("") }
    var blocked by remember { mutableStateOf(false) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok ->
        if (ok) { note = ""; blocked = false; open = true }
        else {
            note = Msg.native_pair_scanner_denied
            // refused with "don't ask again" (or twice): Android shows no prompt any more
            blocked = ctx.activity()?.shouldShowRequestPermissionRationale(Manifest.permission.CAMERA) == false
        }
    }
    // back from Settings with the camera allowed: the note goes
    val owner = LocalLifecycleOwner.current
    DisposableEffect(owner) {
        val o = LifecycleEventObserver { _, e -> if (e == Lifecycle.Event.ON_RESUME && blocked && ctx.cameraAllowed()) { blocked = false; note = "" } }
        owner.lifecycle.addObserver(o)
        onDispose { owner.lifecycle.removeObserver(o) }
    }
    return PairScanner(open, note, blocked,
        start = {
            when {
                !ctx.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY) -> { note = Msg.native_pair_scanner_unavailable; blocked = false }
                ctx.cameraAllowed() -> { note = ""; blocked = false; open = true }
                else -> ask.launch(Manifest.permission.CAMERA)
            }
        },
        close = { open = false },
        failed = { open = false; blocked = false; note = Msg.native_pair_scanner_unavailable })
}

/** Under Scan: why the camera is not there, and (refused for good) the way to Settings. */
@Composable
fun ScanNote(scan: PairScanner) {
    if (scan.note.isEmpty()) return
    val ctx = LocalContext.current
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(scan.note, Modifier.testTag("pair.scan.note"), color = dcs.onSurfaceVariant, fontSize = 13.sp)
        if (scan.blocked) DBtn(Msg.native_settings_system_open, false, modifier = Modifier.testTag("pair.scan.settings")) {
            ctx.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", ctx.packageName, null)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        }
    }
}

/** A code the hub draws: an invitation (`…/pair#invite=…&routes=…`, `…/pair#code=…`) or a pairing link with its token, either inside this app's
 *  `backplane://pair?url=…`. A link with a host but no token is not taken: the camera sees any code around (a poster's web address would become a "hub"). */
fun pairingCode(text: String): Boolean = Pairing.invitation(text) != null || (Pairing.key(text) != null && Pairing.token(text) != null)

private fun Context.cameraAllowed() = ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED

private tailrec fun Context.activity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.activity()
    else -> null
}

/**
 * The scanner, over the pairing screen: the back camera, a square to aim with, one hint line, Close and (with a flash) Light.
 * Frames are read on a worker thread; the first pairing code goes to [found]; a code that is not one says so for a moment and the camera keeps looking.
 * A camera that does not start calls [failed] (the pairing screen says so and keeps its photo and paste).
 */
@Composable
fun ScannerView(close: () -> Unit, failed: () -> Unit, found: (String) -> Unit) {
    val ctx = LocalContext.current
    val owner = LocalLifecycleOwner.current
    BackHandler(onBack = close)
    var camera by remember { mutableStateOf<Camera?>(null) }
    var light by remember { mutableStateOf(false) }
    // when a code that is not a pairing link was last seen (0: none lately)
    var wrong by remember { mutableStateOf(0L) }
    val view = remember { PreviewView(ctx).apply {
        implementationMode = PreviewView.ImplementationMode.COMPATIBLE
        scaleType = PreviewView.ScaleType.FILL_CENTER
    } }
    val onFound by rememberUpdatedState(found); val onFailed by rememberUpdatedState(failed)
    DisposableEffect(owner) {
        val worker = Executors.newSingleThreadExecutor()
        val main = ContextCompat.getMainExecutor(ctx)
        val done = AtomicBoolean(false)
        val future = ProcessCameraProvider.getInstance(ctx)
        var bound: ProcessCameraProvider? = null
        val preview = Preview.Builder().build()
        // about 720p is plenty for a code held up to the phone, and quick to read
        val analysis = ImageAnalysis.Builder()
            .setResolutionSelector(ResolutionSelector.Builder().setResolutionStrategy(ResolutionStrategy(android.util.Size(1280, 720),
                ResolutionStrategy.FALLBACK_RULE_CLOSEST_LOWER_THEN_HIGHER)).build())
            .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST).build()
        var y = ByteArray(0)
        analysis.setAnalyzer(worker) { img ->
            img.use {
                if (done.get()) return@use
                val plane = it.planes[0]
                val buf = plane.buffer
                val n = buf.remaining()
                if (y.size < n) y = ByteArray(n)
                buf.get(y, 0, n)
                val text = QrFrames.read(y, it.width, it.height, plane.rowStride) ?: return@use
                if (pairingCode(text)) { if (done.compareAndSet(false, true)) main.execute { onFound(text) } }
                else main.execute { wrong = System.currentTimeMillis() }
            }
        }
        future.addListener({
            if (!done.get()) runCatching {
                val p = future.get()
                val select = if (p.hasCamera(CameraSelector.DEFAULT_BACK_CAMERA)) CameraSelector.DEFAULT_BACK_CAMERA else CameraSelector.DEFAULT_FRONT_CAMERA
                preview.setSurfaceProvider(view.surfaceProvider)
                p.unbind(preview, analysis)
                camera = p.bindToLifecycle(owner, select, preview, analysis)
                bound = p
            }.onFailure { if (done.compareAndSet(false, true)) onFailed() }
        }, main)
        onDispose {
            done.set(true)
            bound?.unbind(preview, analysis)
            analysis.clearAnalyzer()
            worker.shutdown()
        }
    }
    LaunchedEffect(wrong) { if (wrong != 0L) { kotlinx.coroutines.delay(2000); wrong = 0L } }
    Box(Modifier.fillMaxSize().background(Color.Black).testTag("pair.scanner")) {
        AndroidView({ view }, Modifier.fillMaxSize())
        // the square to aim with; the camera reads the whole frame
        Box(Modifier.align(Alignment.Center).size(260.dp).border(BorderStroke(3.dp, Color.White.copy(alpha = 0.9f))))
        Row(Modifier.fillMaxWidth().background(Color.Black.copy(alpha = 0.55f)).padding(4.dp), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            IconButton(close, Modifier.testTag("pair.scanner.close")) { Icon(Icons.Filled.Close, Msg.common_close, tint = Color.White) }
            Text(Msg.native_pair_scanner_title, color = Color.White, fontFamily = BpMono, fontSize = 14.sp, fontWeight = FontWeight.Bold)
            val c = camera
            if (c != null && c.cameraInfo.hasFlashUnit()) IconButton(onClick = { light = !light; c.cameraControl.enableTorch(light) }, modifier = Modifier.testTag("pair.scanner.light")) {
                Icon(if (light) Icons.Filled.FlashlightOn else Icons.Filled.FlashlightOff, Msg.native_pair_scanner_light, tint = Color.White)
            } else Box(Modifier.size(48.dp))
        }
        val bad = wrong != 0L
        Text(if (bad) Msg.native_pair_scanner_invalid else Msg.native_pair_scanner_instructions,
            Modifier.align(Alignment.BottomCenter).padding(horizontal = 24.dp, vertical = 32.dp).background(Color.Black.copy(alpha = 0.7f))
                .padding(horizontal = 14.dp, vertical = 10.dp).testTag(if (bad) "pair.scanner.invalid" else "pair.scanner.hint"),
            color = if (bad) Color(0xFFFFB4AB) else Color.White, textAlign = TextAlign.Center, fontFamily = BpMono, fontSize = 12.sp)
    }
}

// A QR code in a camera frame's luminance (the Y plane of YUV_420_888): the frame is read at most maxSide pixels across
// (every other or third pixel of a big frame), and inverted too, for a light code on a dark screen. Pure: QrFramesTest runs it on the JVM.
object QrFrames {
    private val hints = mapOf(DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE))

    fun read(y: ByteArray, width: Int, height: Int, rowStride: Int, maxSide: Int = 960): String? {
        if (width <= 0 || height <= 0 || rowStride < width) return null
        val step = maxOf(1, (maxOf(width, height) + maxSide - 1) / maxSide)
        val w = width / step
        val h = height / step
        val px: ByteArray
        if (step == 1 && rowStride == width) px = y
        else {
            px = ByteArray(w * h)
            for (r in 0 until h) {
                val from = r * step * rowStride
                val to = r * w
                if (step == 1) System.arraycopy(y, from, px, to, w)
                else for (c in 0 until w) px[to + c] = y[from + c * step]
            }
        }
        val src = PlanarYUVLuminanceSource(px, w, h, 0, 0, w, h, false)
        return decode(src) ?: decode(src.invert())
    }

    private fun decode(src: LuminanceSource): String? = try {
        QRCodeReader().decode(BinaryBitmap(HybridBinarizer(src)), hints).text
    } catch (_: Exception) { null }
}
