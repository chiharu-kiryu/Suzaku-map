package dev.suzaku.android.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SuzakuImeSetupStatusTest {
    private val serviceId =
        "dev.suzaku.android.ime/dev.suzaku.android.ime.SuzakuInputMethodService"

    @Test
    fun reportsDisabledWhenExactServiceIdIsNotEnabled() {
        val status = SuzakuImeSetupStatus.resolve(
            serviceId = serviceId,
            enabledServiceIds = listOf("dev.suzaku.android.ime/.OtherService"),
            selectedServiceId = null,
        )

        assertEquals(SuzakuImeSetupState.DISABLED, status.state)
        assertFalse(status.enabled)
        assertFalse(status.selected)
    }

    @Test
    fun distinguishesEnabledFromActive() {
        val enabled = SuzakuImeSetupStatus.resolve(
            serviceId = serviceId,
            enabledServiceIds = listOf(serviceId),
            selectedServiceId = "other.package/.Keyboard",
        )
        assertEquals(SuzakuImeSetupState.ENABLED, enabled.state)
        assertTrue(enabled.enabled)
        assertFalse(enabled.selected)

        val active = SuzakuImeSetupStatus.resolve(
            serviceId = serviceId,
            enabledServiceIds = listOf(serviceId),
            selectedServiceId = serviceId,
        )
        assertEquals(SuzakuImeSetupState.ACTIVE, active.state)
        assertTrue(active.enabled)
        assertTrue(active.selected)
    }

    @Test
    fun selectedServiceRemainsActiveDuringTransientEnabledListRefresh() {
        val status = SuzakuImeSetupStatus.resolve(
            serviceId = serviceId,
            enabledServiceIds = emptyList(),
            selectedServiceId = serviceId,
        )

        assertEquals(SuzakuImeSetupState.ACTIVE, status.state)
        assertFalse(status.enabled)
        assertTrue(status.selected)
    }
}
