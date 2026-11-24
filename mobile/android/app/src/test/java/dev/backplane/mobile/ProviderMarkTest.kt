package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ProviderMarkTest {
    @Test
    fun allBuiltInProvidersParseWithinViewBox() {
        val expected = listOf("claude", "codex", "grok", "muse", "antigravity", "opencode", "pi", "omp")
        assertEquals(expected, ProviderMarks.ALL.map { it.id })
        for (spec in ProviderMarks.ALL) {
            val segs = ProviderMarks.segments(spec.id)
            assertNotNull("segments for ${spec.id}", segs)
            assertTrue("${spec.id} has segments", segs!!.isNotEmpty())
            val endXs = mutableListOf<Float>()
            val endYs = mutableListOf<Float>()
            val ctrlXs = mutableListOf<Float>()
            val ctrlYs = mutableListOf<Float>()
            for (s in segs) {
                when (s) {
                    is ProviderMarks.Seg.Move -> { endXs.add(s.x); endYs.add(s.y) }
                    is ProviderMarks.Seg.Line -> { endXs.add(s.x); endYs.add(s.y) }
                    is ProviderMarks.Seg.Cubic -> {
                        ctrlXs.addAll(listOf(s.x1, s.x2))
                        ctrlYs.addAll(listOf(s.y1, s.y2))
                        endXs.add(s.x); endYs.add(s.y)
                    }
                    is ProviderMarks.Seg.Quad -> {
                        ctrlXs.add(s.x1); ctrlYs.add(s.y1)
                        endXs.add(s.x); endYs.add(s.y)
                    }
                    ProviderMarks.Seg.Close -> {}
                }
            }
            assertTrue("${spec.id} end minX=${endXs.min()}", endXs.min() >= -1f)
            assertTrue("${spec.id} end maxX=${endXs.max()} <= ${spec.vbW + 1f}", endXs.max() <= spec.vbW + 1f)
            assertTrue("${spec.id} end minY=${endYs.min()}", endYs.min() >= -1f)
            assertTrue("${spec.id} end maxY=${endYs.max()} <= ${spec.vbH + 1f}", endYs.max() <= spec.vbH + 1f)
            val pad = maxOf(2f, spec.vbW * 0.05f)
            if (ctrlXs.isNotEmpty()) {
                assertTrue("${spec.id} ctrl minX=${ctrlXs.min()}", ctrlXs.min() >= -pad)
                assertTrue("${spec.id} ctrl maxX=${ctrlXs.max()}", ctrlXs.max() <= spec.vbW + pad)
                assertTrue("${spec.id} ctrl minY=${ctrlYs.min()}", ctrlYs.min() >= -pad)
                assertTrue("${spec.id} ctrl maxY=${ctrlYs.max()}", ctrlYs.max() <= spec.vbH + pad)
            }
        }
    }

    @Test
    fun aliasesAndFallbackResolveCleanly() {
        assertEquals("antigravity", ProviderMarks.canonical("agy"))
        assertEquals("antigravity", ProviderMarks.canonical("@Gemini"))
        assertEquals("codex", ProviderMarks.canonical("openai"))
        assertEquals("claude", ProviderMarks.canonical("anthropic"))
        assertEquals("claude", ProviderMarks.providerOfChip("claude / opus / high"))
        assertEquals("codex", ProviderMarks.providerOfChip("openai · gpt-5"))
        assertEquals("", ProviderMarks.providerOfChip("unknown / m1"))
        assertNull(ProviderMarks.spec("mystery"))
        assertEquals("?", ProviderMarks.letter("mystery"))
        assertEquals(0x64748BL, ProviderMarks.color("mystery"))
        assertTrue(ProviderMarks.parsePath("M0 0 Z 1 2 ? 3").isNotEmpty())
    }
}
