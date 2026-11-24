package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Test

class OrderCardTest {
    @Test
    fun orderCardParsesLeadDaysAndEstimateWarnings() {
        val o = DataClient.Order.of(
            JSONObject()
                .put("name", "Main board")
                .put("line", "Quoted. Buy now to place the order.")
                .put("total", "$27.20")
                .put("lead_days", 9)
                .put("estimate_warnings", "thin board; fragile")
        )
        assertEquals(9, o.leadDays)
        assertEquals("thin board; fragile", o.estimateWarnings)
    }

    @Test
    fun orderCardFallsBackCleanlyWhenAbsentOrNegative() {
        val plain = DataClient.Order.of(JSONObject().put("name", "b").put("lead_days", -3))
        assertEquals(0, plain.leadDays)
        assertEquals("", plain.estimateWarnings)

        val fallback = DataClient.Order.of(
            JSONObject()
                .put("name", "b")
                .put("warnings", JSONArray(listOf(" w1 ", "w1", "w2")))
        )
        assertEquals("w1; w2", fallback.estimateWarnings)
    }
}
