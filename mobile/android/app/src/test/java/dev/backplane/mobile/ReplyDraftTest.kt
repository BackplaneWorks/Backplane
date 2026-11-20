package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ReplyDraftTest {
    private val a = ReplyDraft.of("msg_A:0:aa", "assistant", "\n  All done:  \n- merged")!!

    @Test fun messagesOfferReplyAndOthersDoNot() {
        assertEquals(DReplyRef("msg_A:0:aa", "\n  All done:  \n- merged", "assistant"), a)
        assertEquals("user", ReplyDraft.of("m1", "user", "mine")?.role)
        assertNull(ReplyDraft.of("t1", "tool", "Read"))
        assertNull(ReplyDraft.of("m2", "assistant", "  \n "))
        assertNull("not yet on the hub", ReplyDraft.of("local:c1", "user", "sending"))
        assertNull("still streaming", ReplyDraft.of("live:t1", "assistant", "stream", live = true))
    }

    @Test fun theStripTheSendAndTheChip() {
        assertEquals("All done:", ReplyDraft.firstLine(a.text))
        assertEquals(200, ReplyDraft.firstLine("é".repeat(300)).length)
        assertFalse(ReplyDraft.mine(a)); assertTrue(ReplyDraft.mine(DReplyRef("m", "t", "user")))
        assertEquals(mapOf("id" to "msg_A:0:aa", "text" to "\n  All done:  \n- merged"), ReplyDraft.param(a))
        assertEquals(DReplyTo("msg_A:0:aa", "All done:"), ReplyDraft.chip(a))
    }

    @Test fun aSwipeRightReplies() {
        assertTrue(ReplyDraft.swipe(80f, 10f))
        assertFalse(ReplyDraft.swipe(40f, 0f)); assertFalse(ReplyDraft.swipe(80f, 60f)); assertFalse(ReplyDraft.swipe(-90f, 0f))
    }

    @Test fun aReplyWaitsInTheOutboxWithWhatItAnswers() {
        val o = Outbox().push("h", "t1", "make it faster", "c1", ReplyDraft.param(a)).push("h", "t1", "plain", "c2")
        val back = Outbox.decode(o.encode())
        assertEquals(o, back); assertEquals(ReplyDraft.param(a), back.item("c1")?.replyTo); assertNull(back.item("c2")?.replyTo)
        // an outbox written before replies reads back as plain messages
        assertNull(Outbox.decode("""{"v":1,"items":[{"hub":"h","thread":"t1","text":"hi","clientId":"c3"}]}""").item("c3")?.replyTo)
    }
}
