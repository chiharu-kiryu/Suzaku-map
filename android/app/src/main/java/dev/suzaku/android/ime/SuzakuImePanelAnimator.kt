package dev.suzaku.android.ime

import android.view.View

class SuzakuImePanelAnimator(
    private val panel: View,
    private val bubble: View,
) {
    fun sync(expanded: Boolean) {
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
        panel.visibility = View.VISIBLE
        panel.alpha = 0f
        panel.translationY = 24f
        panel.animate().alpha(1f).translationY(0f).setDuration(180L).start()

        bubble.animate().alpha(0f).scaleX(0.92f).scaleY(0.92f).setDuration(140L).withEndAction {
            bubble.visibility = View.GONE
        }.start()
    }

    fun collapse() {
        bubble.visibility = View.VISIBLE
        bubble.alpha = 0f
        bubble.scaleX = 0.92f
        bubble.scaleY = 0.92f
        bubble.animate().alpha(1f).scaleX(1f).scaleY(1f).setDuration(160L).start()

        panel.animate().alpha(0f).translationY(20f).setDuration(150L).withEndAction {
            panel.visibility = View.GONE
            panel.translationY = 0f
        }.start()
    }
}
