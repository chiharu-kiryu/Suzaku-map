package dev.suzaku.android.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SuzakuKeyRepeatStateTest {
    @Test
    fun startsAfterInitialDelayThenUsesRepeatInterval() {
        val state = SuzakuKeyRepeatState(
            initialDelayMillis = 400L,
            repeatIntervalMillis = 70L,
        )

        assertEquals(400L, state.begin())
        assertTrue(state.isActive)
        assertFalse(state.hasRepeated)
        assertEquals(70L, state.tick())
        assertTrue(state.hasRepeated)
        assertTrue(state.finish())
        assertFalse(state.isActive)
        assertFalse(state.hasRepeated)
    }

    @Test
    fun quickTapAndInactiveTicksNeverReportARepeat() {
        val state = SuzakuKeyRepeatState()

        assertNull(state.tick())
        state.begin()
        assertFalse(state.finish())
        assertNull(state.tick())
        assertFalse(state.finish())
    }
}
