package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

// The review bar (feature "reviewmode", docs/PHONE-DATA.md, #1305): the hub's words as they are, and the value its one button sends.
class DReviewBarTest {
    @Test fun the_bar_is_the_hubs_and_its_button_sends_the_other_state() {
        val on = DReviewBar.of(JSONObject("""{"on":true,"text":"Read-only: this request looked like a review, so Backplane refuses changes to the design files.","button":"Allow edits","tip":"t","value":false}"""))!!
        assertEquals(DReviewBar(true, "Read-only: this request looked like a review, so Backplane refuses changes to the design files.", "Allow edits", "t", false), on)
        val off = DReviewBar.of(JSONObject("""{"on":false,"text":"Edits allowed: you switched review mode off for this thread.","button":"Read-only again","tip":"t","value":true}"""))!!
        assertEquals("pressed again: back on", true, off.value)
        assertNull("no review: no bar", DReviewBar.of(null))
        assertNull("nothing to draw", DReviewBar.of(JSONObject("""{"on":true}""")))
    }
}
