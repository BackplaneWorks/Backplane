package dev.backplane.mobile

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

// docs/VIEWER-BACKGROUND.md: the palette the 3D viewers draw for each setting
class ViewerBgTest {
    private val light = 0xf5f6f8 to 0xe1e4e9
    private val dark = 0x4d535e to 0x353a43

    @Test fun autoFollowsTheTheme() {
        assertEquals(light, ViewerBg.resolve("auto", "", dark = false))
        assertEquals(dark, ViewerBg.resolve("auto", "#123456", dark = true))
    }

    @Test fun lightAndDarkIgnoreTheTheme() {
        assertEquals(light, ViewerBg.resolve("light", "", dark = true))
        assertEquals(dark, ViewerBg.resolve("dark", "", dark = false))
    }

    @Test fun customBottomIsEachChannelTimes088Rounded() {
        assertEquals(0xffffff to 0xe0e0e0, ViewerBg.resolve("custom", "#ffffff", dark = true))
        // 31*.88=27.28 -> 27, 111*.88=97.68 -> 98, 67*.88=58.96 -> 59
        assertEquals(0x1f6f43 to 0x1b623b, ViewerBg.resolve("custom", "#1F6F43", dark = false))
        assertEquals(0x7f7f7f to 0x707070, ViewerBg.resolve("custom", "7f7f7f", dark = false))
        assertEquals(0x000000 to 0x000000, ViewerBg.resolve("custom", "#000000", dark = false))
        for (c in 0..255) assertEquals(Math.round(c * 0.88).toInt(), ViewerBg.bottomOf(c shl 8) shr 8)
    }

    @Test fun badCustomFallsBackToAuto() {
        assertEquals(light, ViewerBg.resolve("custom", "", dark = false))
        assertEquals(dark, ViewerBg.resolve("custom", "#12345g", dark = true))
        assertNull(ViewerBg.parse("#12345"))
        assertEquals("#0a0b0c", ViewerBg.hex(0x0a0b0c))
    }

    @Test fun floatsAreTopThenBottom() {
        assertArrayEquals(floatArrayOf(1f, 0f, 128 / 255f, 0f, 1f, 0f), ViewerBg.floats(0xff0080 to 0x00ff00), 1e-6f)
    }
}
