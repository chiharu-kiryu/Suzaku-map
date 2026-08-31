package dev.suzaku.android.ime

internal object SuzakuCompositionSelectionPolicy {
    fun shouldCancel(
        hasComposition: Boolean,
        newSelectionStart: Int,
        newSelectionEnd: Int,
        composingStart: Int,
        composingEnd: Int,
    ): Boolean {
        if (!hasComposition) {
            return false
        }
        if (composingStart < 0 || composingEnd <= composingStart) {
            return true
        }
        return newSelectionStart != composingEnd || newSelectionEnd != composingEnd
    }
}
