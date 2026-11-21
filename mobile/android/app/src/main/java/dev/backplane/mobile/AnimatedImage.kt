package dev.backplane.mobile

import android.graphics.BitmapFactory
import android.graphics.ImageDecoder
import android.graphics.drawable.Animatable
import android.graphics.drawable.Drawable
import android.os.Build
import android.widget.ImageView
import androidx.compose.foundation.Image
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.viewinterop.AndroidView
import java.nio.ByteBuffer

/** Format helpers for GIF detection and upload metadata (unit-tested on JVM). */
object GifFormat {
    const val MAX_UPLOAD_BYTES = 8 * 1024 * 1024

    fun isGif(bytes: ByteArray): Boolean {
        if (bytes.size < 6) return false
        return bytes[0] == 'G'.code.toByte() &&
            bytes[1] == 'I'.code.toByte() &&
            bytes[2] == 'F'.code.toByte() &&
            bytes[3] == '8'.code.toByte() &&
            (bytes[4] == '7'.code.toByte() || bytes[4] == '9'.code.toByte()) &&
            bytes[5] == 'a'.code.toByte()
    }

    fun mimeOf(bytes: ByteArray): String = if (isGif(bytes)) "image/gif" else "image/jpeg"
    fun extOf(bytes: ByteArray): String = if (isGif(bytes)) "gif" else "jpg"
}

/**
 * Renders raster image bytes, animating GIFs on API 28+ via ImageDecoder / AnimatedImageDrawable
 * (stopping playback when disposed) and falling back to BitmapFactory for static images or API 26-27.
 */
@Composable
internal fun AnimatedPic(
    bytes: ByteArray,
    modifier: Modifier = Modifier,
    contentScale: ContentScale = ContentScale.Fit,
    maxSidePx: Int = 0,
) {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P && GifFormat.isGif(bytes)) {
        val drawable: Drawable? = remember(bytes, maxSidePx) {
            runCatching {
                val src = ImageDecoder.createSource(ByteBuffer.wrap(bytes))
                ImageDecoder.decodeDrawable(src) { decoder, info, _ ->
                    if (maxSidePx > 0) {
                        val w = info.size.width
                        val h = info.size.height
                        val longest = maxOf(w, h)
                        if (longest > maxSidePx) {
                            val f = maxSidePx.toFloat() / longest
                            decoder.setTargetSize((w * f).toInt().coerceAtLeast(1), (h * f).toInt().coerceAtLeast(1))
                        }
                    }
                }
            }.getOrNull()
        }
        if (drawable != null) {
            DisposableEffect(drawable) {
                (drawable as? Animatable)?.start()
                onDispose { (drawable as? Animatable)?.stop() }
            }
            val scaleType = when (contentScale) {
                ContentScale.Crop -> ImageView.ScaleType.CENTER_CROP
                ContentScale.FillBounds -> ImageView.ScaleType.FIT_XY
                else -> ImageView.ScaleType.FIT_CENTER
            }
            AndroidView(
                factory = { ctx ->
                    ImageView(ctx).apply {
                        adjustViewBounds = true
                        this.scaleType = scaleType
                        setImageDrawable(drawable)
                        (drawable as? Animatable)?.start()
                    }
                },
                update = { iv ->
                    if (iv.drawable !== drawable) iv.setImageDrawable(drawable)
                    iv.scaleType = scaleType
                    (drawable as? Animatable)?.start()
                },
                modifier = modifier,
            )
            return
        }
    }
    val bmp = remember(bytes) { BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap() }
    if (bmp != null) Image(bmp, null, modifier, contentScale = contentScale)
}
