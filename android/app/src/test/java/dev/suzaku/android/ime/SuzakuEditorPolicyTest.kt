package dev.suzaku.android.ime

import android.text.InputType
import android.view.inputmethod.EditorInfo
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SuzakuEditorPolicyTest {
    @Test
    fun recognizesAllTextPasswordVariations() {
        listOf(
            InputType.TYPE_TEXT_VARIATION_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
        ).forEach { variation ->
            val policy = SuzakuEditorPolicy.from(InputType.TYPE_CLASS_TEXT or variation)
            assertTrue(policy.secureInput)
            assertEquals(SuzakuKeyboardProfile.TEXT, policy.keyboardProfile)
            assertFalse(policy.suggestionsEnabled)
            assertFalse(policy.editorCompletionsEnabled)
        }
    }

    @Test
    fun recognizesNumericPasswordsAndKeepsNumericLayout() {
        val policy = SuzakuEditorPolicy.from(
            InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_VARIATION_PASSWORD
        )

        assertTrue(policy.secureInput)
        assertEquals(SuzakuKeyboardProfile.NUMBER, policy.keyboardProfile)
        assertEquals(InputType.TYPE_CLASS_NUMBER, policy.inputClass)
        assertFalse(policy.suggestionsEnabled)
    }

    @Test
    fun mapsOrdinaryTextPhoneAndDatetimeEditorsToProfiles() {
        val text = SuzakuEditorPolicy.from(InputType.TYPE_CLASS_TEXT)
        val phone = SuzakuEditorPolicy.from(InputType.TYPE_CLASS_PHONE)
        val datetime = SuzakuEditorPolicy.from(InputType.TYPE_CLASS_DATETIME)

        assertFalse(text.secureInput)
        assertEquals(SuzakuKeyboardProfile.TEXT, text.keyboardProfile)
        assertTrue(text.suggestionsEnabled)
        assertFalse(phone.secureInput)
        assertEquals(SuzakuKeyboardProfile.PHONE, phone.keyboardProfile)
        assertFalse(phone.suggestionsEnabled)
        assertEquals(SuzakuKeyboardProfile.DATETIME, datetime.keyboardProfile)
    }

    @Test
    fun preservesCapitalizationFlagsSeparatelyFromVariation() {
        val inputType = InputType.TYPE_CLASS_TEXT or
            InputType.TYPE_TEXT_VARIATION_LONG_MESSAGE or
            InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
        val policy = SuzakuEditorPolicy.from(inputType)

        assertFalse(policy.secureInput)
        assertEquals(InputType.TYPE_TEXT_FLAG_CAP_SENTENCES, policy.capitalizationFlags)
    }

    @Test
    fun recognizesEmailAndUriTextVariations() {
        listOf(
            InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS,
            InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS,
        ).forEach { variation ->
            val policy = SuzakuEditorPolicy.from(InputType.TYPE_CLASS_TEXT or variation)
            assertEquals(SuzakuKeyboardProfile.EMAIL, policy.keyboardProfile)
            assertFalse(policy.suggestionsEnabled)
        }

        val uri = SuzakuEditorPolicy.from(
            InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
        )
        assertEquals(SuzakuKeyboardProfile.URI, uri.keyboardProfile)
        assertFalse(uri.suggestionsEnabled)
    }

    @Test
    fun carriesSignedAndDecimalNumberFlagsIntoPolicy() {
        val policy = SuzakuEditorPolicy.from(
            InputType.TYPE_CLASS_NUMBER or
                InputType.TYPE_NUMBER_FLAG_SIGNED or
                InputType.TYPE_NUMBER_FLAG_DECIMAL
        )

        assertEquals(SuzakuKeyboardProfile.NUMBER, policy.keyboardProfile)
        assertTrue(policy.signedNumbers)
        assertTrue(policy.decimalNumbers)
    }

    @Test
    fun honorsNoSuggestionsAndAutoCompleteFlags() {
        val noSuggestions = SuzakuEditorPolicy.from(
            InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        )
        assertEquals(SuzakuKeyboardProfile.TEXT, noSuggestions.keyboardProfile)
        assertFalse(noSuggestions.suggestionsEnabled)
        assertFalse(noSuggestions.editorCompletionsEnabled)

        val autoComplete = SuzakuEditorPolicy.from(
            InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_AUTO_COMPLETE
        )
        assertEquals(SuzakuKeyboardProfile.TEXT, autoComplete.keyboardProfile)
        assertFalse(autoComplete.suggestionsEnabled)
        assertTrue(autoComplete.editorCompletionsEnabled)

        val secureAutoComplete = SuzakuEditorPolicy.from(
            InputType.TYPE_CLASS_TEXT or
                InputType.TYPE_TEXT_VARIATION_PASSWORD or
                InputType.TYPE_TEXT_FLAG_AUTO_COMPLETE
        )
        assertTrue(secureAutoComplete.secureInput)
        assertFalse(secureAutoComplete.editorCompletionsEnabled)
    }

    @Test
    fun mapsEditorActionsAndHonorsNoEnterActionFlag() {
        val actions = mapOf(
            EditorInfo.IME_ACTION_GO to SuzakuEnterKey.GO,
            EditorInfo.IME_ACTION_SEARCH to SuzakuEnterKey.SEARCH,
            EditorInfo.IME_ACTION_SEND to SuzakuEnterKey.SEND,
            EditorInfo.IME_ACTION_NEXT to SuzakuEnterKey.NEXT,
            EditorInfo.IME_ACTION_DONE to SuzakuEnterKey.DONE,
            EditorInfo.IME_ACTION_PREVIOUS to SuzakuEnterKey.PREVIOUS,
        )
        actions.forEach { (option, enterKey) ->
            val policy = SuzakuEditorPolicy.from(InputType.TYPE_CLASS_TEXT, option)
            assertEquals(option, policy.editorAction)
            assertEquals(enterKey, policy.enterKey)
        }

        val multiline = SuzakuEditorPolicy.from(
            InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE,
            EditorInfo.IME_ACTION_DONE or EditorInfo.IME_FLAG_NO_ENTER_ACTION,
        )
        assertEquals(EditorInfo.IME_ACTION_NONE, multiline.editorAction)
        assertEquals(SuzakuEnterKey.ENTER, multiline.enterKey)
    }
}
