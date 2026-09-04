package dev.suzaku.android.ime

import android.view.animation.PathInterpolator
import android.view.View

class SuzakuImePanelAnimator(
    private val panel: View,
    private val bubble: View,
) {
    private val motionInterpolator = PathInterpolator(0.2f, 0f, 0f, 1f)
    private var expanded: Boolean? = null

    fun sync(expanded: Boolean) {
        this.expanded = expanded
        cancelAnimations()
        if (expanded) {
            panel.visibility = View.VISIBLE
            panel.alpha = 1f
            panel.translationY = 0f
            bubble.visibility = View.GONE
            bubble.alpha = 0f
            bubble.scaleX = 0.92f
            bubble.scaleY = 0.92f
        } else {
            panel.visibility = View.GONE
            panel.alpha = 0f
            panel.translationY = 16f
            bubble.visibility = View.VISIBLE
            bubble.alpha = 1f
            bubble.scaleX = 1f
            bubble.scaleY = 1f
        }
    }

    fun expand() {
        expanded = true
        cancelAnimations()
        if (panel.visibility != View.VISIBLE) {
            panel.alpha = 0f
            panel.translationY = 18f
        }
        panel.visibility = View.VISIBLE
        panel.animate()
            .alpha(1f)
            .translationY(0f)
            .setDuration(140L)
            .setInterpolator(motionInterpolator)
            .withLayer()
            .start()

        if (bubble.visibility != View.VISIBLE) {
            bubble.visibility = View.VISIBLE
            bubble.alpha = 0f
            bubble.scaleX = 0.94f
            bubble.scaleY = 0.94f
        }
        bubble.animate()
            .alpha(0f)
            .scaleX(0.94f)
            .scaleY(0.94f)
            .setDuration(100L)
            .setInterpolator(motionInterpolator)
            .withLayer()
            .withEndAction {
                if (expanded == true) {
                    bubble.visibility = View.GONE
                }
            }
            .start()
    }

    fun collapse() {
        expanded = false
        cancelAnimations()
        if (bubble.visibility != View.VISIBLE) {
            bubble.visibility = View.VISIBLE
            bubble.alpha = 0f
            bubble.scaleX = 0.94f
            bubble.scaleY = 0.94f
        }
        bubble.animate()
            .alpha(1f)
            .scaleX(1f)
            .scaleY(1f)
            .setDuration(120L)
            .setInterpolator(motionInterpolator)
            .withLayer()
            .start()

        panel.animate()
            .alpha(0f)
            .translationY(16f)
            .setDuration(110L)
            .setInterpolator(motionInterpolator)
            .withLayer()
            .withEndAction {
                if (expanded == false) {
                    panel.visibility = View.GONE
                    panel.translationY = 0f
                }
            }
            .start()
    }

    private fun cancelAnimations() {
        panel.animate().cancel()
        bubble.animate().cancel()
    }
}
