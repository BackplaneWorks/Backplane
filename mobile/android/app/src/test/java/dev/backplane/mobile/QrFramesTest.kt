package dev.backplane.mobile

import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

// The scanner's frame reader on frames made the way the camera hands them
// over: a Y plane whose rows may be wider than the image (rowStride)
class QrFramesTest {
    private val link = "backplane://pair?url=http%3A%2F%2F100.64.0.7%3A3787%2Fpair%23invite%3D0123456789abcdef0123456789abcdef"

    // a frame of width x height, rows rowStride bytes apart, with the code
    // drawn side pixels wide at (x, y); dark/light are its luminances
    private fun frame(width: Int, height: Int, rowStride: Int, side: Int, x: Int, y: Int, dark: Int = 30, light: Int = 210): ByteArray {
        val m = QRCodeWriter().encode(link, BarcodeFormat.QR_CODE, side, side,
            mapOf(EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M, EncodeHintType.MARGIN to 4))
        val out = ByteArray(rowStride * height) { light.toByte() }
        for (r in 0 until m.height) for (c in 0 until m.width)
            if (m.get(c, r) && y + r < height && x + c < width) out[(y + r) * rowStride + x + c] = dark.toByte()
        return out
    }

    @Test fun readsAPaddedHdFrame() {
        val f = frame(1280, 720, 1344, 360, 500, 180)
        assertEquals(link, QrFrames.read(f, 1280, 720, 1344))
    }

    @Test fun readsASmallFrameAsItIs() {
        val f = frame(640, 480, 640, 300, 170, 90)
        assertEquals(link, QrFrames.read(f, 640, 480, 640))
    }

    @Test fun readsALightCodeOnADarkScreen() {
        val f = frame(1280, 720, 1280, 400, 400, 160, dark = 220, light = 20)
        assertEquals(link, QrFrames.read(f, 1280, 720, 1280))
    }

    @Test fun readsABigFrameDownscaled() {
        val f = frame(1920, 1080, 1920, 600, 700, 240)
        assertEquals(link, QrFrames.read(f, 1920, 1080, 1920))
    }

    // what the 0.15 hub draws for Add a device: an invitation with its other addresses
    @Test fun readsAnInvitationWithRoutes() {
        val inv = "http://100.64.0.7:3787/pair#invite=0123456789abcdef0123456789abcdef&routes=%5B%22http%3A%2F%2F192.168.1.20%3A3787%22%5D"
        val m = QRCodeWriter().encode(inv, BarcodeFormat.QR_CODE, 400, 400, mapOf(EncodeHintType.MARGIN to 4))
        val f = ByteArray(1280 * 720) { 210.toByte() }
        for (r in 0 until m.height) for (c in 0 until m.width) if (m.get(c, r)) f[(160 + r) * 1280 + 440 + c] = 30
        assertEquals(inv, QrFrames.read(f, 1280, 720, 1280))
    }

    @Test fun aBrokenFrameIsNoCode() {
        assertNull(QrFrames.read(ByteArray(0), 0, 0, 0))
        assertNull(QrFrames.read(ByteArray(640 * 480), 640, 480, 320))
    }

    @Test fun nothingInAnEmptyFrame() {
        assertNull(QrFrames.read(ByteArray(640 * 480) { 128.toByte() }, 640, 480, 640))
    }
}
