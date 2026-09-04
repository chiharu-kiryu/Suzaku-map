package dev.suzaku.android.ime

internal enum class SuzakuClipboardAvailability {
    BLOCKED,
    EMPTY,
    READY,
    SENSITIVE,
}

internal data class SuzakuClipboardSnapshot(
    val availability: SuzakuClipboardAvailability,
    val preview: String? = null,
) {
    val canPaste: Boolean
        get() = availability == SuzakuClipboardAvailability.READY ||
            availability == SuzakuClipboardAvailability.SENSITIVE
}

internal object SuzakuClipboardPolicy {
    private const val DEFAULT_PREVIEW_CODE_POINTS = 80
    private val whitespace = Regex("\\s+")

    fun inspect(
        text: CharSequence?,
        secureInput: Boolean,
        sensitiveContent: Boolean = false,
        contentAvailable: Boolean = !text.isNullOrEmpty(),
        previewCodePointLimit: Int = DEFAULT_PREVIEW_CODE_POINTS,
    ): SuzakuClipboardSnapshot {
        if (secureInput) {
            return SuzakuClipboardSnapshot(SuzakuClipboardAvailability.BLOCKED)
        }

        if (!contentAvailable) {
            return SuzakuClipboardSnapshot(SuzakuClipboardAvailability.EMPTY)
        }
        if (sensitiveContent) {
            return SuzakuClipboardSnapshot(SuzakuClipboardAvailability.SENSITIVE)
        }
        val content = text?.toString()
        if (content.isNullOrEmpty()) {
            return SuzakuClipboardSnapshot(SuzakuClipboardAvailability.EMPTY)
        }

        return SuzakuClipboardSnapshot(
            availability = SuzakuClipboardAvailability.READY,
            preview = buildPreview(content, previewCodePointLimit),
        )
    }

    private fun buildPreview(text: String, codePointLimit: Int): String? {
        val normalized = text.replace(whitespace, " ").trim()
        if (normalized.isEmpty()) {
            return null
        }

        val limit = codePointLimit.coerceAtLeast(1)
        val codePointCount = normalized.codePointCount(0, normalized.length)
        if (codePointCount <= limit) {
            return normalized
        }
        val end = normalized.offsetByCodePoints(0, limit)
        return normalized.substring(0, end) + "…"
    }
}
