package dev.suzaku.android.ime

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SuzakuCompositionSelectionPolicyTest {
    @Test
    fun keepsCompositionWhenCursorRemainsAtItsEnd() {
        assertFalse(
            SuzakuCompositionSelectionPolicy.shouldCancel(
                hasComposition = true,
                newSelectionStart = 8,
                newSelectionEnd = 8,
                composingStart = 3,
                composingEnd = 8,
            )
        )
    }

    @Test
    fun cancelsWhenCursorOrSelectionMovesAwayFromCompositionEnd() {
        listOf(
            4 to 4,
            2 to 2,
            3 to 8,
            8 to 9,
        ).forEach { (start, end) ->
            assertTrue(
                SuzakuCompositionSelectionPolicy.shouldCancel(
                    hasComposition = true,
                    newSelectionStart = start,
                    newSelectionEnd = end,
                    composingStart = 3,
                    composingEnd = 8,
                )
            )
        }
    }

    @Test
    fun cancelsCompositionWhenEditorStopsReportingAValidRange() {
        assertTrue(
            SuzakuCompositionSelectionPolicy.shouldCancel(
                hasComposition = true,
                newSelectionStart = 8,
                newSelectionEnd = 8,
                composingStart = -1,
                composingEnd = -1,
            )
        )
        assertTrue(
            SuzakuCompositionSelectionPolicy.shouldCancel(
                hasComposition = true,
                newSelectionStart = 8,
                newSelectionEnd = 8,
                composingStart = 8,
                composingEnd = 8,
            )
        )
    }

    @Test
    fun ignoresSelectionChangesWithoutAnActiveComposition() {
        assertFalse(
            SuzakuCompositionSelectionPolicy.shouldCancel(
                hasComposition = false,
                newSelectionStart = 1,
                newSelectionEnd = 4,
                composingStart = -1,
                composingEnd = -1,
            )
        )
    }
}
