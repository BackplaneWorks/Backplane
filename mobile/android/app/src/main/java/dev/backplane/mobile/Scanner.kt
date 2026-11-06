package dev.backplane.mobile

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Bundle
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.Camera
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.FlashlightOff
import androidx.compose.material.icons.filled.FlashlightOn
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
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

// A pairing code read by the app's own camera: no Google Play services.
// start() asks for the camera the first time (Android's own prompt, nothing
// before it) and opens the scanner (ScannerActivity) the moment it is
// allowed; the scanner closes on the first pairing code it reads and gives
// it to found.
// blocked: the camera was refused for good (only Settings can allow it now);
// the screen shows ScanBlocked and keeps its other ways to pair.
class PairScanner(val start: () -> Unit, val blocked: Boolean)

@Composable
fun pairScanner(tag: String, found: (String) -> Unit): PairScanner {
    val ctx = LocalContext.current
    var blocked by remember { mutableStateOf(false) }
    val scan = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { r ->
        r.data?.getStringExtra("code")?.let(found)
    }
    fun open() = scan.launch(Intent(ctx, ScannerActivity::class.java).putExtra("tag", tag))
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok ->
        if (ok) { blocked = false; open() }
        // refused with "don't ask again" (or twice): Android shows no prompt any more
        else blocked = ctx.activity()?.shouldShowRequestPermissionRationale(Manifest.permission.CAMERA) == false
    }
    // back from Settings with the camera allowed: the line goes
    val owner = LocalLifecycleOwner.current
    DisposableEffect(owner) {
        val o = LifecycleEventObserver { _, e -> if (e == Lifecycle.Event.ON_RESUME && ctx.hasCamera()) blocked = false }
        owner.lifecycle.addObserver(o)
        onDispose { owner.lifecycle.removeObserver(o) }
    }
    return PairScanner(start = { if (ctx.hasCamera()) open() else ask.launch(Manifest.permission.CAMERA) }, blocked = blocked)
}

// the one line under Scan when the camera was refused for good
@Composable
fun ScanBlocked(tag: String) {
    val ctx = LocalContext.current
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(NativeMessages.native_pair_scanner_denied(tag), style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        TextButton(onClick = {
            ctx.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", ctx.packageName, null))
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        }) { Text(NativeMessages.native_settings_system_open(tag)) }
    }
}

// a code the hub draws: an invitation (Devices, Add a device) or a pairing
// link with its token (Settings, QR code), either in backplane://pair?url=…;
// Pairing.key alone takes any text with a host, and the camera sees any
// code around (a poster's web address would become a "hub")
fun pairingCode(text: String): Boolean = Pairing.invitation(text) != null ||
    (Pairing.key(text) != null && Pairing.socket(text)?.let { Uri.parse(it).getQueryParameter("token") } != null)

private fun Context.hasCamera() = ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED

private tailrec fun Context.activity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.activity()
    else -> null
}

// The scanner, an activity of its own: a dialog's window on Android 15
// (edge to edge enforced) sat under the status bar and ran off the bottom.
// The code comes back as the result's "code".
class ScannerActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val tag = intent.getStringExtra("tag") ?: "en"
        setContent {
            Theme("dark") {
                ScannerView(tag, close = { finish() }) { setResult(RESULT_OK, Intent().putExtra("code", it)); finish() }
            }
        }
    }
}

// the scanner: the back camera full screen, a square to aim with, one hint
// line, Close and (with a flash) the light. Frames are read on a worker
// thread; a code that is not a pairing link says so for a moment and the
// camera keeps looking.
@Composable
private fun ScannerView(tag: String, close: () -> Unit, found: (String) -> Unit) {
    val ctx = LocalContext.current
    val owner = LocalLifecycleOwner.current
    var camera by remember { mutableStateOf<Camera?>(null) }
    var light by remember { mutableStateOf(false) }
    var failed by remember { mutableStateOf(false) }
    // when a code that is not a pairing link was last seen (0: none lately)
    var wrong by remember { mutableStateOf(0L) }
    val view = remember { PreviewView(ctx).apply {
        implementationMode = PreviewView.ImplementationMode.COMPATIBLE
        scaleType = PreviewView.ScaleType.FILL_CENTER
    } }
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
                if (y.size < buf.remaining()) y = ByteArray(buf.remaining())
                val n = buf.remaining()
                buf.get(y, 0, n)
                val text = QrFrames.read(y, it.width, it.height, plane.rowStride) ?: return@use
                if (pairingCode(text)) {
                    if (done.compareAndSet(false, true)) main.execute { found(text) }
                } else main.execute { wrong = System.currentTimeMillis() }
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
            }.onFailure { failed = true }
        }, main)
        onDispose {
            done.set(true)
            bound?.unbind(preview, analysis)
            analysis.clearAnalyzer()
            worker.shutdown()
        }
    }
    LaunchedEffect(wrong) {
        if (wrong != 0L) { kotlinx.coroutines.delay(2000); wrong = 0L }
    }
    Box(Modifier.fillMaxSize().background(Color.Black)) {
        AndroidView({ view }, Modifier.fillMaxSize())
        // the square to aim with; the camera reads the whole frame
        Box(Modifier.align(Alignment.Center).size(260.dp).border(3.dp, Color.White.copy(alpha = 0.9f), RoundedCornerShape(24.dp)))
        Row(Modifier.fillMaxWidth().statusBarsPadding().padding(8.dp), horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically) {
            IconButton(close) { Icon(Icons.Filled.Close, NativeMessages.common_close(tag), tint = Color.White) }
            Text(NativeMessages.native_pair_scanner_title(tag), color = Color.White, style = MaterialTheme.typography.titleMedium)
            val c = camera
            if (c != null && c.cameraInfo.hasFlashUnit()) IconButton(onClick = { light = !light; c.cameraControl.enableTorch(light) }) {
                Icon(if (light) Icons.Filled.FlashlightOn else Icons.Filled.FlashlightOff, NativeMessages.native_pair_scanner_light(tag), tint = Color.White)
            } else Box(Modifier.size(48.dp))
        }
        val note = when {
            failed -> NativeMessages.native_pair_scanner_unavailable(tag)
            wrong != 0L -> NativeMessages.native_pair_scanner_invalid(tag)
            else -> NativeMessages.native_pair_scanner_instructions(tag)
        }
        Text(note, Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(horizontal = 24.dp, vertical = 32.dp)
            .background(Color.Black.copy(alpha = 0.55f), RoundedCornerShape(50)).padding(horizontal = 16.dp, vertical = 10.dp),
            color = if (wrong != 0L && !failed) Color(0xFFFFB4AB) else Color.White, textAlign = TextAlign.Center,
            style = MaterialTheme.typography.bodyMedium)
    }
}

// A QR code in a camera frame's luminance (the Y plane of YUV_420_888):
// the frame is read at most maxSide pixels across (every other or third
// pixel of a big frame), and inverted too, for a light code on a dark screen
object QrFrames {
    private val hints = mapOf(DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE))

    fun read(y: ByteArray, width: Int, height: Int, rowStride: Int, maxSide: Int = 960): String? {
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
