package dev.suzaku.android.ime

import android.content.ComponentName
import android.content.Intent
import android.os.Bundle
import android.provider.Settings
import android.widget.Button
import android.widget.TextView
import android.view.inputmethod.InputMethodManager
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.SwitchCompat

class SuzakuPanelActivity : AppCompatActivity() {
    private lateinit var setupStatus: TextView
    private lateinit var showImePickerButton: Button
    private lateinit var panelSettingsAutoCapitalization: SwitchCompat
    private lateinit var panelSettingsNumberRow: SwitchCompat
    private lateinit var panelSettingsHapticFeedback: SwitchCompat
    private lateinit var panelSettingsStatus: TextView
    private lateinit var settingsStore: SuzakuImeSettingsStore
    private var panelSettings = SuzakuImeSettings()
    private var syncingSettingsControls = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_panel)
        settingsStore = SuzakuImeSettingsStore(this)
        bindViews()
        configureSetupActions()
        configurePreferenceControls()
    }

    override fun onResume() {
        super.onResume()
        refreshSetupStatus()
        syncPreferenceControls(settingsStore.load())
        refreshDiagnostics()
    }

    private fun bindViews() {
        setupStatus = findViewById(R.id.setupStatus)
        showImePickerButton = findViewById(R.id.showImePickerButton)
        panelSettingsAutoCapitalization = findViewById(R.id.panelSettingsAutoCapitalization)
        panelSettingsNumberRow = findViewById(R.id.panelSettingsNumberRow)
        panelSettingsHapticFeedback = findViewById(R.id.panelSettingsHapticFeedback)
        panelSettingsStatus = findViewById(R.id.panelSettingsStatus)
    }

    private fun configureSetupActions() {
        findViewById<Button>(R.id.openImeSettingsButton).setOnClickListener {
            startActivity(Intent(Settings.ACTION_INPUT_METHOD_SETTINGS))
        }
        showImePickerButton.setOnClickListener {
            getSystemService(InputMethodManager::class.java)?.showInputMethodPicker()
        }
        findViewById<Button>(R.id.refreshSetupButton).setOnClickListener {
            refreshSetupStatus()
            syncPreferenceControls(settingsStore.load())
            refreshDiagnostics()
        }
    }

    private fun configurePreferenceControls() {
        panelSettingsAutoCapitalization.setOnCheckedChangeListener { _, enabled ->
            if (!syncingSettingsControls) {
                savePanelSettings(panelSettings.copy(autoCapitalization = enabled))
            }
        }
        panelSettingsNumberRow.setOnCheckedChangeListener { _, enabled ->
            if (!syncingSettingsControls) {
                savePanelSettings(panelSettings.copy(numberRow = enabled))
            }
        }
        panelSettingsHapticFeedback.setOnCheckedChangeListener { _, enabled ->
            if (!syncingSettingsControls) {
                savePanelSettings(panelSettings.copy(hapticFeedback = enabled))
            }
        }
        findViewById<Button>(R.id.panelSettingsResetButton).setOnClickListener {
            syncPreferenceControls(settingsStore.reset())
            panelSettingsStatus.text = getString(R.string.settings_defaults_restored)
        }
    }

    private fun savePanelSettings(updated: SuzakuImeSettings) {
        panelSettings = updated
        settingsStore.save(updated)
        panelSettingsStatus.text = getString(R.string.settings_saved)
    }

    private fun syncPreferenceControls(settings: SuzakuImeSettings) {
        panelSettings = settings
        syncingSettingsControls = true
        panelSettingsAutoCapitalization.isChecked = settings.autoCapitalization
        panelSettingsNumberRow.isChecked = settings.numberRow
        panelSettingsHapticFeedback.isChecked = settings.hapticFeedback
        syncingSettingsControls = false
        panelSettingsStatus.text = getString(R.string.setup_settings_loaded)
    }

    private fun refreshSetupStatus() {
        val inputMethodManager = getSystemService(InputMethodManager::class.java)
        val serviceComponent = ComponentName(this, SuzakuInputMethodService::class.java)
        val serviceId = serviceComponent.flattenToString()
        val enabledServiceIds = inputMethodManager
            ?.enabledInputMethodList
            .orEmpty()
            .map { info ->
                ComponentName(info.serviceInfo.packageName, info.serviceInfo.name).flattenToString()
            }
        val selectedServiceId = Settings.Secure.getString(
            contentResolver,
            Settings.Secure.DEFAULT_INPUT_METHOD,
        )?.let(ComponentName::unflattenFromString)?.flattenToString()
        val status = SuzakuImeSetupStatus.resolve(
            serviceId = serviceId,
            enabledServiceIds = enabledServiceIds,
            selectedServiceId = selectedServiceId,
        )

        setupStatus.text = getString(
            when (status.state) {
                SuzakuImeSetupState.DISABLED -> R.string.setup_status_disabled
                SuzakuImeSetupState.ENABLED -> R.string.setup_status_enabled
                SuzakuImeSetupState.ACTIVE -> R.string.setup_status_active
            }
        )
        showImePickerButton.isEnabled = status.enabled || status.selected
        showImePickerButton.alpha = if (showImePickerButton.isEnabled) 1f else 0.45f
    }

    private fun refreshDiagnostics() {

        findViewById<TextView>(R.id.bootstrapStatus).text =
            runCatching {
                buildString {
                    append(SuzakuNativeBridge.nativeDescribeImeHost())
                    append("\n\n")
                    append(SuzakuNativeBridge.nativeDescribeBootstrap())
                }
            }.getOrElse { "Android bootstrap unavailable" }

        findViewById<TextView>(R.id.panelStatus).text =
            runCatching {
                buildString {
                    append(SuzakuNativeBridge.nativeRegistrationHint())
                    append("\n")
                    append(SuzakuNativeBridge.nativeDescribeImeDispatch())
                    append("\n")
                    append(SuzakuNativeBridge.nativeDescribePanelCompanion())
                }
            }.getOrElse { "Panel companion unavailable" }
    }
}
