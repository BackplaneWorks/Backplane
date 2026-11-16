package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

// CatPick: the colour a person types, the rig cache key and the preview's question while a name is typed (feature cats)
class CatPickTest {
    @Test fun coloursFollowTheHubsRule() {
        assertEquals("#6fb3f2", CatPick.hex("#6FB3F2"))
        assertEquals("#6fb3f2", CatPick.hex(" 6fb3f2 "))
        for (bad in listOf("", "#", "#6fb3f", "#6fb3f2a", "#6fb3fg", "6f b3f2", "##6fb3f2", "red")) assertNull(bad, CatPick.hex(bad))
    }

    @Test fun oneKeyPerLookColourAndMood() {
        assertEquals(CatPick.key(5, "#6FB3F2", "idle"), CatPick.key(5, "6fb3f2", "idle"))
        assertEquals("5||idle", CatPick.key(5, "", ""))
        assertEquals(CatPick.key(5, "", "idle"), CatPick.key(5, "nope", "idle"))
        assertNotEquals(CatPick.key(5, "", "idle"), CatPick.key(6, "", "idle"))
        assertNotEquals(CatPick.key(5, "", "idle"), CatPick.key(5, "", "sleep"))
    }

    @Test fun pickedWhenTheNameGivesAnotherLook() {
        assertFalse(CatPick.picked(12, 12))
        assertTrue(CatPick.picked(12, 40))
        assertFalse(CatPick.picked(12, null))
    }

    @Test fun thePreviewAsksByNameUnlessPicked() {
        assertEquals(mapOf("name" to "Mog", "color" to "#6fb3f2", "mood" to "idle"), CatPick.ask(" Mog ", "#6fb3f2", 12, picked = false))
        assertEquals(mapOf("look" to 12, "color" to "", "mood" to "idle"), CatPick.ask("Mog", "", 12, picked = true))
        assertEquals(mapOf("look" to 7, "color" to "", "mood" to "idle"), CatPick.ask("  ", "", 7, picked = false))
    }
}
