package dev.backplane.mobile

import kotlin.math.roundToInt

// The Browser tab's input, pure (test/mobile/browser_input_cases.json holds this and iOS's BrowserInput.swift to the same answers):
// the key field's change as the type/press ops sent to the page, and a drag on the picture as the page's scroll.
internal object BrowserInput {
    /** What the key field holds between keys: a Backspace on it is a change the field reports. */
    const val SENTINEL = "​"
    /** The key buttons under the field: label to the key's name for `press`. */
    val keyButtons = listOf("⇥" to "Tab", "←" to "ArrowLeft", "↑" to "ArrowUp", "↓" to "ArrowDown", "→" to "ArrowRight", "⌫" to "Backspace", "⏎" to "Enter")
    /** The ops for the field going from [old] to [text], in order: "type" to text or "press" to key. Compared by code point: what is kept at both ends
     *  is not sent again; each removed character other than the sentinel is a Backspace, the sentinel removed alone is one. */
    fun keys(old: String, text: String): List<Pair<String, String>> {
        val a = old.codePoints().toArray(); val b = text.codePoints().toArray(); val s0 = SENTINEL.codePointAt(0)
        var p = 0; while (p < a.size && p < b.size && a[p] == b[p]) p++
        var s = 0; while (s < a.size - p && s < b.size - p && a[a.size - 1 - s] == b[b.size - 1 - s]) s++
        val gone = a.copyOfRange(p, a.size - s); val added = b.copyOfRange(p, b.size - s)
        val out = mutableListOf<Pair<String, String>>()
        val n = gone.count { it != s0 }
        repeat(if (n > 0) n else if (gone.contains(s0)) 1 else 0) { out += "press" to "Backspace" }
        val run = StringBuilder()
        fun flush() { if (run.isNotEmpty()) { out += "type" to run.toString(); run.clear() } }
        var i = 0
        while (i < added.size) {
            val c = added[i]
            when (c) {
                s0 -> {}
                '\r'.code, '\n'.code -> { flush(); out += "press" to "Enter"; if (c == '\r'.code && i + 1 < added.size && added[i + 1] == '\n'.code) i++ }
                '\t'.code -> { flush(); out += "press" to "Tab" }
                else -> run.appendCodePoint(c)
            }
            i++
        }
        flush()
        return out
    }
    /** What the field holds after [text]: the sentinel again when it is gone, after an Enter or a Tab, or past 64 characters after it. */
    fun next(text: String): String =
        if (!text.contains(SENTINEL) || text.any { it == '\n' || it == '\r' || it == '\t' } || text.codePointCount(0, text.length) > 65) SENTINEL else text
    /** A drag of (dx, dy) on a picture [image] pixels wide shown [shown] pixels wide: the page scrolls the other way, in its own pixels. */
    fun scroll(dx: Double, dy: Double, image: Double, shown: Double): Pair<Int, Int> {
        if (shown <= 0 || image <= 0) return 0 to 0
        val s = image / shown
        return (-dx * s).roundHalfAway() to (-dy * s).roundHalfAway()
    }
    private fun Double.roundHalfAway(): Int = if (this < 0) -((-this).roundToInt()) else roundToInt()
    /** A drag sends its scroll in steps of at least this many page pixels (and the rest when the finger lifts). */
    const val STEP = 48
}
