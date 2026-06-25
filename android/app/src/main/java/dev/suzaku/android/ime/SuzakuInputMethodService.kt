package dev.suzaku.android.ime

import android.animation.LayoutTransition
import android.inputmethodservice.InputMethodService
import android.graphics.Color
import android.view.Gravity
import android.view.LayoutInflater
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.Button
import android.widget.EditText
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.TextView

private enum class DrawerMode { KEYBOARD, VOICE, HANDWRITE }

class SuzakuInputMethodService : InputMethodService() {
    private lateinit var composePreview: TextView
    private lateinit var hostStatus: TextView
    private lateinit var candidateStrip: LinearLayout
    private lateinit var keyboardDrawer: View
    private lateinit var voiceDrawer: View
    private lateinit var handwriteDrawer: View
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
    private lateinit var imePanel: View
    private lateinit var compactBubbleShell: View
    private lateinit var compactBubbleButton: ImageButton
    private lateinit var compactBubbleDot: View
    private var drawerMode = DrawerMode.KEYBOARD
    private var imePanelExpanded = false
    private var selectedHandwriteSeed: String? = null
    private var voicePhase = SuzakuVoicePhase.IDLE
    private lateinit var panelAnimator: SuzakuImePanelAnimator
    private lateinit var voiceLevelMeter: SuzakuVoiceLevelMeter
    private lateinit var voiceRecognizer: SuzakuVoiceRecognizer

    override fun onCreateInputView(): View {
        val root = LayoutInflater.from(this).inflate(R.layout.input_view, null, false)
        setCandidatesViewShown(false)
        bindViews(root)
        configureKeyboardRows(keyboardRows)
        configureToolButtons()
        configureVoiceDrawer()
        configureHandwriteDrawer()
        collapseIme()
        refreshImeUi()
        return root
    }
    override fun onStartInput(attribute: EditorInfo?, restarting: Boolean) {
        super.onStartInput(attribute, restarting)
        setCandidatesViewShown(false)
        SuzakuNativeBridge.nativeActivateSession()
        collapseIme()
        refreshImeUi()
    }
    override fun onStartInputView(attribute: EditorInfo?, restarting: Boolean) {
        super.onStartInputView(attribute, restarting)
        setCandidatesViewShown(false)
        SuzakuNativeBridge.nativeActivateSession()
        collapseIme()
        refreshImeUi()
    }
    override fun onFinishInput() {
        super.onFinishInput()
        SuzakuNativeBridge.nativeDeactivateSession()
    }
    override fun onDestroy() {
        super.onDestroy()
        if (::voiceRecognizer.isInitialized) voiceRecognizer.destroy()
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
        toolSettings.setOnClickListener {
            hostStatus.text = buildString {
                append(SuzakuNativeBridge.nativeRegistrationHint())
                append(" · ")
                append(SuzakuNativeBridge.nativeRegistrationTarget())
                append(" · ready=")
                append(SuzakuNativeBridge.nativeRegistrationReady())
            }
        }
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
            onTranscript = { transcript ->
                voiceTranscriptInput.setText(transcript)
                voiceTranscriptInput.setSelection(transcript.length)
                if (transcript.isNotBlank()) {
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
            if (voiceRecognizer.isListening) {
                voiceRecognizer.stop()
            } else {
                voiceRecognizer.start()
            }
            updateVoiceListenButton()
            refreshVoiceLevelMeter()
        }
        voiceUseTranscriptButton.setOnClickListener {
            val transcript = voiceTranscriptInput.text?.toString()?.trim().orEmpty()
            if (transcript.isNotEmpty()) {
                SuzakuNativeBridge.nativeReplaceMarkedText(transcript)
                voiceStatus.text = getString(R.string.voice_status_applied)
                syncComposingText()
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
            handwriteStatus.text = getString(R.string.handwrite_status_writing)
        }
        handwriteCanvas.onStrokeFinished = { strokes ->
            handwriteStatus.text = getString(R.string.handwrite_status_recognizing)
            handwriteCanvas.post {
                val seeds = SuzakuHandwriteRecognizer.candidateSeeds(strokes)
                refreshHandwriteCandidates(seeds)
                if (seeds.isNotEmpty()) {
                    applyHandwriteSeed(seeds.first(), auto = true, totalSeeds = seeds.size)
                }
            }
        }
        handwriteApplyButton.setOnClickListener {
            selectedHandwriteSeed?.let { seed ->
                applyHandwriteSeed(seed, auto = false, totalSeeds = handwriteCandidateStrip.childCount)
                commitSelected(force = true)
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

    private fun configureKeyboardRows(container: LinearLayout) {
        container.removeAllViews()
        listOf(
            listOf("q", "w", "e", "r", "t", "y", "u", "i", "o", "p"),
            listOf("a", "s", "d", "f", "g", "h", "j", "k", "l"),
            listOf("Shift", "z", "x", "c", "v", "b", "n", "m", "Back"),
            listOf("123", ",", "space", ".", "Enter"),
        ).forEach { row ->
            container.addView(buildKeyboardRow(row))
        }
    }

    private fun buildKeyboardRow(keys: List<String>): View {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = dp(6) }
        }

        keys.forEach { key ->
            val weight = when (key) {
                "space" -> 4f
                "Shift", "Back", "Enter" -> 1.4f
                else -> 1f
            }
            row.addView(
                Button(this).apply {
                    layoutParams = LinearLayout.LayoutParams(0, dp(46), weight).apply {
                        marginEnd = dp(6)
                    }
                    text = key
                    isAllCaps = false
                    setBackgroundResource(R.drawable.gboard_key_bg)
                    setTextColor(Color.parseColor("#24476C"))
                    textSize = 19f
                    minHeight = dp(46)
                    setOnClickListener { handleSoftKeyPress(key) }
                }
            )
        }

        return row
    }

    private fun handleSoftKeyPress(key: String) {
        when (key) {
            "Back" -> deleteLastCharacter()
            "Enter" -> {
                if (!commitSelected(force = true)) {
                    currentInputConnection?.commitText("\n", 1)
                }
            }
            "space" -> {
                if (!commitSelected(force = false)) {
                    currentInputConnection?.commitText(" ", 1)
                }
            }
            "Shift", "123" -> hostStatus.text = SuzakuNativeBridge.nativeDescribeImeHost()
            else -> appendCharacter(key)
        }
        refreshImeUi()
    }

    private fun appendCharacter(key: String) {
        val current = SuzakuNativeBridge.nativeDisplayText()
        SuzakuNativeBridge.nativeReplaceMarkedText(current + key)
        syncComposingText()
    }

    private fun deleteLastCharacter() {
        val current = SuzakuNativeBridge.nativeDisplayText()
        if (current.isEmpty()) {
            currentInputConnection?.deleteSurroundingText(1, 0)
            return
        }

        val updated = current.dropLast(1)
        if (updated.isEmpty()) {
            SuzakuNativeBridge.nativeClearMarkedText()
            currentInputConnection?.finishComposingText()
        } else {
            SuzakuNativeBridge.nativeReplaceMarkedText(updated)
            syncComposingText()
        }
    }

    private fun refreshImeUi() {
        val displayText = SuzakuNativeBridge.nativeDisplayText()
        composePreview.text = displayText.ifEmpty { getString(R.string.compose_placeholder) }
        hostStatus.text = SuzakuNativeBridge.nativeDescribeImeHost()
        refreshCandidateStrip(candidateStrip)
        refreshPanelVisibility()
        compactBubbleDot.visibility = if (
            SuzakuNativeBridge.nativeCandidateCount() > 0 || displayText.isNotEmpty()
        ) View.VISIBLE else View.GONE
        refreshDrawerVisibility()
        syncComposingText()
    }

    private fun refreshDrawerVisibility() {
        keyboardDrawer.visibility = if (drawerMode == DrawerMode.KEYBOARD) View.VISIBLE else View.GONE
        voiceDrawer.visibility = if (drawerMode == DrawerMode.VOICE) View.VISIBLE else View.GONE
        handwriteDrawer.visibility = if (drawerMode == DrawerMode.HANDWRITE) View.VISIBLE else View.GONE
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
        voiceListenButton.isEnabled = voiceRecognizer.isAvailable()
    }

    private fun refreshCandidateStrip(strip: LinearLayout?) {
        strip ?: return
        strip.removeAllViews()
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
        selectedHandwriteSeed = seed
        SuzakuNativeBridge.nativeReplaceMarkedText(seed)
        syncComposingText()
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
        return true
    }

    private fun syncComposingText() {
        val connection = currentInputConnection ?: return
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
        drawerMode = mode
        val wasExpanded = imePanelExpanded
        imePanelExpanded = true
        if (!wasExpanded) panelAnimator.expand()
        refreshImeUi()
    }

    private fun collapseIme() {
        val wasExpanded = imePanelExpanded
        imePanelExpanded = false
        if (wasExpanded) panelAnimator.collapse()
        else refreshPanelVisibility()
        refreshVoiceLevelMeter()
    }
}
