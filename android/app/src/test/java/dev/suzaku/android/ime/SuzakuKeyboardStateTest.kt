package dev.suzaku.android.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SuzakuKeyboardStateTest {
    @Test
    fun startsWithLowercaseLettersAndExpectedControls() {
        val state = SuzakuKeyboardState()
        val keys = state.rows().flatten()

        assertEquals(SuzakuKeyboardLayout.LETTERS, state.layout)
        assertEquals(SuzakuShiftState.OFF, state.shift)
        assertTrue(keys.any { it.label == "q" && it.value == "q" })
        assertTrue(keys.any { it.action == SuzakuSoftKeyAction.SHIFT })
        assertTrue(keys.any { it.action == SuzakuSoftKeyAction.SHOW_NUMBERS })
        assertTrue(keys.any { it.action == SuzakuSoftKeyAction.BACKSPACE })
        assertTrue(keys.any { it.action == SuzakuSoftKeyAction.SWITCH_INPUT_METHOD })
    }

    @Test
    fun oneShotShiftUppercasesOneCharacterThenTurnsOff() {
        val state = SuzakuKeyboardState()
        state.press(keyWithAction(state, SuzakuSoftKeyAction.SHIFT))

        assertEquals(SuzakuShiftState.ONCE, state.shift)
        val shiftedQ = state.rows().flatten().first { it.label == "Q" }
        val effect = state.press(shiftedQ)

        assertEquals(SuzakuKeyboardEffect.Insert("Q", keysChanged = true), effect)
        assertEquals(SuzakuShiftState.OFF, state.shift)
        assertTrue(state.rows().flatten().any { it.label == "q" })
    }

    @Test
    fun secondShiftTapLocksCapsUntilShiftIsTappedAgain() {
        val state = SuzakuKeyboardState()
        state.press(keyWithAction(state, SuzakuSoftKeyAction.SHIFT))
        state.press(keyWithAction(state, SuzakuSoftKeyAction.SHIFT))

        assertEquals(SuzakuShiftState.LOCKED, state.shift)
        val shiftedA = state.rows().flatten().first { it.label == "A" }
        assertEquals(SuzakuKeyboardEffect.Insert("A", keysChanged = false), state.press(shiftedA))
        assertEquals(SuzakuShiftState.LOCKED, state.shift)

        state.press(keyWithAction(state, SuzakuSoftKeyAction.SHIFT))
        assertEquals(SuzakuShiftState.OFF, state.shift)
    }

    @Test
    fun numberAndSymbolPagesCanRoundTripBackToLetters() {
        val state = SuzakuKeyboardState()
        state.press(keyWithAction(state, SuzakuSoftKeyAction.SHOW_NUMBERS))

        assertEquals(SuzakuKeyboardLayout.NUMBERS, state.layout)
        assertTrue(state.rows().flatten().any { it.label == "1" })
        assertTrue(state.rows().flatten().any { it.action == SuzakuSoftKeyAction.SHOW_SYMBOLS })

        state.press(keyWithAction(state, SuzakuSoftKeyAction.SHOW_SYMBOLS))
        assertEquals(SuzakuKeyboardLayout.SYMBOLS, state.layout)
        assertTrue(state.rows().flatten().any { it.label == "€" })

        state.press(keyWithAction(state, SuzakuSoftKeyAction.SHOW_LETTERS))
        assertEquals(SuzakuKeyboardLayout.LETTERS, state.layout)
        assertTrue(state.rows().flatten().any { it.label == "z" })
    }

    @Test
    fun resetCanPreferNumbersForNumericEditors() {
        val state = SuzakuKeyboardState()
        state.press(keyWithAction(state, SuzakuSoftKeyAction.SHIFT))
        state.reset(profile = SuzakuKeyboardProfile.NUMBER)

        assertEquals(SuzakuKeyboardLayout.NUMBERS, state.layout)
        assertEquals(SuzakuShiftState.OFF, state.shift)
        assertFalse(state.rows().flatten().any { it.action == SuzakuSoftKeyAction.SHIFT })
    }

    @Test
    fun numberRowCanBeEnabledWithoutLeavingLetterLayout() {
        val state = SuzakuKeyboardState()
        assertTrue(state.setNumberRowEnabled(true))

        assertEquals(SuzakuKeyboardLayout.LETTERS, state.layout)
        assertEquals(5, state.rows().size)
        assertEquals(
            listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0"),
            state.rows().first().map { it.label },
        )
        assertFalse(state.setNumberRowEnabled(true))

        assertTrue(state.setNumberRowEnabled(false))
        assertEquals(4, state.rows().size)
    }

    @Test
    fun automaticShiftRespectsCapsLockAndNumericLayouts() {
        val state = SuzakuKeyboardState()
        state.reset(capitalize = true)
        assertEquals(SuzakuShiftState.ONCE, state.shift)

        assertTrue(state.applyAutomaticShift(false))
        assertEquals(SuzakuShiftState.OFF, state.shift)
        state.press(keyWithAction(state, SuzakuSoftKeyAction.SHIFT))
        state.press(keyWithAction(state, SuzakuSoftKeyAction.SHIFT))
        assertEquals(SuzakuShiftState.LOCKED, state.shift)
        assertFalse(state.applyAutomaticShift(false))

        state.reset(profile = SuzakuKeyboardProfile.NUMBER, capitalize = true)
        assertEquals(SuzakuKeyboardLayout.NUMBERS, state.layout)
        assertEquals(SuzakuShiftState.OFF, state.shift)
        assertFalse(state.applyAutomaticShift(true))
    }

    @Test
    fun emailAndUriProfilesExposeContextShortcuts() {
        val state = SuzakuKeyboardState()
        state.reset(profile = SuzakuKeyboardProfile.EMAIL, enterKey = SuzakuEnterKey.SEND)

        var keys = state.rows().flatten()
        assertTrue(keys.any { it.value == "@" })
        assertTrue(keys.any { it.value == ".com" })
        assertFalse(keys.any { it.action == SuzakuSoftKeyAction.SPACE })
        assertTrue(keys.any { it.action == SuzakuSoftKeyAction.ENTER && it.label == "Send" })
        state.press(keyWithAction(state, SuzakuSoftKeyAction.SHIFT))
        assertTrue(state.rows().flatten().any { it.value == ".com" })

        state.reset(profile = SuzakuKeyboardProfile.URI, enterKey = SuzakuEnterKey.GO)
        keys = state.rows().flatten()
        assertTrue(keys.any { it.value == "/" })
        assertTrue(keys.any { it.value == ".com" })
        assertTrue(keys.any { it.action == SuzakuSoftKeyAction.ENTER && it.label == "Go" })
    }

    @Test
    fun numberProfileBuildsRestrictedPadFromEditorFlags() {
        val state = SuzakuKeyboardState()
        state.reset(
            profile = SuzakuKeyboardProfile.NUMBER,
            signedNumbers = true,
            decimalNumbers = true,
            enterKey = SuzakuEnterKey.DONE,
        )

        val keys = state.rows().flatten()
        assertEquals(SuzakuKeyboardLayout.NUMBERS, state.layout)
        assertTrue(keys.any { it.value == "-" })
        assertTrue(keys.any { it.value == "." })
        assertTrue(keys.any { it.action == SuzakuSoftKeyAction.BACKSPACE })
        assertTrue(keys.any { it.action == SuzakuSoftKeyAction.ENTER && it.label == "Done" })
        assertFalse(keys.any { it.action == SuzakuSoftKeyAction.SHOW_LETTERS })
        assertFalse(keys.any { it.action == SuzakuSoftKeyAction.SHOW_SYMBOLS })

        state.reset(profile = SuzakuKeyboardProfile.NUMBER)
        val unsignedIntegerKeys = state.rows().flatten()
        assertFalse(unsignedIntegerKeys.any { it.value == "-" })
        assertFalse(unsignedIntegerKeys.any { it.value == "." })
    }

    @Test
    fun phoneAndDatetimeProfilesExposeExpectedSeparators() {
        val state = SuzakuKeyboardState()
        state.reset(profile = SuzakuKeyboardProfile.PHONE)
        var values = state.rows().flatten().map { it.value }.toSet()
        assertTrue(values.containsAll(setOf("+", "*", "#")))

        state.reset(profile = SuzakuKeyboardProfile.DATETIME)
        values = state.rows().flatten().map { it.value }.toSet()
        assertTrue(values.containsAll(setOf("/", ":", "-")))
    }

    @Test
    fun punctuationCommitsAtCandidateBoundaryAndEveryProfileCanSwitchKeyboard() {
        val state = SuzakuKeyboardState()
        val period = state.rows().flatten().first { it.value == "." }
        assertEquals(SuzakuKeyboardEffect.CommitLiteral("."), state.press(period))

        state.press(keyWithAction(state, SuzakuSoftKeyAction.SHOW_NUMBERS))
        val digit = state.rows().flatten().first { it.value == "1" }
        assertEquals(SuzakuKeyboardEffect.CommitLiteral("1"), state.press(digit))

        SuzakuKeyboardProfile.entries.forEach { profile ->
            state.reset(profile = profile)
            val switchKey = keyWithAction(state, SuzakuSoftKeyAction.SWITCH_INPUT_METHOD)
            assertEquals(SuzakuKeyboardEffect.SwitchInputMethod, state.press(switchKey))
        }
    }

    @Test
    fun dropLastTextElementKeepsCommonUnicodeClustersIntact() {
        assertEquals("hello", dropLastTextElement("hello😀"))
        assertEquals("", dropLastTextElement("😀"))
        assertEquals("", dropLastTextElement("e\u0301"))
        assertEquals("", dropLastTextElement("👍🏽"))
        assertEquals("", dropLastTextElement("🇨🇳"))
        assertEquals("", dropLastTextElement("👨‍👩‍👧‍👦"))
        assertEquals("hello", dropLastTextElement("hello!"))
        assertEquals("", dropLastTextElement(""))
    }

    private fun keyWithAction(
        state: SuzakuKeyboardState,
        action: SuzakuSoftKeyAction,
    ): SuzakuSoftKey = state.rows().flatten().first { it.action == action }
}
