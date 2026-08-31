package dev.suzaku.android.ime

import android.animation.LayoutTransition
import android.annotation.SuppressLint
import android.graphics.Color
import android.inputmethodservice.InputMethodService
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

private enum class DrawerMode { KEYBOARD, VOICE, HANDWRITE, SETTINGS }
private const val MAX_EDITOR_COMPLETIONS = 6

class SuzakuInputMethodService : InputMethodService() {
    private lateinit var composePreview: TextView
    private lateinit var hostStatus: TextView
    private lateinit var candidateStrip: LinearLayout
    private lateinit var keyboardDrawer: View
    private lateinit var voiceDrawer: View
    private lateinit var handwriteDrawer: View
    private lateinit var settingsDrawer: View
    private lateinit var keyboardRows: LinearLayout
    private lateinit var voiceStatus: TextView
    private lateinit var voiceLevelRow: LinearLayout
    private lateinit var voiceTranscriptInput: EditText
    private lateinit var handwriteStatus: TextView
    private lateinit var handwriteCanvas: SuzakuHandwriteCanvasView
    private lateinit var handwriteCandidateStrip: LinearLayout
    private lateinit var toolKeyboard: ImageButton
    private lateinit var toolVoice: ImageButton
    private lateinit var toolHandwrite: ImageButton
    private lateinit var toolSettings: ImageButton
    private lateinit var toolCollapse: ImageButton
    private lateinit var voiceListenButton: Button
    private lateinit var voiceUseTranscriptButton: Button
    private lateinit var voiceCommitButton: Button
    private lateinit var voiceClearButton: Button
    private lateinit var handwriteApplyButton: Button
    private lateinit var handwriteClearButton: Button
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
    private var imeSettings = SuzakuImeSettings()
    private var syncingSettingsControls = false
    private val keyboardState = SuzakuKeyboardState()
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
    private lateinit var settingsStore: SuzakuImeSettingsStore
    private lateinit var panelAnimator: SuzakuImePanelAnimator
    private lateinit var voiceLevelMeter: SuzakuVoiceLevelMeter
    private lateinit var voiceRecognizer: SuzakuVoiceRecognizer

    override fun onCreate() {
        super.onCreate()
        settingsStore = SuzakuImeSettingsStore(this)
        imeSettings = settingsStore.load()
        keyboardState.setNumberRowEnabled(imeSettings.numberRow)
    }

    override fun onCreateInputView(): View {
        val root = LayoutInflater.from(this).inflate(R.layout.input_view, null, false)
        setCandidatesViewShown(false)
        bindViews(root)
        configureKeyboardRows(keyboardRows)
        configureToolButtons()
        configureVoiceDrawer()
        configureHandwriteDrawer()
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
        SuzakuNativeBridge.nativeClearMarkedText()
        SuzakuNativeBridge.nativeDeactivateSession()
    }
    override fun onDestroy() {
        super.onDestroy()
        stopBackspaceRepeat(updateAutomaticShift = false)
        editorCompletions = emptyList()
        if (::voiceRecognizer.isInitialized) voiceRecognizer.destroy()
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
        settingsDrawer = root.findViewById(R.id.settingsDrawer)
        keyboardRows = root.findViewById(R.id.keyboardRows)
        voiceStatus = root.findViewById(R.id.voiceStatus)
        voiceLevelRow = root.findViewById(R.id.voiceLevelRow)
        voiceTranscriptInput = root.findViewById(R.id.voiceTranscriptInput)
        handwriteStatus = root.findViewById(R.id.handwriteStatus)
        handwriteCanvas = root.findViewById(R.id.handwriteCanvas)
        handwriteCandidateStrip = root.findViewById(R.id.handwriteCandidateStrip)
        toolKeyboard = root.findViewById(R.id.toolKeyboard)
        toolVoice = root.findViewById(R.id.toolVoice)
        toolHandwrite = root.findViewById(R.id.toolHandwrite)
        toolSettings = root.findViewById(R.id.toolSettings)
        toolCollapse = root.findViewById(R.id.toolCollapse)
        voiceListenButton = root.findViewById(R.id.voiceListenButton)
        voiceUseTranscriptButton = root.findViewById(R.id.voiceUseTranscriptButton)
        voiceCommitButton = root.findViewById(R.id.voiceCommitButton)
        voiceClearButton = root.findViewById(R.id.voiceClearButton)
        handwriteApplyButton = root.findViewById(R.id.handwriteApplyButton)
        handwriteClearButton = root.findViewById(R.id.handwriteClearButton)
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
        candidateStrip.layoutTransition = buildStripTransition()
        handwriteCandidateStrip.layoutTransition = buildStripTransition()
    }
    private fun configureToolButtons() {
        styleToolButton(toolKeyboard)
        styleToolButton(toolVoice)
        styleToolButton(toolHandwrite)
        styleToolButton(toolSettings)
        styleToolButton(toolCollapse)
        styleToolButton(compactBubbleButton)
        toolKeyboard.setOnClickListener { expandIme(DrawerMode.KEYBOARD) }
        toolVoice.setOnClickListener { expandIme(DrawerMode.VOICE) }
        toolHandwrite.setOnClickListener { expandIme(DrawerMode.HANDWRITE) }
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
                    syncComposingText()
                    refreshCandidateStrip(candidateStrip)
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
                    syncComposingText()
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
        container.removeAllViews()
        keyboardState.rows().forEach { row ->
            container.addView(buildKeyboardRow(row))
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
            row.addView(
                Button(this).apply {
                    layoutParams = LinearLayout.LayoutParams(0, dp(46), key.weight).apply {
                        marginEnd = dp(6)
                    }
                    text = key.label
                    contentDescription = key.accessibilityLabel
                    isAllCaps = false
                    setBackgroundResource(
                        if (key.active) R.drawable.candidate_chip_selected
                        else R.drawable.gboard_key_bg
                    )
                    setTextColor(Color.parseColor(if (key.active) "#123A63" else "#24476C"))
                    textSize = if (
                        key.action == SuzakuSoftKeyAction.INSERT ||
                        key.action == SuzakuSoftKeyAction.COMMIT_LITERAL
                    ) 19f else 16f
                    minHeight = dp(46)
                    setOnClickListener { view ->
                        if (imeSettings.hapticFeedback) {
                            view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
                        }
                        handleSoftKeyPress(key)
                    }
                    when (key.action) {
                        SuzakuSoftKeyAction.BACKSPACE -> configureBackspaceRepeat(this)
                        SuzakuSoftKeyAction.SWITCH_INPUT_METHOD -> {
                            setOnLongClickListener {
                                showInputMethodPicker()
                                true
                            }
                        }
                        else -> Unit
                    }
                }
            )
        }

        return row
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
        syncComposingText()
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
            syncComposingText()
        }
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
        if (drawerMode == DrawerMode.VOICE || drawerMode == DrawerMode.HANDWRITE) {
            drawerMode = DrawerMode.KEYBOARD
        }
    }

    private fun refreshImeUi() {
        val displayText = if (editorPolicy.suggestionsEnabled) {
            SuzakuNativeBridge.nativeDisplayText()
        } else {
            ""
        }
        composePreview.text = when {
            editorPolicy.secureInput -> getString(R.string.secure_input_placeholder)
            !editorPolicy.suggestionsEnabled -> getString(R.string.direct_input_placeholder)
            displayText.isEmpty() -> getString(R.string.compose_placeholder)
            else -> displayText
        }
        hostStatus.text = if (editorPolicy.secureInput) {
            getString(R.string.host_status_secure)
        } else {
            SuzakuNativeBridge.nativeDescribeImeHost()
        }
        refreshCandidateStrip(candidateStrip)
        refreshPanelVisibility()
        val hasCandidateActivity = when {
            editorPolicy.editorCompletionsEnabled -> editorCompletions.isNotEmpty()
            editorPolicy.suggestionsEnabled ->
                SuzakuNativeBridge.nativeCandidateCount() > 0 || displayText.isNotEmpty()
            else -> false
        }
        compactBubbleDot.visibility = if (hasCandidateActivity) View.VISIBLE else View.GONE
        refreshDrawerVisibility()
        syncComposingText()
    }

    private fun refreshDrawerVisibility() {
        if (
            editorPolicy.secureInput &&
            (drawerMode == DrawerMode.VOICE || drawerMode == DrawerMode.HANDWRITE)
        ) {
            drawerMode = DrawerMode.KEYBOARD
        }
        keyboardDrawer.visibility = if (drawerMode == DrawerMode.KEYBOARD) View.VISIBLE else View.GONE
        voiceDrawer.visibility = if (drawerMode == DrawerMode.VOICE) View.VISIBLE else View.GONE
        handwriteDrawer.visibility = if (drawerMode == DrawerMode.HANDWRITE) View.VISIBLE else View.GONE
        settingsDrawer.visibility = if (drawerMode == DrawerMode.SETTINGS) View.VISIBLE else View.GONE
        val multimodalInputEnabled = !editorPolicy.secureInput
        toolVoice.isEnabled = multimodalInputEnabled
        toolVoice.alpha = if (multimodalInputEnabled) 1f else 0.38f
        toolHandwrite.isEnabled = multimodalInputEnabled
        toolHandwrite.alpha = if (multimodalInputEnabled) 1f else 0.38f
        handwriteCanvas.isEnabled = multimodalInputEnabled
        handwriteApplyButton.isEnabled = multimodalInputEnabled
        handwriteClearButton.isEnabled = multimodalInputEnabled
        voiceCommitButton.isEnabled = multimodalInputEnabled && editorPolicy.suggestionsEnabled
        if (!::voiceRecognizer.isInitialized || !voiceRecognizer.isListening) {
            voicePhase = if (voiceTranscriptInput.text?.isNotBlank() == true) {
                SuzakuVoicePhase.READY
            } else if (voicePhase == SuzakuVoicePhase.ERROR) {
                SuzakuVoicePhase.ERROR
            } else {
                SuzakuVoicePhase.IDLE
            }
        }
        syncVoiceStatusFromPhase()
        updateVoiceListenButton()
        refreshVoiceLevelMeter()
        handwriteStatus.text = when {
            selectedHandwriteSeed != null -> getString(R.string.handwrite_status_ready, selectedHandwriteSeed)
            handwriteStatus.text == getString(R.string.handwrite_status_writing) -> handwriteStatus.text
            handwriteStatus.text == getString(R.string.handwrite_status_recognizing) -> handwriteStatus.text
            else -> getString(R.string.handwrite_status_idle)
        }
    }

    private fun updateVoiceListenButton() {
        if (!::voiceRecognizer.isInitialized) {
            return
        }
        voiceListenButton.text = getString(
            if (voiceRecognizer.isListening) R.string.voice_stop else R.string.voice_listen
        )
        voiceListenButton.isEnabled = !editorPolicy.secureInput && voiceRecognizer.isAvailable()
    }

    private fun refreshCandidateStrip(strip: LinearLayout?) {
        strip ?: return
        strip.removeAllViews()
        if (editorPolicy.secureInput) {
            strip.addView(
                buildCandidateChip(
                    getString(R.string.candidate_secure_input),
                    primary = false,
                    selected = false,
                    index = -1,
                )
            )
            return
        }
        if (editorPolicy.editorCompletionsEnabled) {
            refreshEditorCompletions(strip)
            return
        }
        if (!editorPolicy.suggestionsEnabled) {
            strip.addView(
                buildCandidateChip(
                    getString(R.string.candidate_direct_input),
                    primary = false,
                    selected = false,
                    index = -1,
                )
            )
            return
        }
        val count = SuzakuNativeBridge.nativeCandidateCount()
        val selected = SuzakuNativeBridge.nativeSelectedIndex()

        if (count <= 0) {
            strip.addView(buildCandidateChip(getString(R.string.candidate_empty), false, false, -1))
            return
        }

        for (index in 0 until count.coerceAtMost(6)) {
            val label = SuzakuNativeBridge.nativeCandidateLabel(index)
            if (index > 0) {
                strip.addView(View(this).apply {
                    layoutParams = LinearLayout.LayoutParams(dp(1), dp(22)).apply {
                        marginStart = dp(2)
                        marginEnd = dp(2)
                        gravity = Gravity.CENTER_VERTICAL
                    }
                    setBackgroundColor(Color.parseColor("#C8D7E7"))
                    alpha = 0.55f
                })
            }
            strip.addView(
                buildCandidateChip(
                    getString(R.string.candidate_label_format, index + 1, label),
                    index == 0,
                    index == selected,
                    index,
                    continuous = true,
                )
            )
        }
    }

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
            val text = completion.label
                ?.toString()
                ?.takeIf { it.isNotBlank() }
                ?: completion.text.toString()
            val chip = buildCandidateChip(
                text = text,
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
    ): TextView {
        return TextView(this).apply {
            val horizontal = if (primary) dp(18) else if (continuous) dp(10) else dp(14)
            val vertical = if (primary) dp(11) else if (continuous) dp(8) else dp(10)
            setPadding(horizontal, vertical, horizontal, vertical)
            this.text = text
            textSize = if (primary) 17f else if (continuous) 14f else 15f
            minWidth = if (primary) dp(104) else dp(0)
            setTextColor(
                when {
                    selected -> 0xFF163A61.toInt()
                    primary -> 0xFF173B63.toInt()
                    else -> 0xFF254A72.toInt()
                }
            )
            background = getDrawable(
                when {
                    selected && !primary -> R.drawable.candidate_chip_selected
                    selected || primary -> R.drawable.candidate_chip_primary
                    continuous -> R.drawable.candidate_chip_flat
                    else -> R.drawable.candidate_chip_secondary
                }
            )
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { marginEnd = if (continuous) dp(0) else dp(8) }
            alpha = 0f
            translationY = dp(4).toFloat()
            post { animate().alpha(1f).translationY(0f).setDuration(140L).start() }
            if (index >= 0) {
                setOnClickListener {
                    SuzakuNativeBridge.nativeSelectCandidate(index)
                    commitSelected(force = true)
                    refreshImeUi()
                }
            }
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

    private fun syncComposingText() {
        val connection = currentInputConnection ?: return
        if (!editorPolicy.suggestionsEnabled) {
            connection.finishComposingText()
            return
        }
        val text = SuzakuNativeBridge.nativeDisplayText()
        if (text.isEmpty()) {
            connection.finishComposingText()
        } else {
            connection.setComposingText(text, 1)
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
        voiceStatus.text = when (voicePhase) {
            SuzakuVoicePhase.IDLE -> getString(R.string.voice_status_idle)
            SuzakuVoicePhase.STARTING -> getString(R.string.voice_status_starting)
            SuzakuVoicePhase.LISTENING -> getString(R.string.voice_status_listening)
            SuzakuVoicePhase.HEARING -> getString(R.string.voice_status_hearing)
            SuzakuVoicePhase.PROCESSING -> getString(R.string.voice_status_processing)
            SuzakuVoicePhase.READY -> getString(R.string.voice_status_ready)
            SuzakuVoicePhase.ERROR -> voiceStatus.text?.takeIf { it.isNotBlank() }
                ?: getString(R.string.voice_status_idle)
        }
    }

    private fun refreshVoiceLevelMeter() =
        voiceLevelMeter.refresh(voicePhase, imePanelExpanded && drawerMode == DrawerMode.VOICE)

    private fun refreshPanelVisibility() {
        panelAnimator.sync(imePanelExpanded)
    }

    private fun expandIme(mode: DrawerMode) {
        drawerMode = if (
            editorPolicy.secureInput &&
            (mode == DrawerMode.VOICE || mode == DrawerMode.HANDWRITE)
        ) {
            DrawerMode.KEYBOARD
        } else {
            mode
        }
        val wasExpanded = imePanelExpanded
        imePanelExpanded = true
        if (!wasExpanded) panelAnimator.expand()
        refreshImeUi()
    }

    private fun collapseIme() {
        stopBackspaceRepeat()
        val wasExpanded = imePanelExpanded
        imePanelExpanded = false
        if (wasExpanded) panelAnimator.collapse()
        else refreshPanelVisibility()
        refreshVoiceLevelMeter()
    }
}
