package dev.backplane.mobile

import android.content.Context
import android.widget.Toast
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning

// A QR code as the hub drew it (core/qr.bend: rows, '1' dark), on white
// with four modules of quiet zone
@Composable
fun QrCode(rows: List<String>, modifier: Modifier = Modifier) {
    Canvas(modifier.widthIn(max = 320.dp).aspectRatio(1f)) {
        val n = rows.size + 8
        val m = minOf(size.width, size.height) / n
        drawRect(Color.White, Offset.Zero, Size(m * n, m * n))
        rows.forEachIndexed { y, row ->
            var x = 0
            while (x < row.length) {
                if (row[x] != '1') { x++; continue }
                val start = x
                while (x < row.length && row[x] == '1') x++
                drawRect(Color.Black, Offset((start + 4) * m, (y + 4) * m), Size((x - start) * m + 0.5f, m + 0.5f))
            }
        }
    }
}

// The camera reading a pairing code: Google Play services' scanner (no
// camera permission of our own); the code goes to found when it is a link
// Pairing understands
fun scanPairing(ctx: Context, tag: String, found: (String) -> Unit) {
    val opts = GmsBarcodeScannerOptions.Builder().setBarcodeFormats(Barcode.FORMAT_QR_CODE).build()
    GmsBarcodeScanning.getClient(ctx, opts).startScan()
        .addOnSuccessListener { b ->
            val s = b.rawValue.orEmpty()
            if (Pairing.key(s) != null) found(s)
            else Toast.makeText(ctx, NativeMessages.native_pair_scanner_invalid(tag), Toast.LENGTH_SHORT).show()
        }
        .addOnFailureListener { Toast.makeText(ctx, NativeMessages.native_pair_scanner_unavailable(tag), Toast.LENGTH_LONG).show() }
}

// A photo or screenshot of a pairing code (the gallery, Files): read on the
// phone with ZXing, for phones without Google Play services' scanner too
fun pairingFromImage(ctx: Context, uri: android.net.Uri): String? = runCatching {
    val src = ctx.contentResolver.openInputStream(uri)?.use { android.graphics.BitmapFactory.decodeStream(it) } ?: return null
    // a big photo is read at about 1600 px across (enough for a code, and quick)
    val scale = maxOf(1, maxOf(src.width, src.height) / 1600)
    val bmp = if (scale > 1) android.graphics.Bitmap.createScaledBitmap(src, src.width / scale, src.height / scale, true) else src
    val px = IntArray(bmp.width * bmp.height)
    bmp.getPixels(px, 0, bmp.width, 0, 0, bmp.width, bmp.height)
    val bits = com.google.zxing.BinaryBitmap(com.google.zxing.common.HybridBinarizer(com.google.zxing.RGBLuminanceSource(bmp.width, bmp.height, px)))
    val hints = mapOf(com.google.zxing.DecodeHintType.TRY_HARDER to true, com.google.zxing.DecodeHintType.POSSIBLE_FORMATS to listOf(com.google.zxing.BarcodeFormat.QR_CODE))
    com.google.zxing.qrcode.QRCodeReader().decode(bits, hints).text
}.getOrNull()?.takeIf { Pairing.key(it) != null || Pairing.invitation(it) != null }
