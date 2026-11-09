package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Test

// The find bar's stepping and the search snippet's marks (DataFind.kt).
class DataFindTest {
    private val m = listOf("m1", "m4", "m9")

    @Test fun newResultsStartOnTheNewestOrKeepTheOneShown() {
        assertEquals(2, DataFind.pick(m, null))
        assertEquals(1, DataFind.pick(m, "m4"))
        assertEquals(2, DataFind.pick(m, "gone"))
        assertEquals(-1, DataFind.pick(emptyList(), "m4"))
    }
    @Test fun nextAndPreviousWrapRound() {
        assertEquals(1, DataFind.step(2, 3, -1)); assertEquals(0, DataFind.step(2, 3, 1)); assertEquals(2, DataFind.step(0, 3, -1))
        assertEquals(0, DataFind.step(0, 1, 1)); assertEquals(-1, DataFind.step(0, 0, 1)); assertEquals(2, DataFind.step(-1, 3, -1))
    }
    @Test fun theCountSaysWhereAndHowMany() {
        assertEquals("3 of 3", DataFind.count(2, 3, 3, false)); assertEquals("1 of 1000+", DataFind.count(0, 1000, 1000, true)); assertEquals("no match", DataFind.count(-1, 0, 0, false))
    }
    @Test fun marksAreEveryWordAnyCaseJoined() {
        assertEquals(listOf(4..9, 17..22), DataFind.marks("the Zephyr, said zephyrs", "zephyr"))
        assertEquals(listOf(0..4), DataFind.marks("abcde", "abc cde"))
        assertEquals(listOf(0..1, 3..4), DataFind.marks("ab ab", "  AB "))
        assertEquals(emptyList<IntRange>(), DataFind.marks("text", ""))
    }
    @Test fun whoSaidIt() { assertEquals("you said", DataFind.who("user")); assertEquals("agent said", DataFind.who("assistant")) }
}
