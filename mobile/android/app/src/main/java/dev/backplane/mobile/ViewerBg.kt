package dev.backplane.mobile

import kotlin.math.roundToInt

// The 3D viewers' background (docs/VIEWER-BACKGROUND.md): one setting for the Mech viewer and the 3D board viewer, kept on
// this device. A vertical gradient, top to bottom, sRGB bytes 0xrrggbb; the renderer mixes the two ends in linear light.
object ViewerBg {
    const val LIGHT_TOP = 0xf5f6f8
    const val LIGHT_BOTTOM = 0xe1e4e9
    const val DARK_TOP = 0x4d535e
    const val DARK_BOTTOM = 0x353a43
    val MODES = listOf("auto", "light", "dark", "custom")

    /** `#rrggbb` (or `rrggbb`) to 0xrrggbb; null when it is not one. */
    fun parse(hex: String): Int? {
        val h = hex.trim().removePrefix("#")
        if (h.length != 6 || h.any { it !in '0'..'9' && it.lowercaseChar() !in 'a'..'f' }) return null
        return h.toInt(16)
    }
    fun hex(c: Int) = "#%06x".format(c and 0xFFFFFF)

    /** A custom colour's bottom: each channel × 0.88, rounded. */
    fun bottomOf(c: Int): Int {
        fun ch(s: Int) = (((c shr s) and 255) * 0.88).roundToInt().coerceIn(0, 255) shl s
        return ch(16) or ch(8) or ch(0)
    }

    /** (top, bottom) for the setting: auto follows the app's light or dark; a custom colour that does not parse falls back to auto. */
    fun resolve(mode: String, custom: String, dark: Boolean): Pair<Int, Int> {
        val c = if (mode == "custom") parse(custom) else null
        return when {
            c != null -> c to bottomOf(c)
            mode == "light" -> LIGHT_TOP to LIGHT_BOTTOM
            mode == "dark" -> DARK_TOP to DARK_BOTTOM
            dark -> DARK_TOP to DARK_BOTTOM
            else -> LIGHT_TOP to LIGHT_BOTTOM
        }
    }

    /** The two ends as floats 0..1 (sRGB), top then bottom: what PlotRenderer.backdrop takes. */
    fun floats(g: Pair<Int, Int>) = FloatArray(6) { i -> val c = if (i < 3) g.first else g.second; ((c shr (16 - 8 * (i % 3))) and 255) / 255f }

    // the custom colour's swatches, the desktop's (Slate, Graphite, Sand, Sky, Mint, Rose, Plum, Navy); the first is the default
    val SWATCHES = listOf(0x8a94a6, 0x5a5f66, 0xd9cfb8, 0xa9c7e8, 0xa8d8c0, 0xe2b4bd, 0x6b5b7b, 0x2f4468)
}
