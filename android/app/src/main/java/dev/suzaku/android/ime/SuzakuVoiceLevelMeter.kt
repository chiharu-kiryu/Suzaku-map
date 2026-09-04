package dev.suzaku.android.ime

import android.widget.LinearLayout
import android.view.View

class SuzakuVoiceLevelMeter(
    private val row: LinearLayout,
    private val bars: List<View>,
) {
    private var phase = SuzakuVoicePhase.IDLE
    private var drawerVisible = false
    private var initialized = false
    private var pulseFrame = 0
    private val pulseRunnable = object : Runnable {
        override fun run() {
            if (!shouldAnimate()) {
                render()
                return
            }
            pulseFrame = (pulseFrame + 1) % 12
            render()
            row.postOnAnimationDelayed(this, 112L)
        }
    }

    fun refresh(nextPhase: SuzakuVoicePhase, isDrawerVisible: Boolean) {
        if (initialized && phase == nextPhase && drawerVisible == isDrawerVisible) {
            return
        }
        initialized = true
        phase = nextPhase
        drawerVisible = isDrawerVisible
        row.removeCallbacks(pulseRunnable)
        render()
        if (shouldAnimate()) {
            row.post(pulseRunnable)
        }
    }

    private fun shouldAnimate(): Boolean = drawerVisible && when (phase) {
        SuzakuVoicePhase.STARTING,
        SuzakuVoicePhase.LISTENING,
        SuzakuVoicePhase.HEARING,
        SuzakuVoicePhase.PROCESSING,
        -> true
        else -> false
    }

    private fun render() {
        val active = shouldAnimate()
        val phaseBias = when (phase) {
            SuzakuVoicePhase.HEARING -> 0.95f
            SuzakuVoicePhase.PROCESSING -> 0.8f
            SuzakuVoicePhase.STARTING -> 0.65f
            SuzakuVoicePhase.LISTENING -> 0.72f
            SuzakuVoicePhase.READY -> 0.55f
            else -> 0.35f
        }
        bars.forEachIndexed { index, bar ->
            val wave = if (active) ((pulseFrame + index * 2) % 12) / 11f else 0f
            val emphasis = if (active) 0.45f + wave * 0.55f else 0f
            bar.alpha = (phaseBias * 0.55f) + emphasis * 0.45f
            bar.scaleY = 0.85f + emphasis * 0.45f
        }
    }
}
