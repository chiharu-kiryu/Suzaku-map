package dev.suzaku.android.ime

internal enum class SuzakuImeSetupState {
    DISABLED,
    ENABLED,
    ACTIVE,
}

internal data class SuzakuImeSetupStatus(
    val state: SuzakuImeSetupState,
    val enabled: Boolean,
    val selected: Boolean,
) {
    companion object {
        fun resolve(
            serviceId: String,
            enabledServiceIds: Collection<String>,
            selectedServiceId: String?,
        ): SuzakuImeSetupStatus {
            val enabled = serviceId in enabledServiceIds
            val selected = selectedServiceId == serviceId
            val state = when {
                selected -> SuzakuImeSetupState.ACTIVE
                enabled -> SuzakuImeSetupState.ENABLED
                else -> SuzakuImeSetupState.DISABLED
            }
            return SuzakuImeSetupStatus(
                state = state,
                enabled = enabled,
                selected = selected,
            )
        }
    }
}
