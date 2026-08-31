package dev.suzaku.android.ime

import android.content.Context
import android.content.SharedPreferences

internal data class SuzakuImeSettings(
    val autoCapitalization: Boolean = true,
    val numberRow: Boolean = false,
    val hapticFeedback: Boolean = true,
)

internal class SuzakuImeSettingsStore(context: Context) {
    private val preferences: SharedPreferences = context
        .createDeviceProtectedStorageContext()
        .getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE)

    fun load(): SuzakuImeSettings = SuzakuImeSettings(
        autoCapitalization = preferences.getBoolean(KEY_AUTO_CAPITALIZATION, true),
        numberRow = preferences.getBoolean(KEY_NUMBER_ROW, false),
        hapticFeedback = preferences.getBoolean(KEY_HAPTIC_FEEDBACK, true),
    )

    fun save(settings: SuzakuImeSettings) {
        preferences.edit()
            .putBoolean(KEY_AUTO_CAPITALIZATION, settings.autoCapitalization)
            .putBoolean(KEY_NUMBER_ROW, settings.numberRow)
            .putBoolean(KEY_HAPTIC_FEEDBACK, settings.hapticFeedback)
            .apply()
    }

    fun reset(): SuzakuImeSettings {
        preferences.edit().clear().apply()
        return SuzakuImeSettings()
    }

    private companion object {
        const val PREFERENCES_NAME = "suzaku_ime_settings"
        const val KEY_AUTO_CAPITALIZATION = "auto_capitalization"
        const val KEY_NUMBER_ROW = "number_row"
        const val KEY_HAPTIC_FEEDBACK = "haptic_feedback"
    }
}
