package dev.suzaku.android.ime

internal enum class SuzakuKeyboardLayout {
    LETTERS,
    NUMBERS,
    SYMBOLS,
}

internal enum class SuzakuKeyboardProfile {
    TEXT,
    EMAIL,
    URI,
    NUMBER,
    PHONE,
    DATETIME;

    val usesDedicatedNumberPad: Boolean
        get() = this == NUMBER || this == PHONE || this == DATETIME
}

internal enum class SuzakuEnterKey(
    val label: String,
    val accessibilityLabel: String,
) {
    ENTER("↵", "Enter"),
    GO("Go", "Go"),
    SEARCH("Search", "Search"),
    SEND("Send", "Send"),
    NEXT("Next", "Next"),
    DONE("Done", "Done"),
    PREVIOUS("Prev", "Previous"),
}

internal enum class SuzakuShiftState {
    OFF,
    ONCE,
    LOCKED,
}

internal enum class SuzakuSoftKeyAction {
    INSERT,
    COMMIT_LITERAL,
    SHIFT,
    BACKSPACE,
    ENTER,
    SPACE,
    SWITCH_INPUT_METHOD,
    SHOW_LETTERS,
    SHOW_NUMBERS,
    SHOW_SYMBOLS,
}

internal data class SuzakuSoftKey(
    val label: String,
    val action: SuzakuSoftKeyAction,
    val value: String = "",
    val weight: Float = 1f,
    val active: Boolean = false,
    val accessibilityLabel: String = label,
)

internal sealed class SuzakuKeyboardEffect {
    data class Insert(val text: String, val keysChanged: Boolean) : SuzakuKeyboardEffect()
    data class CommitLiteral(val text: String) : SuzakuKeyboardEffect()
    object Backspace : SuzakuKeyboardEffect()
    object Enter : SuzakuKeyboardEffect()
    object Space : SuzakuKeyboardEffect()
    object SwitchInputMethod : SuzakuKeyboardEffect()
    object KeysChanged : SuzakuKeyboardEffect()
}

internal class SuzakuKeyboardState {
    var layout: SuzakuKeyboardLayout = SuzakuKeyboardLayout.LETTERS
        private set
    var shift: SuzakuShiftState = SuzakuShiftState.OFF
        private set
    var numberRowEnabled: Boolean = false
        private set
    var profile: SuzakuKeyboardProfile = SuzakuKeyboardProfile.TEXT
        private set
    var enterKey: SuzakuEnterKey = SuzakuEnterKey.ENTER
        private set
    private var signedNumbers: Boolean = false
    private var decimalNumbers: Boolean = false

    fun reset(
        profile: SuzakuKeyboardProfile = SuzakuKeyboardProfile.TEXT,
        capitalize: Boolean = false,
        signedNumbers: Boolean = false,
        decimalNumbers: Boolean = false,
        enterKey: SuzakuEnterKey = SuzakuEnterKey.ENTER,
    ) {
        this.profile = profile
        this.signedNumbers = signedNumbers
        this.decimalNumbers = decimalNumbers
        this.enterKey = enterKey
        layout = if (profile.usesDedicatedNumberPad) {
            SuzakuKeyboardLayout.NUMBERS
        } else {
            SuzakuKeyboardLayout.LETTERS
        }
        shift = if (layout == SuzakuKeyboardLayout.LETTERS && capitalize) {
            SuzakuShiftState.ONCE
        } else {
            SuzakuShiftState.OFF
        }
    }

    fun setNumberRowEnabled(enabled: Boolean): Boolean {
        if (numberRowEnabled == enabled) {
            return false
        }
        numberRowEnabled = enabled
        return true
    }

    fun applyAutomaticShift(enabled: Boolean): Boolean {
        if (layout != SuzakuKeyboardLayout.LETTERS || shift == SuzakuShiftState.LOCKED) {
            return false
        }
        val next = if (enabled) SuzakuShiftState.ONCE else SuzakuShiftState.OFF
        if (shift == next) {
            return false
        }
        shift = next
        return true
    }

    fun rows(): List<List<SuzakuSoftKey>> = when (layout) {
        SuzakuKeyboardLayout.LETTERS -> letterRows()
        SuzakuKeyboardLayout.NUMBERS -> if (profile.usesDedicatedNumberPad) {
            dedicatedNumberRows()
        } else {
            numberRows()
        }
        SuzakuKeyboardLayout.SYMBOLS -> symbolRows()
    }

    fun press(key: SuzakuSoftKey): SuzakuKeyboardEffect = when (key.action) {
        SuzakuSoftKeyAction.INSERT -> {
            val consumedOneShotShift = layout == SuzakuKeyboardLayout.LETTERS &&
                shift == SuzakuShiftState.ONCE
            if (consumedOneShotShift) {
                shift = SuzakuShiftState.OFF
            }
            SuzakuKeyboardEffect.Insert(key.value, consumedOneShotShift)
        }
        SuzakuSoftKeyAction.COMMIT_LITERAL -> SuzakuKeyboardEffect.CommitLiteral(key.value)
        SuzakuSoftKeyAction.SHIFT -> {
            shift = when (shift) {
                SuzakuShiftState.OFF -> SuzakuShiftState.ONCE
                SuzakuShiftState.ONCE -> SuzakuShiftState.LOCKED
                SuzakuShiftState.LOCKED -> SuzakuShiftState.OFF
            }
            SuzakuKeyboardEffect.KeysChanged
        }
        SuzakuSoftKeyAction.BACKSPACE -> SuzakuKeyboardEffect.Backspace
        SuzakuSoftKeyAction.ENTER -> SuzakuKeyboardEffect.Enter
        SuzakuSoftKeyAction.SPACE -> SuzakuKeyboardEffect.Space
        SuzakuSoftKeyAction.SWITCH_INPUT_METHOD -> SuzakuKeyboardEffect.SwitchInputMethod
        SuzakuSoftKeyAction.SHOW_LETTERS -> {
            layout = SuzakuKeyboardLayout.LETTERS
            shift = SuzakuShiftState.OFF
            SuzakuKeyboardEffect.KeysChanged
        }
        SuzakuSoftKeyAction.SHOW_NUMBERS -> {
            layout = SuzakuKeyboardLayout.NUMBERS
            shift = SuzakuShiftState.OFF
            SuzakuKeyboardEffect.KeysChanged
        }
        SuzakuSoftKeyAction.SHOW_SYMBOLS -> {
            layout = SuzakuKeyboardLayout.SYMBOLS
            shift = SuzakuShiftState.OFF
            SuzakuKeyboardEffect.KeysChanged
        }
    }

    private fun letterRows(): List<List<SuzakuSoftKey>> = buildList {
        if (numberRowEnabled) {
            add(characterKeys(listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0")))
        }
        add(characterRow("qwertyuiop"))
        add(characterRow("asdfghjkl"))
        add(listOf(shiftKey()) + characterRow("zxcvbnm") + backspaceKey())
        add(
            when (profile) {
                SuzakuKeyboardProfile.EMAIL -> listOf(
                    switchInputMethodKey(),
                    controlKey("?123", SuzakuSoftKeyAction.SHOW_NUMBERS, 1.4f, "Show numbers"),
                    literalKey("@"),
                    literalKey(".com", weight = 2.8f),
                    literalKey("."),
                    enterKey(),
                )
                SuzakuKeyboardProfile.URI -> listOf(
                    switchInputMethodKey(),
                    controlKey("?123", SuzakuSoftKeyAction.SHOW_NUMBERS, 1.4f, "Show numbers"),
                    literalKey("/"),
                    literalKey(".com", weight = 2.8f),
                    literalKey("."),
                    enterKey(),
                )
                else -> listOf(
                    switchInputMethodKey(),
                    controlKey("?123", SuzakuSoftKeyAction.SHOW_NUMBERS, 1.4f, "Show numbers"),
                    literalKey(","),
                    spaceKey(),
                    literalKey("."),
                    enterKey(),
                )
            }
        )
    }

    private fun dedicatedNumberRows(): List<List<SuzakuSoftKey>> {
        val digitRows = listOf(
            literalKeys(listOf("1", "2", "3")),
            literalKeys(listOf("4", "5", "6")),
            literalKeys(listOf("7", "8", "9")),
        )
        return when (profile) {
            SuzakuKeyboardProfile.NUMBER -> digitRows + listOf(
                buildList {
                    add(switchInputMethodKey())
                    if (signedNumbers) add(literalKey("-"))
                    add(literalKey("0"))
                    if (decimalNumbers) add(literalKey("."))
                    add(backspaceKey())
                    add(enterKey())
                }
            )
            SuzakuKeyboardProfile.PHONE -> digitRows + listOf(
                literalKeys(listOf("*", "0", "#")),
                listOf(switchInputMethodKey(), literalKey("+"), backspaceKey(), enterKey()),
            )
            SuzakuKeyboardProfile.DATETIME -> digitRows + listOf(
                literalKeys(listOf("/", "0", ":")),
                listOf(switchInputMethodKey(), literalKey("-"), backspaceKey(), enterKey()),
            )
            else -> digitRows
        }
    }

    private fun numberRows(): List<List<SuzakuSoftKey>> = listOf(
        literalKeys(listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0")),
        literalKeys(listOf("@", "#", "\$", "%", "&", "-", "+", "(", ")", "/")),
        listOf(controlKey("#+=", SuzakuSoftKeyAction.SHOW_SYMBOLS, 1.4f, "Show symbols")) +
            literalKeys(listOf("*", "\"", "'", ":", ";", "!", "?")) + backspaceKey(),
        navigationRow(),
    )

    private fun symbolRows(): List<List<SuzakuSoftKey>> = listOf(
        literalKeys(listOf("[", "]", "{", "}", "#", "%", "^", "*", "+", "=")),
        literalKeys(listOf("_", "\\", "|", "~", "<", ">", "€", "£", "¥", "•")),
        listOf(controlKey("123", SuzakuSoftKeyAction.SHOW_NUMBERS, 1.4f, "Show numbers")) +
            literalKeys(listOf("@", "&", "-", "+", "(", ")", "/")) + backspaceKey(),
        navigationRow(),
    )

    private fun navigationRow(): List<SuzakuSoftKey> = listOf(
        switchInputMethodKey(),
        controlKey("ABC", SuzakuSoftKeyAction.SHOW_LETTERS, 1.4f, "Show letters"),
        literalKey(","),
        spaceKey(),
        literalKey("."),
        enterKey(),
    )

    private fun characterRow(characters: String): List<SuzakuSoftKey> =
        characters.map { characterKey(it.toString(), shiftable = true) }

    private fun characterKeys(values: List<String>): List<SuzakuSoftKey> = values.map(::characterKey)

    private fun literalKeys(values: List<String>): List<SuzakuSoftKey> = values.map(::literalKey)

    private fun literalKey(value: String, weight: Float = 1f): SuzakuSoftKey = SuzakuSoftKey(
        label = value,
        action = SuzakuSoftKeyAction.COMMIT_LITERAL,
        value = value,
        weight = weight,
        accessibilityLabel = value,
    )

    private fun characterKey(
        value: String,
        weight: Float = 1f,
        shiftable: Boolean = false,
    ): SuzakuSoftKey {
        val displayed = if (
            shiftable &&
            layout == SuzakuKeyboardLayout.LETTERS &&
            shift != SuzakuShiftState.OFF
        ) {
            value.uppercase()
        } else {
            value
        }
        return SuzakuSoftKey(
            label = displayed,
            action = SuzakuSoftKeyAction.INSERT,
            value = displayed,
            weight = weight,
            accessibilityLabel = displayed,
        )
    }

    private fun shiftKey(): SuzakuSoftKey = SuzakuSoftKey(
        label = if (shift == SuzakuShiftState.LOCKED) "⇪" else "⇧",
        action = SuzakuSoftKeyAction.SHIFT,
        weight = 1.4f,
        active = shift != SuzakuShiftState.OFF,
        accessibilityLabel = when (shift) {
            SuzakuShiftState.OFF -> "Shift"
            SuzakuShiftState.ONCE -> "Shift once"
            SuzakuShiftState.LOCKED -> "Caps lock"
        },
    )

    private fun backspaceKey(): SuzakuSoftKey = controlKey(
        label = "⌫",
        action = SuzakuSoftKeyAction.BACKSPACE,
        weight = 1.4f,
        accessibilityLabel = "Backspace",
    )

    private fun enterKey(): SuzakuSoftKey = controlKey(
        label = enterKey.label,
        action = SuzakuSoftKeyAction.ENTER,
        weight = if (enterKey.label.length > 2) 1.8f else 1.4f,
        accessibilityLabel = enterKey.accessibilityLabel,
    )

    private fun spaceKey(): SuzakuSoftKey = controlKey(
        label = "space",
        action = SuzakuSoftKeyAction.SPACE,
        weight = 4f,
        accessibilityLabel = "Space",
    )

    private fun switchInputMethodKey(): SuzakuSoftKey = controlKey(
        label = "🌐",
        action = SuzakuSoftKeyAction.SWITCH_INPUT_METHOD,
        weight = 1.2f,
        accessibilityLabel = "Switch keyboard",
    )

    private fun controlKey(
        label: String,
        action: SuzakuSoftKeyAction,
        weight: Float,
        accessibilityLabel: String,
    ): SuzakuSoftKey = SuzakuSoftKey(
        label = label,
        action = action,
        weight = weight,
        accessibilityLabel = accessibilityLabel,
    )
}

internal fun dropLastTextElement(text: String): String {
    if (text.isEmpty()) {
        return text
    }

    var start = previousCodePointStart(text, text.length)
    var codePoint = text.codePointAt(start)

    if (codePoint == '\n'.code && start > 0 && text.codePointBefore(start) == '\r'.code) {
        start = previousCodePointStart(text, start)
        return text.substring(0, start)
    }

    if (isRegionalIndicator(codePoint)) {
        var cursor = start
        var regionalIndicatorCount = 1
        while (cursor > 0) {
            val previous = previousCodePointStart(text, cursor)
            if (!isRegionalIndicator(text.codePointAt(previous))) {
                break
            }
            regionalIndicatorCount += 1
            cursor = previous
        }
        if (regionalIndicatorCount % 2 == 0) {
            start = previousCodePointStart(text, start)
        }
        return text.substring(0, start)
    }

    while (isGraphemeExtension(codePoint) && start > 0) {
        start = previousCodePointStart(text, start)
        codePoint = text.codePointAt(start)
    }

    while (start > 0) {
        val joinerStart = previousCodePointStart(text, start)
        if (text.codePointAt(joinerStart) != ZERO_WIDTH_JOINER) {
            break
        }
        start = joinerStart
        if (start == 0) {
            break
        }
        start = previousCodePointStart(text, start)
        codePoint = text.codePointAt(start)
        while (isGraphemeExtension(codePoint) && start > 0) {
            start = previousCodePointStart(text, start)
            codePoint = text.codePointAt(start)
        }
    }

    return text.substring(0, start)
}

private const val ZERO_WIDTH_JOINER = 0x200D

private fun previousCodePointStart(text: String, end: Int): Int =
    end - Character.charCount(text.codePointBefore(end))

private fun isRegionalIndicator(codePoint: Int): Boolean = codePoint in 0x1F1E6..0x1F1FF

private fun isGraphemeExtension(codePoint: Int): Boolean {
    val category = Character.getType(codePoint)
    return category == Character.NON_SPACING_MARK.toInt() ||
        category == Character.COMBINING_SPACING_MARK.toInt() ||
        category == Character.ENCLOSING_MARK.toInt() ||
        codePoint in 0x1F3FB..0x1F3FF ||
        codePoint in 0xFE00..0xFE0F ||
        codePoint in 0xE0100..0xE01EF ||
        codePoint in 0xE0020..0xE007F
}
