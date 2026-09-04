package dev.suzaku.android.ime

internal data class SuzakuImeRenderSnapshot(
    val displayText: String,
    val selectedIndex: Int,
    val candidateLabels: List<String>,
) {
    companion object {
        val EMPTY = SuzakuImeRenderSnapshot(
            displayText = "",
            selectedIndex = 0,
            candidateLabels = emptyList(),
        )

        fun fromNativePayload(
            payload: Array<out String>,
            candidateLimit: Int = 6,
        ): SuzakuImeRenderSnapshot {
            if (payload.isEmpty()) {
                return EMPTY
            }
            return SuzakuImeRenderSnapshot(
                displayText = payload[0],
                selectedIndex = payload.getOrNull(1)?.toIntOrNull()?.coerceAtLeast(0) ?: 0,
                candidateLabels = payload.drop(2).take(candidateLimit.coerceAtLeast(0)),
            )
        }
    }
}
