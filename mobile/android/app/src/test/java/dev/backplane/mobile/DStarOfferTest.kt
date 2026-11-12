package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

// The GitHub star card (feature "star", docs/PHONE-DATA.md): the hub's words as they are, drawn at a pause.
class DStarOfferTest {
    private val o = """{"token":"m5","text":"Like Backplane? Give us a star on GitHub!","url":"https://github.com/BackplaneWorks/Backplane","label":"Star on GitHub","repo":"BackplaneWorks/Backplane","why":"Stars help."}"""
    @Test fun an_offer_carries_the_repository_and_why() {
        val s = DStarOffer.of(JSONObject(o))!!
        assertEquals(DStarOffer("m5", "Like Backplane? Give us a star on GitHub!", "https://github.com/BackplaneWorks/Backplane", "Star on GitHub", "BackplaneWorks/Backplane", "Stars help."), s)
        assertEquals("github.com/BackplaneWorks/Backplane", s.place)
        assertEquals("an older hub: no repo, no why", "", DStarOffer.of(JSONObject("""{"token":"m5","text":"x","url":"https://github.com/a/b","label":""}"""))!!.why)
        assertNull("only a web page", DStarOffer.of(JSONObject("""{"token":"m5","text":"x","url":"file:///etc/passwd"}""")))
    }
    @Test fun the_card_waits_for_a_pause() {
        val s = DStarOffer.of(JSONObject(o))
        assertTrue(DStarOffer.due(s, running = false, typing = false))
        assertFalse("a turn running", DStarOffer.due(s, running = true, typing = false))
        assertFalse("never under the person's typing", DStarOffer.due(s, running = false, typing = true))
        assertFalse("no offer", DStarOffer.due(null, running = false, typing = false))
    }
}
