package dev.backplane.mobile

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

// The alerts banner's Not now: asked again a week later, not on the next launch.
class AlertsDueTest {
    private val day = 24L * 60 * 60 * 1000
    private val at = 1_800_000_000_000L
    @Test fun never_pressed_is_due() { assertTrue(alertsDue(at, 0)) }
    @Test fun within_the_week_waits() {
        assertFalse(alertsDue(at, at))
        assertFalse(alertsDue(at + 6 * day, at))
        assertFalse(alertsDue(at + 7 * day - 1, at))
    }
    @Test fun a_week_later_is_due() { assertTrue(alertsDue(at + 7 * day, at)); assertTrue(alertsDue(at + 30 * day, at)) }
    @Test fun a_clock_moved_back_is_due() { assertTrue(alertsDue(at - day, at)) }
}
