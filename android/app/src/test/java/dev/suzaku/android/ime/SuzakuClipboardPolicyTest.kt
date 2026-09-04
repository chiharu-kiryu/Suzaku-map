package dev.suzaku.android.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SuzakuClipboardPolicyTest {
    @Test
    fun secureEditorsBlockClipboardWithoutExposingPreview() {
        val snapshot = SuzakuClipboardPolicy.inspect(
            text = "secret",
            secureInput = true,
        )

        assertEquals(SuzakuClipboardAvailability.BLOCKED, snapshot.availability)
        assertFalse(snapshot.canPaste)
        assertNull(snapshot.preview)
    }

    @Test
    fun emptyClipboardCannotBePasted() {
        val snapshot = SuzakuClipboardPolicy.inspect(text = null, secureInput = false)

        assertEquals(SuzakuClipboardAvailability.EMPTY, snapshot.availability)
        assertFalse(snapshot.canPaste)
        assertNull(snapshot.preview)
    }

    @Test
    fun ordinaryTextGetsCompactPreviewAndRemainsPasteable() {
        val snapshot = SuzakuClipboardPolicy.inspect(
            text = "  hello\n\tSuzaku  ",
            secureInput = false,
        )

        assertEquals(SuzakuClipboardAvailability.READY, snapshot.availability)
        assertTrue(snapshot.canPaste)
        assertEquals("hello Suzaku", snapshot.preview)
    }

    @Test
    fun sensitiveClipboardCanBePastedButNeverPreviewed() {
        val snapshot = SuzakuClipboardPolicy.inspect(
            text = "one-time code 123456",
            secureInput = false,
            sensitiveContent = true,
        )

        assertEquals(SuzakuClipboardAvailability.SENSITIVE, snapshot.availability)
        assertTrue(snapshot.canPaste)
        assertNull(snapshot.preview)
    }

    @Test
    fun sensitiveClipboardCanBeInspectedWithoutMaterializingItsText() {
        val snapshot = SuzakuClipboardPolicy.inspect(
            text = null,
            secureInput = false,
            sensitiveContent = true,
            contentAvailable = true,
        )

        assertEquals(SuzakuClipboardAvailability.SENSITIVE, snapshot.availability)
        assertTrue(snapshot.canPaste)
        assertNull(snapshot.preview)
    }

    @Test
    fun previewTruncationDoesNotSplitUnicodeCodePoints() {
        val snapshot = SuzakuClipboardPolicy.inspect(
            text = "😀😀😀😀",
            secureInput = false,
            previewCodePointLimit = 3,
        )

        assertEquals("😀😀😀…", snapshot.preview)
    }

    @Test
    fun whitespaceOnlyClipboardStaysPasteableWithoutFakePreview() {
        val snapshot = SuzakuClipboardPolicy.inspect(
            text = " \n\t ",
            secureInput = false,
        )

        assertEquals(SuzakuClipboardAvailability.READY, snapshot.availability)
        assertTrue(snapshot.canPaste)
        assertNull(snapshot.preview)
    }
}
