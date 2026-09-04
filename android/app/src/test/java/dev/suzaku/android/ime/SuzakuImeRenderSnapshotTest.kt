package dev.suzaku.android.ime

import org.junit.Assert.assertEquals
import org.junit.Test

class SuzakuImeRenderSnapshotTest {
    @Test
    fun parsesDisplaySelectionAndCandidatesFromOnePayload() {
        val snapshot = SuzakuImeRenderSnapshot.fromNativePayload(
            arrayOf("ni hao", "2", "你好", "你号", "拟好"),
        )

        assertEquals("ni hao", snapshot.displayText)
        assertEquals(2, snapshot.selectedIndex)
        assertEquals(listOf("你好", "你号", "拟好"), snapshot.candidateLabels)
    }

    @Test
    fun malformedOrMissingMetadataFallsBackSafely() {
        assertEquals(SuzakuImeRenderSnapshot.EMPTY, SuzakuImeRenderSnapshot.fromNativePayload(emptyArray()))

        val snapshot = SuzakuImeRenderSnapshot.fromNativePayload(arrayOf("draft", "invalid", "one"))
        assertEquals(0, snapshot.selectedIndex)
        assertEquals(listOf("one"), snapshot.candidateLabels)
    }

    @Test
    fun candidatePayloadIsBoundedBeforeRendering() {
        val snapshot = SuzakuImeRenderSnapshot.fromNativePayload(
            arrayOf("draft", "0", "1", "2", "3", "4"),
            candidateLimit = 2,
        )

        assertEquals(listOf("1", "2"), snapshot.candidateLabels)
    }
}
