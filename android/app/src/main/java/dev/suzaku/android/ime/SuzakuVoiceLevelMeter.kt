package dev.suzaku.android.ime

import android.widget.LinearLayout
import android.view.View

class SuzakuVoiceLevelMeter(
    private val row: LinearLayout,
    private val bars: List<View>,
) {
    private var phase = SuzakuVoicePhase.IDLE
    private var drawerVisible = false
    private var pulseFrame = 0
    private val pulseRunnable = object : Runnable {
        override fun run() {
            if (!shouldAnimate()) {
                render()
                return
            }
            pulseFrame = (pulseFrame + 1) % 12
            render()
            row.postDelayed(this, 140L)
        }
    }

    fun refresh(nextPhase: SuzakuVoicePhase, isDrawerVisible: Boolean) {
        phase = nextPhase
        drawerVisible = isDrawerVisible
        row.removeCallbacks(pulseRunnable)
        render()
        if (shouldAnimate()) {
            row.post(pulseRunnable)
        }
    }

    private fun shouldAnimate(): Boolean =
        drawerVisible && phase in setOf(
            SuzakuVoicePhase.STARTING,
            SuzakuVoicePhase.LISTENING,
            SuzakuVoicePhase.HEARING,
            SuzakuVoicePhase.PROCESSING,
        )

    private fun render() {
        val active = shouldAnimate()
        bars.forEachIndexed { index, bar ->
            val phaseBias = when (phase) {
                SuzakuVoicePhase.HEARING -> 0.95f
                SuzakuVoicePhase.PROCESSING -> 0.8f
                SuzakuVoicePhase.STARTING -> 0.65f
                SuzakuVoicePhase.LISTENING -> 0.72f
                SuzakuVoicePhase.READY -> 0.55f
                else -> 0.35f
            }
            val wave = if (active) ((pulseFrame + index * 2) % 12) / 11f else 0f
            val emphasis = if (active) 0.45f + wave * 0.55f else 0f
            bar.alpha = (phaseBias * 0.55f) + emphasis * 0.45f
            bar.scaleY = 0.85f + emphasis * 0.45f
        }
    }
}
