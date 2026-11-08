package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Test

class LinkGrammarTest {
    private fun kinds(t: String) = linkRuns(t).mapNotNull { (text, url) -> url?.let { "$it|$text" } }

    @Test fun paths() {
        assertEquals(listOf("bppath:src/x.rs:12|src/x.rs:12"), kinds("see src/x.rs:12 now"))
        assertEquals(listOf("bppath:src/x.rs|src/x.rs"), kinds("in (src/x.rs)."))
        assertEquals(listOf("bppath:/home/h/proj/src/a.kt:3:4|/home/h/proj/src/a.kt:3:4"), kinds("/home/h/proj/src/a.kt:3:4,"))
        assertEquals(listOf("bppath:README.md|README.md"), kinds("see README.md."))
        assertEquals(listOf("bppath:src/x.rs#L40-L50|src/x.rs#L40-L50"), kinds("src/x.rs#L40-L50"))
    }
    @Test fun prose_is_not_a_link() {
        assertEquals(emptyList<String>(), kinds("and/or TCP/IP 1/2 e.g. i.e. v1.2.3 a.b"))
        assertEquals(emptyList<String>(), kinds("snake_case_word and foo/bar"))
        assertEquals(emptyList<String>(), kinds("ids 1700000000-1 and 20261107"))
        assertEquals(emptyList<String>(), kinds("x me@x"))
    }
    @Test fun contacts() {
        assertEquals(listOf("mailto:a.b@example.com|a.b@example.com"), kinds("mail a.b@example.com, ok"))
        assertEquals(listOf("tel:+15551234567|+1 555 123 4567"), kinds("call +1 555 123 4567 now"))
        assertEquals(listOf("tel:5551234567|(555) 123-4567"), kinds("call (555) 123-4567."))
    }
    @Test fun web_and_backplane() {
        assertEquals(listOf("https://example.com/a_(b)|https://example.com/a_(b)"), kinds("visit https://example.com/a_(b)). ok"))
        assertEquals(listOf("backplane://thread/lab~t9|backplane://thread/lab~t9"), kinds("(backplane://thread/lab~t9)"))
        assertEquals(listOf("backplane://conv/room%3Aabc|backplane://conv/room%3Aabc"), kinds("backplane://conv/room%3Aabc"))
    }
}
