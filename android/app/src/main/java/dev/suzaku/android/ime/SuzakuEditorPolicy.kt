package dev.suzaku.android.ime

import android.text.InputType
import android.view.inputmethod.EditorInfo

internal data class SuzakuEditorPolicy(
    val inputType: Int,
    val inputClass: Int,
    val secureInput: Boolean,
    val keyboardProfile: SuzakuKeyboardProfile,
    val signedNumbers: Boolean,
    val decimalNumbers: Boolean,
    val suggestionsEnabled: Boolean,
    val editorCompletionsEnabled: Boolean,
    val capitalizationFlags: Int,
    val editorAction: Int,
    val enterKey: SuzakuEnterKey,
) {
    companion object {
        fun from(
            inputType: Int,
            imeOptions: Int = EditorInfo.IME_ACTION_NONE,
        ): SuzakuEditorPolicy {
            val inputClass = inputType.and(InputType.TYPE_MASK_CLASS)
            val variation = inputType.and(InputType.TYPE_MASK_VARIATION)
            val secureInput = when (inputClass) {
                InputType.TYPE_CLASS_TEXT -> variation == InputType.TYPE_TEXT_VARIATION_PASSWORD ||
                    variation == InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD ||
                    variation == InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD
                InputType.TYPE_CLASS_NUMBER -> variation == InputType.TYPE_NUMBER_VARIATION_PASSWORD
                else -> false
            }
            val keyboardProfile = when (inputClass) {
                InputType.TYPE_CLASS_NUMBER -> SuzakuKeyboardProfile.NUMBER
                InputType.TYPE_CLASS_PHONE -> SuzakuKeyboardProfile.PHONE
                InputType.TYPE_CLASS_DATETIME -> SuzakuKeyboardProfile.DATETIME
                InputType.TYPE_CLASS_TEXT -> when (variation) {
                    InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS,
                    InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS,
                    -> SuzakuKeyboardProfile.EMAIL
                    InputType.TYPE_TEXT_VARIATION_URI -> SuzakuKeyboardProfile.URI
                    else -> SuzakuKeyboardProfile.TEXT
                }
                else -> SuzakuKeyboardProfile.TEXT
            }
            val signedNumbers = inputClass == InputType.TYPE_CLASS_NUMBER &&
                inputType.and(InputType.TYPE_NUMBER_FLAG_SIGNED) != 0
            val decimalNumbers = inputClass == InputType.TYPE_CLASS_NUMBER &&
                inputType.and(InputType.TYPE_NUMBER_FLAG_DECIMAL) != 0
            val autoComplete = inputType.and(InputType.TYPE_TEXT_FLAG_AUTO_COMPLETE) != 0
            val suggestionsDisabledByFlags =
                inputType.and(InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS) != 0 || autoComplete
            val suggestionsEnabled = inputClass == InputType.TYPE_CLASS_TEXT &&
                keyboardProfile == SuzakuKeyboardProfile.TEXT &&
                !secureInput &&
                !suggestionsDisabledByFlags
            val editorCompletionsEnabled = inputClass == InputType.TYPE_CLASS_TEXT &&
                !secureInput &&
                autoComplete
            val capitalizationFlags = inputType.and(
                InputType.TYPE_TEXT_FLAG_CAP_CHARACTERS or
                    InputType.TYPE_TEXT_FLAG_CAP_WORDS or
                    InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
            )
            val requestedAction = imeOptions.and(EditorInfo.IME_MASK_ACTION)
            val editorAction = if (imeOptions.and(EditorInfo.IME_FLAG_NO_ENTER_ACTION) != 0) {
                EditorInfo.IME_ACTION_NONE
            } else {
                requestedAction
            }
            val enterKey = when (editorAction) {
                EditorInfo.IME_ACTION_GO -> SuzakuEnterKey.GO
                EditorInfo.IME_ACTION_SEARCH -> SuzakuEnterKey.SEARCH
                EditorInfo.IME_ACTION_SEND -> SuzakuEnterKey.SEND
                EditorInfo.IME_ACTION_NEXT -> SuzakuEnterKey.NEXT
                EditorInfo.IME_ACTION_DONE -> SuzakuEnterKey.DONE
                EditorInfo.IME_ACTION_PREVIOUS -> SuzakuEnterKey.PREVIOUS
                else -> SuzakuEnterKey.ENTER
            }
            return SuzakuEditorPolicy(
                inputType = inputType,
                inputClass = inputClass,
                secureInput = secureInput,
                keyboardProfile = keyboardProfile,
                signedNumbers = signedNumbers,
                decimalNumbers = decimalNumbers,
                suggestionsEnabled = suggestionsEnabled,
                editorCompletionsEnabled = editorCompletionsEnabled,
                capitalizationFlags = capitalizationFlags,
                editorAction = editorAction,
                enterKey = enterKey,
            )
        }
    }
}
