package dev.suzaku.android.ime

object SuzakuNativeBridge {
    init {
        runCatching { System.loadLibrary("suzaku_map") }
    }

    external fun nativeDescribeImeHost(): String
    external fun nativeDescribePanelCompanion(): String
    external fun nativeDescribeBootstrap(): String

    external fun nativeActivateSession(): Boolean
    external fun nativeDeactivateSession()
    external fun nativeReplaceMarkedText(text: String): Boolean
    external fun nativeClearMarkedText()
    external fun nativeMoveSelection(delta: Int)
    external fun nativeSelectCandidate(index: Int)
    external fun nativeCommitSelected(force: Boolean): Boolean
    external fun nativeCandidateCount(): Int
    external fun nativeSelectedIndex(): Int
    external fun nativeCandidateLabel(index: Int): String
    external fun nativeDisplayText(): String
    external fun nativeTakeLastCommittedText(): String
}
