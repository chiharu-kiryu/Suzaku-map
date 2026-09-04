package dev.suzaku.android.ime

import android.animation.LayoutTransition
import android.annotation.SuppressLint
import android.content.ClipDescription
import android.content.ClipboardManager
import android.graphics.Color
import android.inputmethodservice.InputMethodService
import android.os.Build
import android.text.InputType
import android.view.Gravity
import android.view.HapticFeedbackConstants
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.inputmethod.CompletionInfo
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.Button
import android.widget.EditText
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.TextView
import androidx.appcompat.widget.SwitchCompat

private enum class DrawerMode { KEYBOARD, VOICE, HANDWRITE, CLIPBOARD, SETTINGS }
private const val MAX_EDITOR_COMPLETIONS = 6

private data class ClipboardRead(
    val text: String?,
    val sensitive: Boolean,
    val hasContent: Boolean,
)

private enum class CandidateRenderMode {
    SECURE,
    DIRECT,
    EDITOR_WAITING,
    EDITOR_COMPLETIONS,
    EMPTY,
    NATIVE,
}

private data class CandidateRenderState(
    val mode: CandidateRenderMode,
    val labels: List<String>,
    val selectedIndex: Int = -1,
)

private data class CandidateChipBinding(
    val text: String,
    val primary: Boolean,
    val selected: Boolean,
    val index: Int,
    val continuous: Boolean,
)

private data class DrawerRenderState(
    val mode: DrawerMode,
    val panelExpanded: Boolean,
    val secureInput: Boolean,
    val suggestionsEnabled: Boolean,
    val voicePhase: SuzakuVoicePhase,
    val voiceListening: Boolean,
    val voiceAvailable: Boolean,
    val clipboardCanPaste: Boolean,
    val selectedHandwriteSeed: String?,
    val handwriteStatus: String,
)

class SuzakuInputMethodService : InputMethodService() {
    private lateinit var composePreview: TextView
    private lateinit var hostStatus: TextView
    private lateinit var candidateStrip: LinearLayout
    private lateinit var keyboardDrawer: View
    private lateinit var voiceDrawer: View
    private lateinit var handwriteDrawer: View
    private lateinit var clipboardDrawer: View
    private lateinit var settingsDrawer: View
    private lateinit var keyboardRows: LinearLayout
    private lateinit var voiceStatus: TextView
    private lateinit var voiceLevelRow: LinearLayout
    private lateinit var voiceTranscriptInput: EditText
    private lateinit var handwriteStatus: TextView
    private lateinit var handwriteCanvas: SuzakuHandwriteCanvasView
    private lateinit var handwriteCandidateStrip: LinearLayout
    private lateinit var clipboardPreview: TextView
    private lateinit var clipboardStatus: TextView
    private lateinit var toolKeyboard: ImageButton
    private lateinit var toolVoice: ImageButton
    private lateinit var toolHandwrite: ImageButton
    private lateinit var toolClipboard: ImageButton
    private lateinit var toolSettings: ImageButton
    private lateinit var toolCollapse: ImageButton
    private lateinit var voiceListenButton: Button
    private lateinit var voiceUseTranscriptButton: Button
    private lateinit var voiceCommitButton: Button
    private lateinit var voiceClearButton: Button
    private lateinit var handwriteApplyButton: Button
    private lateinit var handwriteClearButton: Button
    private lateinit var clipboardPasteButton: Button
    private lateinit var clipboardRefreshButton: Button
    private lateinit var settingsAutoCapitalization: SwitchCompat
    private lateinit var settingsNumberRow: SwitchCompat
    private lateinit var settingsHapticFeedback: SwitchCompat
    private lateinit var settingsStatus: TextView
    private lateinit var settingsDiagnosticsButton: Button
    private lateinit var settingsResetButton: Button
    private lateinit var imePanel: View
    private lateinit var compactBubbleShell: View
    private lateinit var compactBubbleButton: ImageButton
    private lateinit var compactBubbleDot: View
    private var drawerMode = DrawerMode.KEYBOARD
    private var imePanelExpanded = false
    private var selectedHandwriteSeed: String? = null
    private var voicePhase = SuzakuVoicePhase.IDLE
    private var editorPolicy = SuzakuEditorPolicy.from(InputType.TYPE_CLASS_TEXT)
    private var editorCompletions: List<CompletionInfo> = emptyList()
    private var lastCandidateRenderState: CandidateRenderState? = null
    private var lastDrawerRenderState: DrawerRenderState? = null
    private var cachedHostDescription: String? = null
    private var nativeRenderSnapshotAvailable = true
    private var clipboardSnapshot = SuzakuClipboardPolicy.inspect(null, secureInput = false)
    private var imeSettings = SuzakuImeSettings()
    private var syncingSettingsControls = false
    private val keyboardState = SuzakuKeyboardState()
    private var lastKeyboardRows: List<List<SuzakuSoftKey>>? = null
    private val backspaceRepeatState = SuzakuKeyRepeatState()
    private var backspaceRepeatView: View? = null
    private val backspaceRepeatRunnable = object : Runnable {
        override fun run() {
            val view = backspaceRepeatView ?: return
            val nextDelay = backspaceRepeatState.tick() ?: return
            if (imeSettings.hapticFeedback) {
                view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
            }
            deleteLastCharacter()
            refreshImeUi()
            view.postDelayed(this, nextDelay)
        }
    }
    private val softKeyClickListener = View.OnClickListener { view ->
        val key = view.tag as? SuzakuSoftKey ?: return@OnClickListener
        if (imeSettings.hapticFeedback) {
            view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
        }
        handleSoftKeyPress(key)
    }
    private val nativeCandidateClickListener = View.OnClickListener { view ->
        val index = (view.tag as? CandidateChipBinding)?.index ?: return@OnClickListener
        if (index < 0) {
            return@OnClickListener
        }
        SuzakuNativeBridge.nativeSelectCandidate(index)
        commitSelected(force = true)
        refreshImeUi()
    }
    private lateinit var settingsStore: SuzakuImeSettingsStore
    private lateinit var panelAnimator: SuzakuImePanelAnimator
    private lateinit var voiceLevelMeter: SuzakuVoiceLevelMeter
    private lateinit var voiceRecognizer: SuzakuVoiceRecognizer
    private lateinit var clipboardManager: ClipboardManager
    private val clipboardChangedListener = ClipboardManager.OnPrimaryClipChangedListener {
        if (
            ::clipboardPreview.isInitialized &&
            imePanelExpanded &&
            drawerMode == DrawerMode.CLIPBOARD
        ) {
            refreshClipboardDrawer()
        }
    }

    override fun onCreate() {
        super.onCreate()
        settingsStore = SuzakuImeSettingsStore(this)
        imeSettings = settingsStore.load()
        keyboardState.setNumberRowEnabled(imeSettings.numberRow)
        clipboardManager = getSystemService(ClipboardManager::class.java)
        clipboardManager.addPrimaryClipChangedListener(clipboardChangedListener)
    }

    override fun onCreateInputView(): View {
        val root = LayoutInflater.from(this).inflate(R.layout.input_view, null, false)
        setCandidatesViewShown(false)
        bindViews(root)
        configureKeyboardRows(keyboardRows)
        configureToolButtons()
        configureVoiceDrawer()
        configureHandwriteDrawer()
        configureClipboardDrawer()
        configureSettingsDrawer()
        collapseIme()
        refreshImeUi()
        return root
    }
    override fun onStartInput(attribute: EditorInfo?, restarting: Boolean) {
        super.onStartInput(attribute, restarting)
        stopBackspaceRepeat(updateAutomaticShift = false)
        imeSettings = settingsStore.load()
        if (::settingsAutoCapitalization.isInitialized) {
            syncSettingsControls()
        }
        setCandidatesViewShown(false)
        if (!restarting) {
            editorCompletions = emptyList()
            SuzakuNativeBridge.nativeClearMarkedText()
        }
        SuzakuNativeBridge.nativeActivateSession()
        resetKeyboardForEditor(attribute)
        if (::imePanel.isInitialized) {
            collapseIme()
            refreshImeUi()
        }
    }
    override fun onStartInputView(attribute: EditorInfo?, restarting: Boolean) {
        super.onStartInputView(attribute, restarting)
        stopBackspaceRepeat(updateAutomaticShift = false)
        setCandidatesViewShown(false)
        if (!restarting) {
            SuzakuNativeBridge.nativeClearMarkedText()
        }
        SuzakuNativeBridge.nativeActivateSession()
        resetKeyboardForEditor(attribute)
        collapseIme()
        refreshImeUi()
    }
    override fun onFinishInput() {
        super.onFinishInput()
        if (::voiceRecognizer.isInitialized) {
            voiceRecognizer.stop()
        }
        stopBackspaceRepeat(updateAutomaticShift = false)
        editorCompletions = emptyList()
        clearClipboardPreview()
        SuzakuNativeBridge.nativeClearMarkedText()
        SuzakuNativeBridge.nativeDeactivateSession()
    }
    override fun onDestroy() {
        stopBackspaceRepeat(updateAutomaticShift = false)
        editorCompletions = emptyList()
        if (::clipboardManager.isInitialized) {
            clipboardManager.removePrimaryClipChangedListener(clipboardChangedListener)
        }
        if (::voiceRecognizer.isInitialized) voiceRecognizer.destroy()
        super.onDestroy()
    }

    override fun onDisplayCompletions(completions: Array<out CompletionInfo>?) {
        super.onDisplayCompletions(completions)
        editorCompletions = if (editorPolicy.editorCompletionsEnabled) {
            completions.orEmpty()
                .filter { it.text?.isNotBlank() == true }
                .take(MAX_EDITOR_COMPLETIONS)
        } else {
            emptyList()
        }
        lastCandidateRenderState = null
        if (::imePanel.isInitialized) {
            refreshImeUi()
        }
    }

    override fun onUpdateSelection(
        oldSelStart: Int,
        oldSelEnd: Int,
        newSelStart: Int,
        newSelEnd: Int,
        candidatesStart: Int,
        candidatesEnd: Int,
    ) {
        super.onUpdateSelection(
            oldSelStart,
            oldSelEnd,
            newSelStart,
            newSelEnd,
            candidatesStart,
            candidatesEnd,
        )
        if (editorPolicy.suggestionsEnabled) {
            val hasComposition = SuzakuNativeBridge.nativeDisplayText().isNotEmpty()
            if (
                SuzakuCompositionSelectionPolicy.shouldCancel(
                    hasComposition = hasComposition,
                    newSelectionStart = newSelStart,
                    newSelectionEnd = newSelEnd,
                    composingStart = candidatesStart,
                    composingEnd = candidatesEnd,
                )
            ) {
                SuzakuNativeBridge.nativeClearMarkedText()
                selectedHandwriteSeed = null
                currentInputConnection?.finishComposingText()
                if (::imePanel.isInitialized) {
                    refreshImeUi()
                }
            }
        }
        if (::keyboardRows.isInitialized && !backspaceRepeatState.isActive) {
            refreshAutomaticShift()
        }
    }
    private fun bindViews(root: View) {
        imePanel = root.findViewById(R.id.imePanel)
        compactBubbleShell = root.findViewById(R.id.compactBubbleShell)
        compactBubbleButton = root.findViewById(R.id.compactBubbleButton)
        compactBubbleDot = root.findViewById(R.id.compactBubbleDot)
        composePreview = root.findViewById(R.id.composePreview)
        hostStatus = root.findViewById(R.id.hostStatus)
        candidateStrip = root.findViewById(R.id.candidateStrip)
        keyboardDrawer = root.findViewById(R.id.keyboardDrawer)
        voiceDrawer = root.findViewById(R.id.voiceDrawer)
        handwriteDrawer = root.findViewById(R.id.handwriteDrawer)
        clipboardDrawer = root.findViewById(R.id.clipboardDrawer)
        settingsDrawer = root.findViewById(R.id.settingsDrawer)
        keyboardRows = root.findViewById(R.id.keyboardRows)
        voiceStatus = root.findViewById(R.id.voiceStatus)
        voiceLevelRow = root.findViewById(R.id.voiceLevelRow)
        voiceTranscriptInput = root.findViewById(R.id.voiceTranscriptInput)
        handwriteStatus = root.findViewById(R.id.handwriteStatus)
        handwriteCanvas = root.findViewById(R.id.handwriteCanvas)
        handwriteCandidateStrip = root.findViewById(R.id.handwriteCandidateStrip)
        clipboardPreview = root.findViewById(R.id.clipboardPreview)
        clipboardStatus = root.findViewById(R.id.clipboardStatus)
        toolKeyboard = root.findViewById(R.id.toolKeyboard)
        toolVoice = root.findViewById(R.id.toolVoice)
        toolHandwrite = root.findViewById(R.id.toolHandwrite)
        toolClipboard = root.findViewById(R.id.toolClipboard)
        toolSettings = root.findViewById(R.id.toolSettings)
        toolCollapse = root.findViewById(R.id.toolCollapse)
        voiceListenButton = root.findViewById(R.id.voiceListenButton)
        voiceUseTranscriptButton = root.findViewById(R.id.voiceUseTranscriptButton)
        voiceCommitButton = root.findViewById(R.id.voiceCommitButton)
        voiceClearButton = root.findViewById(R.id.voiceClearButton)
        handwriteApplyButton = root.findViewById(R.id.handwriteApplyButton)
        handwriteClearButton = root.findViewById(R.id.handwriteClearButton)
        clipboardPasteButton = root.findViewById(R.id.clipboardPasteButton)
        clipboardRefreshButton = root.findViewById(R.id.clipboardRefreshButton)
        settingsAutoCapitalization = root.findViewById(R.id.settingsAutoCapitalization)
        settingsNumberRow = root.findViewById(R.id.settingsNumberRow)
        settingsHapticFeedback = root.findViewById(R.id.settingsHapticFeedback)
        settingsStatus = root.findViewById(R.id.settingsStatus)
        settingsDiagnosticsButton = root.findViewById(R.id.settingsDiagnosticsButton)
        settingsResetButton = root.findViewById(R.id.settingsResetButton)
        voiceLevelMeter = SuzakuVoiceLevelMeter(
            voiceLevelRow,
            listOf(
                root.findViewById(R.id.voiceLevelBar1),
                root.findViewById(R.id.voiceLevelBar2),
                root.findViewById(R.id.voiceLevelBar3),
                root.findViewById(R.id.voiceLevelBar4),
            ),
        )
        panelAnimator = SuzakuImePanelAnimator(imePanel, compactBubbleShell)
        lastCandidateRenderState = null
        lastDrawerRenderState = null
        lastKeyboardRows = null
        handwriteCandidateStrip.layoutTransition = buildStripTransition()
    }
    private fun configureToolButtons() {
        styleToolButton(toolKeyboard)
        styleToolButton(toolVoice)
        styleToolButton(toolHandwrite)
        styleToolButton(toolClipboard)
        styleToolButton(toolSettings)
        styleToolButton(toolCollapse)
        styleToolButton(compactBubbleButton)
        toolKeyboard.setOnClickListener { expandIme(DrawerMode.KEYBOARD) }
        toolVoice.setOnClickListener { expandIme(DrawerMode.VOICE) }
        toolHandwrite.setOnClickListener { expandIme(DrawerMode.HANDWRITE) }
        toolClipboard.setOnClickListener { expandIme(DrawerMode.CLIPBOARD) }
        toolSettings.setOnClickListener { expandIme(DrawerMode.SETTINGS) }
        toolCollapse.setOnClickListener { collapseIme() }
        compactBubbleButton.setOnClickListener { expandIme(drawerMode) }
    }

    private fun configureVoiceDrawer() {
        styleActionButton(voiceListenButton)
        styleActionButton(voiceUseTranscriptButton)
        styleActionButton(voiceCommitButton)
        styleActionButton(voiceClearButton)
        voiceRecognizer = SuzakuVoiceRecognizer(
            context = this,
            onStatus = { status ->
                voiceStatus.text = status
                updateVoiceListenButton()
            },
            onTranscript = secureTranscript@ { transcript ->
                if (editorPolicy.secureInput) {
                    return@secureTranscript
                }
                voiceTranscriptInput.setText(transcript)
                voiceTranscriptInput.setSelection(transcript.length)
                if (transcript.isNotBlank() && editorPolicy.suggestionsEnabled) {
                    SuzakuNativeBridge.nativeReplaceMarkedText(transcript)
                    refreshImeUi()
                }
            },
            onPhaseChanged = { phase ->
                voicePhase = phase
                syncVoiceStatusFromPhase()
                refreshVoiceLevelMeter()
            },
        )

        voiceListenButton.setOnClickListener {
            if (editorPolicy.secureInput) {
                return@setOnClickListener
            }
            if (voiceRecognizer.isListening) {
                voiceRecognizer.stop()
            } else {
                voiceRecognizer.start()
            }
            updateVoiceListenButton()
            refreshVoiceLevelMeter()
        }
        voiceUseTranscriptButton.setOnClickListener {
            if (editorPolicy.secureInput) {
                return@setOnClickListener
            }
            val transcript = voiceTranscriptInput.text?.toString()?.trim().orEmpty()
            if (transcript.isNotEmpty()) {
                if (editorPolicy.suggestionsEnabled) {
                    SuzakuNativeBridge.nativeReplaceMarkedText(transcript)
                } else {
                    currentInputConnection?.commitText(transcript, 1)
                }
                voiceStatus.text = getString(R.string.voice_status_applied)
                refreshImeUi()
            }
        }
        voiceCommitButton.setOnClickListener {
            if (commitSelected(force = true)) {
                voiceStatus.text = getString(R.string.voice_status_committed)
                refreshImeUi()
            }
        }
        voiceClearButton.setOnClickListener {
            voiceRecognizer.stop()
            voiceTranscriptInput.setText("")
            SuzakuNativeBridge.nativeClearMarkedText()
            voicePhase = SuzakuVoicePhase.IDLE
            syncVoiceStatusFromPhase()
            currentInputConnection?.finishComposingText()
            updateVoiceListenButton()
            refreshVoiceLevelMeter()
            refreshImeUi()
        }
        syncVoiceStatusFromPhase()
        updateVoiceListenButton()
        refreshVoiceLevelMeter()
    }
    private fun configureHandwriteDrawer() {
        styleActionButton(handwriteApplyButton, compact = true)
        styleActionButton(handwriteClearButton, compact = true)
        handwriteCanvas.onStrokeStarted = {
            if (!editorPolicy.secureInput) {
                handwriteStatus.text = getString(R.string.handwrite_status_writing)
            }
        }
        handwriteCanvas.onStrokeFinished = secureStroke@ { strokes ->
            if (editorPolicy.secureInput) {
                return@secureStroke
            }
            handwriteStatus.text = getString(R.string.handwrite_status_recognizing)
            handwriteCanvas.post {
                if (editorPolicy.secureInput) {
                    return@post
                }
                val seeds = SuzakuHandwriteRecognizer.candidateSeeds(strokes)
                refreshHandwriteCandidates(seeds)
                if (seeds.isNotEmpty()) {
                    applyHandwriteSeed(seeds.first(), auto = true, totalSeeds = seeds.size)
                }
            }
        }
        handwriteApplyButton.setOnClickListener {
            if (editorPolicy.secureInput) {
                return@setOnClickListener
            }
            selectedHandwriteSeed?.let { seed ->
                applyHandwriteSeed(seed, auto = false, totalSeeds = handwriteCandidateStrip.childCount)
                if (editorPolicy.suggestionsEnabled) {
                    commitSelected(force = true)
                } else {
                    currentInputConnection?.commitText(seed, 1)
                    selectedHandwriteSeed = null
                }
                handwriteStatus.text = getString(R.string.handwrite_status_committed)
                refreshImeUi()
            }
        }
        handwriteClearButton.setOnClickListener {
            handwriteCanvas.clearCanvas()
            selectedHandwriteSeed = null
            handwriteCandidateStrip.removeAllViews()
            handwriteStatus.text = getString(R.string.handwrite_status_idle)
        }
    }

    private fun configureClipboardDrawer() {
        styleActionButton(clipboardRefreshButton, compact = true)
        styleActionButton(clipboardPasteButton, compact = true)
        clipboardRefreshButton.setOnClickListener { refreshClipboardDrawer() }
        clipboardPasteButton.setOnClickListener { pasteClipboard() }
        clearClipboardPreview()
    }

    private fun configureSettingsDrawer() {
        styleActionButton(settingsDiagnosticsButton, compact = true)
        styleActionButton(settingsResetButton, compact = true)
        syncSettingsControls()

        settingsAutoCapitalization.setOnCheckedChangeListener { _, enabled ->
            if (!syncingSettingsControls) {
                applyImeSettings(imeSettings.copy(autoCapitalization = enabled))
            }
        }
        settingsNumberRow.setOnCheckedChangeListener { _, enabled ->
            if (!syncingSettingsControls) {
                applyImeSettings(imeSettings.copy(numberRow = enabled))
            }
        }
        settingsHapticFeedback.setOnCheckedChangeListener { _, enabled ->
            if (!syncingSettingsControls) {
                applyImeSettings(imeSettings.copy(hapticFeedback = enabled))
            }
        }
        settingsDiagnosticsButton.setOnClickListener {
            settingsStatus.text = buildString {
                append(SuzakuNativeBridge.nativeRegistrationHint())
                append(" · ")
                append(SuzakuNativeBridge.nativeRegistrationTarget())
                append(" · ready=")
                append(SuzakuNativeBridge.nativeRegistrationReady())
            }
        }
        settingsResetButton.setOnClickListener {
            imeSettings = settingsStore.reset()
            keyboardState.setNumberRowEnabled(imeSettings.numberRow)
            syncSettingsControls()
            keyboardState.applyAutomaticShift(false)
            refreshAutomaticShift()
            configureKeyboardRows(keyboardRows)
            settingsStatus.text = getString(R.string.settings_defaults_restored)
        }
    }

    private fun applyImeSettings(updated: SuzakuImeSettings) {
        val previous = imeSettings
        imeSettings = updated
        settingsStore.save(updated)
        val numberRowChanged = keyboardState.setNumberRowEnabled(updated.numberRow)

        if (numberRowChanged) {
            configureKeyboardRows(keyboardRows)
        }
        if (previous.autoCapitalization != updated.autoCapitalization) {
            if (updated.autoCapitalization) {
                refreshAutomaticShift()
            } else if (keyboardState.applyAutomaticShift(false)) {
                configureKeyboardRows(keyboardRows)
            }
        }
        settingsStatus.text = getString(R.string.settings_saved)
    }

    private fun syncSettingsControls() {
        syncingSettingsControls = true
        settingsAutoCapitalization.isChecked = imeSettings.autoCapitalization
        settingsNumberRow.isChecked = imeSettings.numberRow
        settingsHapticFeedback.isChecked = imeSettings.hapticFeedback
        syncingSettingsControls = false
    }

    private fun configureKeyboardRows(container: LinearLayout) {
        stopBackspaceRepeat(updateAutomaticShift = false)
        val rows = keyboardState.rows()
        if (rows == lastKeyboardRows) {
            return
        }

        container.suppressLayout(true)
        try {
            if (canReuseKeyboardRows(container, rows)) {
                rows.forEachIndexed { rowIndex, keys ->
                    val row = container.getChildAt(rowIndex) as LinearLayout
                    keys.forEachIndexed { keyIndex, key ->
                        bindKeyboardButton(row.getChildAt(keyIndex) as Button, key)
                    }
                }
            } else {
                container.removeAllViews()
                rows.forEach { row -> container.addView(buildKeyboardRow(row)) }
            }
            lastKeyboardRows = rows
        } finally {
            container.suppressLayout(false)
            container.requestLayout()
        }
    }

    private fun canReuseKeyboardRows(
        container: LinearLayout,
        rows: List<List<SuzakuSoftKey>>,
    ): Boolean {
        if (container.childCount != rows.size) {
            return false
        }
        return rows.indices.all { rowIndex ->
            val row = container.getChildAt(rowIndex) as? LinearLayout ?: return@all false
            row.childCount == rows[rowIndex].size &&
                (0 until row.childCount).all { row.getChildAt(it) is Button }
        }
    }

    private fun buildKeyboardRow(keys: List<SuzakuSoftKey>): View {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(6) }
        }

        keys.forEach { key ->
            row.addView(Button(this).also { bindKeyboardButton(it, key) })
        }

        return row
    }

    // Reused Button instances keep normal click accessibility; touch handling is only swapped
    // in for backspace hold-to-repeat and removed again when a slot changes action.
    @SuppressLint("ClickableViewAccessibility")
    private fun bindKeyboardButton(button: Button, key: SuzakuSoftKey) {
        val previousKey = button.tag as? SuzakuSoftKey
        val params = button.layoutParams as? LinearLayout.LayoutParams
        if (params == null) {
            button.layoutParams = LinearLayout.LayoutParams(0, dp(46), key.weight).apply {
                marginEnd = dp(6)
            }
        } else if (params.weight != key.weight) {
            params.weight = key.weight
            button.layoutParams = params
        }
        setTextIfChanged(button, key.label)
        if (button.contentDescription != key.accessibilityLabel) {
            button.contentDescription = key.accessibilityLabel
        }
        button.tag = key
        if (previousKey == null) {
            button.isAllCaps = false
            button.minHeight = dp(46)
            button.setOnClickListener(softKeyClickListener)
        }
        if (previousKey == null || previousKey.active != key.active) {
            button.setBackgroundResource(
                if (key.active) R.drawable.candidate_chip_selected else R.drawable.gboard_key_bg
            )
            button.setTextColor(Color.parseColor(if (key.active) "#123A63" else "#24476C"))
        }
        val textSize = if (
            key.action == SuzakuSoftKeyAction.INSERT ||
            key.action == SuzakuSoftKeyAction.COMMIT_LITERAL
        ) 19f else 16f
        val previousUsesLargeText = previousKey?.action == SuzakuSoftKeyAction.INSERT ||
            previousKey?.action == SuzakuSoftKeyAction.COMMIT_LITERAL
        val nextUsesLargeText = key.action == SuzakuSoftKeyAction.INSERT ||
            key.action == SuzakuSoftKeyAction.COMMIT_LITERAL
        if (previousKey == null || previousUsesLargeText != nextUsesLargeText) {
            button.textSize = textSize
        }

        if (previousKey?.action != key.action) {
            button.setOnTouchListener(null)
            button.setOnLongClickListener(null)
            button.isLongClickable = false
            when (key.action) {
                SuzakuSoftKeyAction.BACKSPACE -> configureBackspaceRepeat(button)
                SuzakuSoftKeyAction.SWITCH_INPUT_METHOD -> {
                    button.setOnLongClickListener {
                        showInputMethodPicker()
                        true
                    }
                }
                else -> Unit
            }
        }
    }

    // Quick taps delegate to performClick(); the touch listener only adds hold-to-repeat timing.
    @SuppressLint("ClickableViewAccessibility")
    private fun configureBackspaceRepeat(button: Button) {
        button.setOnTouchListener { view, event ->
            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    view.isPressed = true
                    startBackspaceRepeat(view)
                    true
                }
                MotionEvent.ACTION_MOVE -> {
                    val outside = event.x < 0f || event.y < 0f ||
                        event.x >= view.width.toFloat() || event.y >= view.height.toFloat()
                    if (outside) {
                        view.isPressed = false
                        stopBackspaceRepeat()
                    }
                    true
                }
                MotionEvent.ACTION_UP -> {
                    val shouldClick = backspaceRepeatState.isActive &&
                        !backspaceRepeatState.hasRepeated
                    view.isPressed = false
                    stopBackspaceRepeat()
                    if (shouldClick) {
                        view.performClick()
                    }
                    true
                }
                MotionEvent.ACTION_CANCEL, MotionEvent.ACTION_OUTSIDE -> {
                    view.isPressed = false
                    stopBackspaceRepeat()
                    true
                }
                else -> true
            }
        }
    }

    private fun startBackspaceRepeat(view: View) {
        stopBackspaceRepeat(updateAutomaticShift = false)
        backspaceRepeatView = view
        view.postDelayed(backspaceRepeatRunnable, backspaceRepeatState.begin())
    }

    private fun stopBackspaceRepeat(updateAutomaticShift: Boolean = true): Boolean {
        backspaceRepeatView?.removeCallbacks(backspaceRepeatRunnable)
        backspaceRepeatView = null
        val repeated = backspaceRepeatState.finish()
        if (repeated && updateAutomaticShift) {
            refreshAutomaticShift()
        }
        return repeated
    }

    private fun handleSoftKeyPress(key: SuzakuSoftKey) {
        var textChanged = false
        when (val effect = keyboardState.press(key)) {
            is SuzakuKeyboardEffect.Insert -> {
                appendCharacter(effect.text)
                textChanged = true
                if (effect.keysChanged) {
                    configureKeyboardRows(keyboardRows)
                }
            }
            is SuzakuKeyboardEffect.CommitLiteral -> {
                commitSelected(force = true)
                currentInputConnection?.commitText(effect.text, 1)
                textChanged = true
            }
            SuzakuKeyboardEffect.Backspace -> {
                deleteLastCharacter()
                textChanged = true
            }
            SuzakuKeyboardEffect.Enter -> {
                handleEnter()
                textChanged = true
            }
            SuzakuKeyboardEffect.Space -> {
                commitSelected(force = true)
                currentInputConnection?.commitText(" ", 1)
                textChanged = true
            }
            SuzakuKeyboardEffect.SwitchInputMethod -> switchInputMethod()
            SuzakuKeyboardEffect.KeysChanged -> configureKeyboardRows(keyboardRows)
        }
        refreshImeUi()
        if (textChanged) {
            refreshAutomaticShift()
        }
    }

    private fun switchInputMethod() {
        val switched = shouldOfferSwitchingToNextInputMethod() &&
            switchToNextInputMethod(false)
        if (!switched) {
            showInputMethodPicker()
        }
    }

    private fun showInputMethodPicker() {
        getSystemService(InputMethodManager::class.java)?.showInputMethodPicker()
    }

    private fun handleEnter() {
        commitSelected(force = true)
        val editorAction = editorPolicy.editorAction
        val hasEditorAction = editorAction != EditorInfo.IME_ACTION_NONE &&
            editorAction != EditorInfo.IME_ACTION_UNSPECIFIED
        if (hasEditorAction && currentInputConnection?.performEditorAction(editorAction) == true) {
            return
        }
        currentInputConnection?.commitText("\n", 1)
    }

    private fun appendCharacter(key: String) {
        if (!editorPolicy.suggestionsEnabled) {
            currentInputConnection?.commitText(key, 1)
            return
        }
        val current = SuzakuNativeBridge.nativeDisplayText()
        SuzakuNativeBridge.nativeReplaceMarkedText(current + key)
    }

    private fun deleteLastCharacter() {
        if (!editorPolicy.suggestionsEnabled) {
            currentInputConnection?.deleteSurroundingTextInCodePoints(1, 0)
            return
        }
        val current = SuzakuNativeBridge.nativeDisplayText()
        if (current.isEmpty()) {
            currentInputConnection?.deleteSurroundingTextInCodePoints(1, 0)
            return
        }

        val updated = dropLastTextElement(current)
        if (updated.isEmpty()) {
            SuzakuNativeBridge.nativeClearMarkedText()
            currentInputConnection?.finishComposingText()
        } else {
            SuzakuNativeBridge.nativeReplaceMarkedText(updated)
        }
    }

    private fun refreshClipboardDrawer() {
        val clipboard = readClipboard()
        clipboardSnapshot = SuzakuClipboardPolicy.inspect(
            text = clipboard.text,
            secureInput = editorPolicy.secureInput,
            sensitiveContent = clipboard.sensitive,
            contentAvailable = clipboard.hasContent,
        )
        renderClipboardSnapshot(clipboardSnapshot)
    }

    private fun pasteClipboard() {
        val clipboard = readClipboard(includeSensitiveText = true)
        val snapshot = SuzakuClipboardPolicy.inspect(
            text = clipboard.text,
            secureInput = editorPolicy.secureInput,
            sensitiveContent = clipboard.sensitive,
            contentAvailable = clipboard.hasContent,
        )
        clipboardSnapshot = snapshot
        renderClipboardSnapshot(snapshot)
        val text = clipboard.text
        if (!snapshot.canPaste || text.isNullOrEmpty()) {
            return
        }
        val connection = currentInputConnection ?: return

        connection.beginBatchEdit()
        val pasted = try {
            commitSelected(force = true)
            editorCompletions = emptyList()
            selectedHandwriteSeed = null
            connection.commitText(text, 1)
        } finally {
            connection.endBatchEdit()
        }
        renderClipboardSnapshot(
            snapshot,
            statusText = getString(
                if (pasted) R.string.clipboard_pasted else R.string.clipboard_paste_failed
            ),
        )
        refreshImeUi()
        refreshAutomaticShift()
    }

    private fun readClipboard(
        includeSensitiveText: Boolean = false,
    ): ClipboardRead = runCatching {
        val clip = clipboardManager.primaryClip
            ?: return@runCatching ClipboardRead(
                text = null,
                sensitive = false,
                hasContent = false,
            )
        if (clip.itemCount == 0) {
            return@runCatching ClipboardRead(
                text = null,
                sensitive = false,
                hasContent = false,
            )
        }
        val sensitive = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            clip.description.extras?.getBoolean(
                ClipDescription.EXTRA_IS_SENSITIVE,
                false,
            ) == true
        val text = if (sensitive && !includeSensitiveText) {
            null
        } else {
            clip.getItemAt(0).coerceToText(this)?.toString()
        }
        ClipboardRead(
            text = text,
            sensitive = sensitive,
            hasContent = if (sensitive && !includeSensitiveText) {
                true
            } else {
                !text.isNullOrEmpty()
            },
        )
    }.getOrDefault(
        ClipboardRead(
            text = null,
            sensitive = false,
            hasContent = false,
        )
    )

    private fun renderClipboardSnapshot(
        snapshot: SuzakuClipboardSnapshot,
        statusText: String = getString(R.string.clipboard_privacy_note),
    ) {
        if (!::clipboardPreview.isInitialized) {
            return
        }
        clipboardPreview.text = when (snapshot.availability) {
            SuzakuClipboardAvailability.BLOCKED -> getString(R.string.clipboard_secure)
            SuzakuClipboardAvailability.EMPTY -> getString(R.string.clipboard_empty)
            SuzakuClipboardAvailability.SENSITIVE -> getString(R.string.clipboard_sensitive)
            SuzakuClipboardAvailability.READY -> snapshot.preview
                ?: getString(R.string.clipboard_whitespace)
        }
        clipboardStatus.text = statusText
        clipboardPasteButton.isEnabled = snapshot.canPaste
        clipboardPasteButton.alpha = if (snapshot.canPaste) 1f else 0.45f
    }

    private fun clearClipboardPreview() {
        clipboardSnapshot = SuzakuClipboardPolicy.inspect(
            text = null,
            secureInput = editorPolicy.secureInput,
        )
        renderClipboardSnapshot(clipboardSnapshot)
    }

    private fun resetKeyboardForEditor(attribute: EditorInfo?) {
        editorPolicy = SuzakuEditorPolicy.from(
            inputType = attribute?.inputType ?: InputType.TYPE_CLASS_TEXT,
            imeOptions = attribute?.imeOptions ?: EditorInfo.IME_ACTION_NONE,
        )
        if (!editorPolicy.editorCompletionsEnabled) {
            editorCompletions = emptyList()
        }
        if (!editorPolicy.suggestionsEnabled) {
            enterDirectInputMode()
        }
        if (editorPolicy.secureInput) {
            enterSecureInputMode()
        }
        val capsMode = currentInputConnection?.getCursorCapsMode(editorPolicy.inputType)
            ?: editorPolicy.capitalizationFlags
        val capitalize = imeSettings.autoCapitalization &&
            capsMode.and(editorPolicy.capitalizationFlags) != 0
        keyboardState.setNumberRowEnabled(imeSettings.numberRow)
        keyboardState.reset(
            profile = editorPolicy.keyboardProfile,
            capitalize = capitalize,
            signedNumbers = editorPolicy.signedNumbers,
            decimalNumbers = editorPolicy.decimalNumbers,
            enterKey = editorPolicy.enterKey,
        )
        if (::keyboardRows.isInitialized) {
            configureKeyboardRows(keyboardRows)
        }
    }

    private fun refreshAutomaticShift() {
        if (!imeSettings.autoCapitalization) {
            return
        }
        val capsMode = currentInputConnection?.getCursorCapsMode(editorPolicy.inputType) ?: return
        if (
            keyboardState.applyAutomaticShift(
                capsMode.and(editorPolicy.capitalizationFlags) != 0
            ) &&
            ::keyboardRows.isInitialized
        ) {
            configureKeyboardRows(keyboardRows)
        }
    }

    private fun enterDirectInputMode() {
        SuzakuNativeBridge.nativeClearMarkedText()
        currentInputConnection?.finishComposingText()
    }

    private fun enterSecureInputMode() {
        enterDirectInputMode()
        editorCompletions = emptyList()
        if (::voiceRecognizer.isInitialized) {
            voiceRecognizer.stop()
        }
        if (::voiceTranscriptInput.isInitialized) {
            voiceTranscriptInput.setText("")
        }
        voicePhase = SuzakuVoicePhase.IDLE
        selectedHandwriteSeed = null
        if (::handwriteCanvas.isInitialized) {
            handwriteCanvas.clearCanvas()
        }
        if (::handwriteCandidateStrip.isInitialized) {
            handwriteCandidateStrip.removeAllViews()
        }
        if (
            drawerMode == DrawerMode.VOICE ||
            drawerMode == DrawerMode.HANDWRITE ||
            drawerMode == DrawerMode.CLIPBOARD
        ) {
            drawerMode = DrawerMode.KEYBOARD
        }
        clearClipboardPreview()
    }

    private fun refreshImeUi() {
        val snapshot = if (editorPolicy.suggestionsEnabled) {
            readImeRenderSnapshot()
        } else {
            SuzakuImeRenderSnapshot.EMPTY
        }
        val preview = when {
            editorPolicy.secureInput -> getString(R.string.secure_input_placeholder)
            !editorPolicy.suggestionsEnabled -> getString(R.string.direct_input_placeholder)
            snapshot.displayText.isEmpty() -> getString(R.string.compose_placeholder)
            else -> snapshot.displayText
        }
        setTextIfChanged(composePreview, preview)
        val status = if (editorPolicy.secureInput) {
            getString(R.string.host_status_secure)
        } else {
            cachedHostDescription ?: SuzakuNativeBridge.nativeDescribeImeHost().also {
                cachedHostDescription = it
            }
        }
        setTextIfChanged(hostStatus, status)
        refreshCandidateStrip(candidateStrip, snapshot)
        val hasCandidateActivity = when {
            editorPolicy.editorCompletionsEnabled -> editorCompletions.isNotEmpty()
            editorPolicy.suggestionsEnabled ->
                snapshot.candidateLabels.isNotEmpty() || snapshot.displayText.isNotEmpty()
            else -> false
        }
        setVisibilityIfChanged(
            compactBubbleDot,
            if (hasCandidateActivity) View.VISIBLE else View.GONE,
        )
        refreshDrawerVisibility()
        if (editorPolicy.suggestionsEnabled) {
            syncComposingText(snapshot.displayText)
        }
    }

    private fun readImeRenderSnapshot(): SuzakuImeRenderSnapshot {
        if (nativeRenderSnapshotAvailable) {
            runCatching { SuzakuNativeBridge.nativeRenderSnapshot() }
                .onSuccess { payload ->
                    return SuzakuImeRenderSnapshot.fromNativePayload(payload)
                }
                .onFailure { nativeRenderSnapshotAvailable = false }
        }

        val count = SuzakuNativeBridge.nativeCandidateCount().coerceAtMost(6)
        return SuzakuImeRenderSnapshot(
            displayText = SuzakuNativeBridge.nativeDisplayText(),
            selectedIndex = SuzakuNativeBridge.nativeSelectedIndex(),
            candidateLabels = (0 until count).map(SuzakuNativeBridge::nativeCandidateLabel),
        )
    }

    private fun refreshDrawerVisibility() {
        if (
            editorPolicy.secureInput &&
            (
                drawerMode == DrawerMode.VOICE ||
                    drawerMode == DrawerMode.HANDWRITE ||
                    drawerMode == DrawerMode.CLIPBOARD
            )
        ) {
            drawerMode = DrawerMode.KEYBOARD
        }
        val voiceListening = ::voiceRecognizer.isInitialized && voiceRecognizer.isListening
        if (!voiceListening) {
            voicePhase = if (voiceTranscriptInput.text?.isNotBlank() == true) {
                SuzakuVoicePhase.READY
            } else if (voicePhase == SuzakuVoicePhase.ERROR) {
                SuzakuVoicePhase.ERROR
            } else {
                SuzakuVoicePhase.IDLE
            }
        }
        val nextHandwriteStatus = when {
            selectedHandwriteSeed != null ->
                getString(R.string.handwrite_status_ready, selectedHandwriteSeed)
            handwriteStatus.text == getString(R.string.handwrite_status_writing) ->
                handwriteStatus.text.toString()
            handwriteStatus.text == getString(R.string.handwrite_status_recognizing) ->
                handwriteStatus.text.toString()
            else -> getString(R.string.handwrite_status_idle)
        }
        val state = DrawerRenderState(
            mode = drawerMode,
            panelExpanded = imePanelExpanded,
            secureInput = editorPolicy.secureInput,
            suggestionsEnabled = editorPolicy.suggestionsEnabled,
            voicePhase = voicePhase,
            voiceListening = voiceListening,
            voiceAvailable = ::voiceRecognizer.isInitialized && voiceRecognizer.isAvailable(),
            clipboardCanPaste = clipboardSnapshot.canPaste,
            selectedHandwriteSeed = selectedHandwriteSeed,
            handwriteStatus = nextHandwriteStatus,
        )
        if (state == lastDrawerRenderState) {
            return
        }
        lastDrawerRenderState = state
        setVisibilityIfChanged(
            keyboardDrawer,
            if (drawerMode == DrawerMode.KEYBOARD) View.VISIBLE else View.GONE,
        )
        setVisibilityIfChanged(
            voiceDrawer,
            if (drawerMode == DrawerMode.VOICE) View.VISIBLE else View.GONE,
        )
        setVisibilityIfChanged(
            handwriteDrawer,
            if (drawerMode == DrawerMode.HANDWRITE) View.VISIBLE else View.GONE,
        )
        setVisibilityIfChanged(
            clipboardDrawer,
            if (drawerMode == DrawerMode.CLIPBOARD) View.VISIBLE else View.GONE,
        )
        setVisibilityIfChanged(
            settingsDrawer,
            if (drawerMode == DrawerMode.SETTINGS) View.VISIBLE else View.GONE,
        )
        val multimodalInputEnabled = !editorPolicy.secureInput
        setEnabledState(toolVoice, multimodalInputEnabled, disabledAlpha = 0.38f)
        setEnabledState(toolHandwrite, multimodalInputEnabled, disabledAlpha = 0.38f)
        setEnabledState(toolClipboard, multimodalInputEnabled, disabledAlpha = 0.38f)
        setEnabledState(
            clipboardPasteButton,
            multimodalInputEnabled && clipboardSnapshot.canPaste,
            disabledAlpha = 0.45f,
        )
        setEnabledState(handwriteCanvas, multimodalInputEnabled)
        setEnabledState(handwriteApplyButton, multimodalInputEnabled)
        setEnabledState(handwriteClearButton, multimodalInputEnabled)
        setEnabledState(
            voiceCommitButton,
            multimodalInputEnabled && editorPolicy.suggestionsEnabled,
        )
        syncVoiceStatusFromPhase()
        updateVoiceListenButton()
        refreshVoiceLevelMeter()
        setTextIfChanged(handwriteStatus, nextHandwriteStatus)
    }

    private fun updateVoiceListenButton() {
        if (!::voiceRecognizer.isInitialized) {
            return
        }
        setTextIfChanged(
            voiceListenButton,
            getString(
                if (voiceRecognizer.isListening) R.string.voice_stop else R.string.voice_listen
            ),
        )
        setEnabledState(
            voiceListenButton,
            !editorPolicy.secureInput && voiceRecognizer.isAvailable(),
        )
    }

    private fun refreshCandidateStrip(
        strip: LinearLayout?,
        snapshot: SuzakuImeRenderSnapshot,
    ) {
        strip ?: return
        val state = candidateRenderState(snapshot)
        if (state == lastCandidateRenderState) {
            return
        }
        lastCandidateRenderState = state
        strip.suppressLayout(true)
        try {
            when (state.mode) {
                CandidateRenderMode.EDITOR_COMPLETIONS -> {
                    strip.removeAllViews()
                    refreshEditorCompletions(strip)
                }
                CandidateRenderMode.NATIVE -> renderNativeCandidates(strip, state)
                else -> renderStaticCandidate(strip, state.labels.single())
            }
        } finally {
            strip.suppressLayout(false)
            strip.requestLayout()
        }
    }

    private fun candidateRenderState(snapshot: SuzakuImeRenderSnapshot): CandidateRenderState =
        when {
            editorPolicy.secureInput -> CandidateRenderState(
                CandidateRenderMode.SECURE,
                listOf(getString(R.string.candidate_secure_input)),
            )
            editorPolicy.editorCompletionsEnabled && editorCompletions.isEmpty() ->
                CandidateRenderState(
                    CandidateRenderMode.EDITOR_WAITING,
                    listOf(getString(R.string.candidate_editor_completion_waiting)),
                )
            editorPolicy.editorCompletionsEnabled -> CandidateRenderState(
                CandidateRenderMode.EDITOR_COMPLETIONS,
                editorCompletions.map(::editorCompletionLabel),
            )
            !editorPolicy.suggestionsEnabled -> CandidateRenderState(
                CandidateRenderMode.DIRECT,
                listOf(getString(R.string.candidate_direct_input)),
            )
            snapshot.candidateLabels.isEmpty() -> CandidateRenderState(
                CandidateRenderMode.EMPTY,
                listOf(getString(R.string.candidate_empty)),
            )
            else -> CandidateRenderState(
                CandidateRenderMode.NATIVE,
                snapshot.candidateLabels,
                snapshot.selectedIndex,
            )
        }

    private fun renderNativeCandidates(strip: LinearLayout, state: CandidateRenderState) {
        var childIndex = 0
        state.labels.forEachIndexed { index, label ->
            if (index > 0) {
                ensureCandidateSeparator(strip, childIndex)
                childIndex += 1
            }
            bindCandidateChip(
                ensureCandidateChip(strip, childIndex),
                CandidateChipBinding(
                    text = getString(R.string.candidate_label_format, index + 1, label),
                    primary = index == 0,
                    selected = index == state.selectedIndex,
                    index = index,
                    continuous = true,
                ),
            )
            childIndex += 1
        }
        trimCandidateChildren(strip, childIndex)
    }

    private fun renderStaticCandidate(strip: LinearLayout, text: String) {
        bindCandidateChip(
            ensureCandidateChip(strip, 0),
            CandidateChipBinding(
                text = text,
                primary = false,
                selected = false,
                index = -1,
                continuous = false,
            ),
        )
        trimCandidateChildren(strip, 1)
    }

    private fun ensureCandidateChip(strip: LinearLayout, index: Int): TextView {
        val current = strip.getChildAt(index)
        if (current is TextView) {
            return current
        }
        if (current != null) {
            strip.removeViewAt(index)
        }
        return TextView(this).also { strip.addView(it, index) }
    }

    private fun ensureCandidateSeparator(strip: LinearLayout, index: Int) {
        val current = strip.getChildAt(index)
        if (current != null && current !is TextView) {
            return
        }
        if (current != null) {
            strip.removeViewAt(index)
        }
        strip.addView(buildCandidateSeparator(), index)
    }

    private fun trimCandidateChildren(strip: LinearLayout, desiredCount: Int) {
        while (strip.childCount > desiredCount) {
            strip.removeViewAt(strip.childCount - 1)
        }
    }

    private fun buildCandidateSeparator(): View = View(this).apply {
        layoutParams = LinearLayout.LayoutParams(dp(1), dp(22)).apply {
            marginStart = dp(2)
            marginEnd = dp(2)
            gravity = Gravity.CENTER_VERTICAL
        }
        setBackgroundColor(Color.parseColor("#C8D7E7"))
        alpha = 0.55f
    }

    private fun editorCompletionLabel(completion: CompletionInfo): String =
        completion.label
            ?.toString()
            ?.takeIf { it.isNotBlank() }
            ?: completion.text.toString()

    private fun refreshEditorCompletions(strip: LinearLayout) {
        if (editorCompletions.isEmpty()) {
            strip.addView(
                buildCandidateChip(
                    getString(R.string.candidate_editor_completion_waiting),
                    primary = false,
                    selected = false,
                    index = -1,
                )
            )
            return
        }

        editorCompletions.forEachIndexed { index, completion ->
            val chip = buildCandidateChip(
                text = editorCompletionLabel(completion),
                primary = index == 0,
                selected = false,
                index = -1,
            )
            chip.setOnClickListener { commitEditorCompletion(completion) }
            strip.addView(chip)
        }
    }

    private fun commitEditorCompletion(completion: CompletionInfo) {
        if (!editorPolicy.editorCompletionsEnabled || editorPolicy.secureInput) {
            return
        }
        val connection = currentInputConnection ?: return
        val text = completion.text?.toString().orEmpty()
        if (text.isEmpty()) {
            return
        }
        if (!connection.commitCompletion(completion)) {
            connection.commitText(text, 1)
        }
        editorCompletions = emptyList()
        refreshImeUi()
    }

    private fun refreshHandwriteCandidates(seeds: List<String>) {
        handwriteCandidateStrip.removeAllViews()
        if (seeds.isEmpty()) {
            handwriteStatus.text = getString(R.string.handwrite_status_idle)
            return
        }

        seeds.forEachIndexed { index, seed ->
            val chip = buildCandidateChip(
                seed,
                primary = index == 0,
                selected = seed == selectedHandwriteSeed,
                index = -1,
                animateAppearance = true,
            )
            chip.setOnClickListener {
                applyHandwriteSeed(seed, auto = false)
                refreshHandwriteCandidates(seeds)
            }
            handwriteCandidateStrip.addView(chip)
        }
    }

    private fun applyHandwriteSeed(seed: String, auto: Boolean, totalSeeds: Int = 1) {
        if (editorPolicy.secureInput) {
            return
        }
        selectedHandwriteSeed = seed
        if (editorPolicy.suggestionsEnabled) {
            SuzakuNativeBridge.nativeReplaceMarkedText(seed)
            syncComposingText()
        }
        handwriteStatus.text = if (auto) {
            getString(R.string.handwrite_status_auto, seed)
        } else if (totalSeeds > 1) {
            getString(R.string.handwrite_status_ready_count, totalSeeds, seed)
        } else {
            getString(R.string.handwrite_status_ready, seed)
        }
    }

    private fun buildCandidateChip(
        text: String,
        primary: Boolean,
        selected: Boolean,
        index: Int,
        continuous: Boolean = false,
        animateAppearance: Boolean = false,
    ): TextView {
        val chip = TextView(this)
        bindCandidateChip(
            chip,
            CandidateChipBinding(text, primary, selected, index, continuous),
        )
        if (animateAppearance) {
            chip.alpha = 0f
            chip.translationY = dp(4).toFloat()
            chip.post { chip.animate().alpha(1f).translationY(0f).setDuration(120L).start() }
        }
        return chip
    }

    private fun bindCandidateChip(view: TextView, binding: CandidateChipBinding) {
        val previous = view.tag as? CandidateChipBinding
        if (previous == binding) {
            return
        }
        view.animate().cancel()
        setTextIfChanged(view, binding.text)
        val horizontal = if (binding.primary) dp(18) else if (binding.continuous) dp(10) else dp(14)
        val vertical = if (binding.primary) dp(11) else if (binding.continuous) dp(8) else dp(10)
        if (
            view.paddingLeft != horizontal ||
            view.paddingTop != vertical ||
            view.paddingRight != horizontal ||
            view.paddingBottom != vertical
        ) {
            view.setPadding(horizontal, vertical, horizontal, vertical)
        }
        val textSize = if (binding.primary) 17f else if (binding.continuous) 14f else 15f
        val previousTextSize = when {
            previous?.primary == true -> 17f
            previous?.continuous == true -> 14f
            else -> 15f
        }
        if (previous == null || previousTextSize != textSize) {
            view.textSize = textSize
        }
        val minWidth = if (binding.primary) dp(104) else dp(0)
        if (view.minWidth != minWidth) {
            view.minWidth = minWidth
        }
        if (
            previous == null ||
            previous.selected != binding.selected ||
            previous.primary != binding.primary ||
            previous.continuous != binding.continuous
        ) {
            view.setTextColor(
                when {
                    binding.selected -> 0xFF163A61.toInt()
                    binding.primary -> 0xFF173B63.toInt()
                    else -> 0xFF254A72.toInt()
                }
            )
            view.setBackgroundResource(
                when {
                    binding.selected && !binding.primary -> R.drawable.candidate_chip_selected
                    binding.selected || binding.primary -> R.drawable.candidate_chip_primary
                    binding.continuous -> R.drawable.candidate_chip_flat
                    else -> R.drawable.candidate_chip_secondary
                }
            )
        }
        val params = view.layoutParams as? LinearLayout.LayoutParams
        val marginEnd = if (binding.continuous) dp(0) else dp(8)
        if (params == null) {
            view.layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { this.marginEnd = marginEnd }
        } else if (params.marginEnd != marginEnd) {
            params.marginEnd = marginEnd
            view.layoutParams = params
        }
        view.alpha = 1f
        view.translationY = 0f
        view.tag = binding
        if (binding.index >= 0) {
            view.setOnClickListener(nativeCandidateClickListener)
        } else {
            view.setOnClickListener(null)
            view.isClickable = false
        }
    }

    private fun commitSelected(force: Boolean): Boolean {
        if (!editorPolicy.suggestionsEnabled) {
            return false
        }
        val committed = SuzakuNativeBridge.nativeCommitSelected(force)
        if (!committed) {
            return false
        }

        val delta = SuzakuNativeBridge.nativeTakeLastCommittedText()
        if (delta.isNotEmpty()) {
            currentInputConnection?.finishComposingText()
            currentInputConnection?.commitText(delta, 1)
        }
        SuzakuNativeBridge.nativeClearMarkedText()
        selectedHandwriteSeed = null
        refreshAutomaticShift()
        return true
    }

    private fun syncComposingText(
        displayText: String = SuzakuNativeBridge.nativeDisplayText(),
    ) {
        val connection = currentInputConnection ?: return
        if (!editorPolicy.suggestionsEnabled) {
            connection.finishComposingText()
            return
        }
        if (displayText.isEmpty()) {
            connection.finishComposingText()
        } else {
            connection.setComposingText(displayText, 1)
        }
    }

    private fun setTextIfChanged(view: TextView, value: CharSequence) {
        if (view.text.toString() != value.toString()) {
            view.text = value
        }
    }

    private fun setVisibilityIfChanged(view: View, visibility: Int) {
        if (view.visibility != visibility) {
            view.visibility = visibility
        }
    }

    private fun setEnabledState(
        view: View,
        enabled: Boolean,
        disabledAlpha: Float? = null,
    ) {
        if (view.isEnabled != enabled) {
            view.isEnabled = enabled
        }
        disabledAlpha?.let { alpha ->
            val nextAlpha = if (enabled) 1f else alpha
            if (view.alpha != nextAlpha) {
                view.alpha = nextAlpha
            }
        }
    }

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()

    private fun styleToolButton(button: ImageButton) {
        button.setBackgroundResource(R.drawable.tool_button_bg)
        button.imageTintList = android.content.res.ColorStateList.valueOf(Color.parseColor("#486989"))
    }

    private fun styleActionButton(button: Button, compact: Boolean = false) {
        button.setBackgroundResource(R.drawable.gboard_action_button_bg)
        button.setTextColor(Color.parseColor("#204667"))
        button.isAllCaps = false
        button.textSize = if (compact) 13f else 14f
        val minHeight = dp(if (compact) 38 else 42)
        button.minHeight = minHeight
        button.minimumHeight = minHeight
        button.setPadding(
            dp(if (compact) 12 else 14),
            dp(if (compact) 8 else 10),
            dp(if (compact) 12 else 14),
            dp(if (compact) 8 else 10),
        )
    }

    private fun buildStripTransition(): LayoutTransition = LayoutTransition().apply {
        setDuration(LayoutTransition.APPEARING, 120L)
        setDuration(LayoutTransition.CHANGE_APPEARING, 120L)
        setDuration(LayoutTransition.DISAPPEARING, 90L)
    }

    private fun syncVoiceStatusFromPhase() {
        val status = when (voicePhase) {
            SuzakuVoicePhase.IDLE -> getString(R.string.voice_status_idle)
            SuzakuVoicePhase.STARTING -> getString(R.string.voice_status_starting)
            SuzakuVoicePhase.LISTENING -> getString(R.string.voice_status_listening)
            SuzakuVoicePhase.HEARING -> getString(R.string.voice_status_hearing)
            SuzakuVoicePhase.PROCESSING -> getString(R.string.voice_status_processing)
            SuzakuVoicePhase.READY -> getString(R.string.voice_status_ready)
            SuzakuVoicePhase.ERROR -> voiceStatus.text?.takeIf { it.isNotBlank() }
                ?: getString(R.string.voice_status_idle)
        }
        setTextIfChanged(voiceStatus, status)
    }

    private fun refreshVoiceLevelMeter() =
        voiceLevelMeter.refresh(voicePhase, imePanelExpanded && drawerMode == DrawerMode.VOICE)

    private fun refreshPanelVisibility() {
        panelAnimator.sync(imePanelExpanded)
    }

    private fun expandIme(mode: DrawerMode) {
        drawerMode = if (
            editorPolicy.secureInput &&
            (
                mode == DrawerMode.VOICE ||
                    mode == DrawerMode.HANDWRITE ||
                    mode == DrawerMode.CLIPBOARD
            )
        ) {
            DrawerMode.KEYBOARD
        } else {
            mode
        }
        val wasExpanded = imePanelExpanded
        imePanelExpanded = true
        if (!wasExpanded) panelAnimator.expand()
        refreshImeUi()
        if (drawerMode == DrawerMode.CLIPBOARD) {
            refreshClipboardDrawer()
        }
    }

    private fun collapseIme() {
        stopBackspaceRepeat()
        clearClipboardPreview()
        lastDrawerRenderState = null
        val wasExpanded = imePanelExpanded
        imePanelExpanded = false
        if (wasExpanded) panelAnimator.collapse()
        else refreshPanelVisibility()
        refreshVoiceLevelMeter()
    }
}
