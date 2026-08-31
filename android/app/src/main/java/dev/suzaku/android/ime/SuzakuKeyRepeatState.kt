package dev.suzaku.android.ime

internal class SuzakuKeyRepeatState(
    val initialDelayMillis: Long = 380L,
    val repeatIntervalMillis: Long = 65L,
) {
    var isActive: Boolean = false
        private set
    var hasRepeated: Boolean = false
        private set

    init {
        require(initialDelayMillis > 0L)
        require(repeatIntervalMillis > 0L)
    }

    fun begin(): Long {
        isActive = true
        hasRepeated = false
        return initialDelayMillis
    }

    fun tick(): Long? {
        if (!isActive) {
            return null
        }
        hasRepeated = true
        return repeatIntervalMillis
    }

    fun finish(): Boolean {
        val repeated = isActive && hasRepeated
        isActive = false
        hasRepeated = false
        return repeated
    }
}
