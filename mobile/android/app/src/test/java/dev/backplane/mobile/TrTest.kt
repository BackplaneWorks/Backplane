package dev.backplane.mobile

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class TrTest {
    @After fun reset() { Tr.fixed = null }
    private fun r(msg: String, l: String = "en", vararg a: Pair<String, Any>) = Tr.render(Icu.parse(msg), a.toMap(), l)

    @Test fun icuSubset() {
        assertEquals("Delete notes.md?", r("Delete {title}?", "en", "title" to "notes.md"))
        assertEquals("1,234,567 rows", r("{n, number} rows", "en", "n" to 1234567))
        val p = "{n, plural, =0 {No files changed} one {# file changed} other {# files changed}}"
        assertEquals("No files changed", r(p, "en", "n" to 0)); assertEquals("1 file changed", r(p, "en", "n" to 1)); assertEquals("2,000 files changed", r(p, "en", "n" to 2000))
        // Japanese has no "one": 1 reads as other
        assertEquals("1 件", r("{n, plural, one {# 個} other {# 件}}", "ja", "n" to 1))
        val s = "{who, select, you {You} agent {Agent} other {Someone}}"
        assertEquals("You", r(s, "en", "who" to "you")); assertEquals("Someone", r(s, "en", "who" to "bob")); assertEquals("Someone", r(s, "en", "who" to "other"))
        // # is the nearest plural's number, also inside a select in it; outside a plural it is literal
        assertEquals("3 of them, #1", r("{n, plural, other {{k, select, a {# of them} other {none}}}}, #1", "en", "n" to 3, "k" to "a"))
        // apostrophes quote before {}#|; '' is one; don't stays
        assertEquals("{literal} it's don't #", r("'{literal}' it''s don't '#'"))
        assertEquals("a | b", r("a '|' b"))
    }

    @Test fun catalogTyped() {
        Tr.fixed = "en"
        assertEquals("1 thread", Msg.thread_count(1)); assertEquals("5 threads", Msg.thread_count(5))
        assertEquals("Language", Msg.settings_lang)
        Tr.fixed = "ja"
        assertEquals("言語", Msg.settings_lang)
        assertEquals(Tr.render(Icu.parse(Msg.ja[Msg.keys.indexOf("thread.count")]), mapOf("n" to 5L), "ja"), Msg.thread_count(5))
    }

    @Test fun reverse() {
        Tr.fixed = "ja"
        assertEquals("言語", Tr.t("Language"))
        assertEquals("自動", Tr.t("Automatic"))
        // templates: plural with its number, =0, select
        val ja = { k: String, a: Map<String, Any> -> Tr.render(Icu.parse(Msg.ja[Msg.keys.indexOf(k)]), a, "ja") }
        assertEquals(ja("files.changed", mapOf("n" to 12L)), Tr.t("12 files changed"))
        assertEquals(ja("files.changed", mapOf("n" to 0L)), Tr.t("No files changed"))
        assertEquals(ja("files.changed", mapOf("n" to 1L)), Tr.t("1 file changed"))
        assertEquals(ja("fold.calls", mapOf("count" to 3L, "plural" to "other")), Tr.t("3 tool calls"))
        assertEquals(ja("thread.delete.ask", mapOf("title" to "Board review")), Tr.t("Delete Board review?"))
        // joined parts each on their own
        assertEquals(Tr.t("Language") + " · README.md", Tr.t("Language · README.md"))
        // content is not touched
        for (c in listOf("Fix the USB-C footprint on J3", "README.md", "https://example.com/a", "  ", "12")) assertEquals(c, Tr.t(c))
        Tr.fixed = "en"
        assertEquals("12 files changed", Tr.t("12 files changed"))
    }

    @Test fun languagePolicy() {
        assertEquals("ja", Tr.resolve("auto", "ja")); assertEquals("en", Tr.resolve("auto", "fr")); assertEquals("en", Tr.resolve("auto", "en"))
        assertEquals("ja", Tr.resolve("ja", "en")); assertEquals("en", Tr.resolve("en", "ja"))
    }

    @Test fun everyMessageParsesAndTemplatesReadBack() {
        Tr.fixed = "ja"
        for (i in Msg.keys.indices) { Icu.parse(Msg.en[i]); Icu.parse(Msg.ja[i]) }
        // every template variant matches the English it renders, back to the same Japanese
        var n = 0
        for (i in Msg.templates) for (v in Icu.variants(i, Icu.parse(Msg.en[i])).take(1)) {
            val names = Msg.argNames[i]!!; val a = names.associateWith<String, Any> { if (it in setOf("n", "count", "total", "num") || Msg.en[i].contains("{$it, plural") || Msg.en[i].contains("{$it, number")) 7L else "Xq$it" }
            val en = Tr.render(Icu.parse(Msg.en[i]), a, "en")
            if (en.contains("Xq") || en.contains("7")) { assertTrue("$en reads back", Tr.t(en) != en || Tr.render(Icu.parse(Msg.ja[i]), a, "ja") == en); n++ }
        }
        assertTrue(n > 100)
    }

    /** Msg.kt is what i18n/en.msg and ja.msg say (scripts/i18n-android.py --check, from the JVM too). */
    @Test fun msgIsCurrent() {
        val dir = generateSequence(java.io.File("").absoluteFile) { it.parentFile }.map { java.io.File(it, "i18n") }.first { java.io.File(it, "en.msg").isFile }
        fun load(f: String) = java.io.File(dir, f).readLines().map { it.trim() }.filter { it.isNotEmpty() && !it.startsWith("#") }.associate { l ->
            val k = l.substringBefore(" = "); val v = l.substringAfter(" = ")
            k to Regex("""\\(.)""").replace(v) { m -> when (m.groupValues[1]) { "s" -> " "; "n" -> "\n"; else -> m.groupValues[1] } }
        }
        val en = load("en.msg"); val ja = load("ja.msg")
        assertEquals("keys (run python3 scripts/i18n-android.py)", en.keys.toList(), Msg.keys.toList())
        Msg.keys.forEachIndexed { i, k -> assertEquals(k, en[k], Msg.en[i]); assertEquals(k, ja[k], Msg.ja[i]) }
    }

    /** What the app's pure parts say in English (shared oracles keep them English) reads in Japanese on screen through Tr.t. */
    @Test fun appEnglishReadsInJapanese() {
        Tr.fixed = "ja"
        val said = listOf(HubList.LOCAL, Pairing.message("wrong_code"), Pairing.message("expired_code"), Pairing.message("other"),
            HubRescue.panel(HubFetch.Loading).text, HubRescue.panel(HubFetch.Unreachable).text, HubRescue.after(HubReply.Missing).text, HubRescue.after(HubReply.Unreachable).text,
            HubRescue.step(null, HubRescue.LIMIT_MS, false).text, HubRescue.step(HubHealth("starting", false), 0, false).text, HubRescue.step(HubHealth("ok", true), HubRescue.SETTLE_MS, true).text,
            "Restart hub", "Buy now", "Design change", "Layers")
        for (e in said) assertTrue("not translated: $e", Tr.t(e) != e)
        val fold = DataFold.label(listOf(DEntry("a", "tool", "x", 0, tool = DTool("Read", "x", "")), DEntry("b", "tool", "y", 0, tool = DTool("Bash", "y", "")), DEntry("c", "step", "", 0)), false)
        assertTrue("fold: ${Tr.t(fold)}", Tr.t(fold).startsWith(Tr.t("2 tool calls")) && Tr.t("2 tool calls") != "2 tool calls")
    }
}
