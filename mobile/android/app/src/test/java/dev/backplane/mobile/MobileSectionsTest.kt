package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Unit tests for the mobile-native Crew/Inbox/Bench sections contract: status pills, snooze/wake/settle fields, hardware PCB badges, and tab partitioning. */
class MobileSectionsTest {
    private fun row(j: String) = DSecRow.of(JSONObject(j))

    @Test fun parsesNewHubFields() {
        val r = row(
            """{
                "id":"t1","title":"Route USB-C diff pair","word":"running","label":"Working","tone":"run",
                "ago":"2m","project":"Mainboard","nav":"t1","settle":"",
                "snooze":[{"label":"1 hour","value":"t1|1h"},{"label":"Tomorrow","value":"t1|tomorrow"},{"label":"1 week","value":"t1|1w"}]
            }"""
        )
        assertEquals("Working", r.label)
        assertEquals("run", r.tone)
        assertEquals(3, r.snooze.size)
        assertEquals("t1|1h", r.snooze[0].value)
        assertEquals("Tomorrow", r.snooze[1].label)
        assertEquals("", r.wake)
        assertEquals("", r.settle)
    }

    @Test fun parsesHardwareBoardBadgeAndWake() {
        val proj = row("""{"id":"p1","key":"k1","title":"SensorBoard","hasBoard":true,"hasSchematic":true,"pinned":true}""")
        assertTrue(proj.hasBoard)
        assertTrue(proj.hasSchematic)
        assertTrue(proj.pinned)

        val snz = row("""{"id":"t2","title":"Old build","word":"failed","label":"Failed","tone":"bad","wake":"t2|off","ago":"3h"}""")
        assertEquals("t2|off", snz.wake)
        val pill = statusPillOf(snz)!!
        assertEquals(PillKind.FAILED, pill.kind)
        assertEquals(PillTone.BAD_OUTLINE, pill.tone)

        val idleSnz = row("""{"id":"t3","title":"Idle snooze","word":"idle","wake":"t3|off","ago":"1h"}""")
        assertEquals(StatusPillModel(PillKind.SNOOZED, "Snoozed", PillTone.DIM), statusPillOf(idleSnz))
    }

    @Test fun resolvesHubLabelAndTonePills() {
        assertEquals(StatusPillModel(PillKind.APPROVE, "Approve", PillTone.BAD), statusPillOf(row("""{"id":"1","label":"Approve","tone":"bad"}""")))
        assertEquals(StatusPillModel(PillKind.SIGN_IN, "Sign in", PillTone.BAD), statusPillOf(row("""{"id":"2","label":"Sign in","tone":"bad"}""")))
        assertEquals(StatusPillModel(PillKind.INPUT, "Input", PillTone.BAD), statusPillOf(row("""{"id":"3","label":"Input","tone":"bad"}""")))
        assertEquals(StatusPillModel(PillKind.FAILED, "Failed", PillTone.BAD_OUTLINE), statusPillOf(row("""{"id":"4","label":"Failed","tone":"bad"}""")))
        assertEquals(StatusPillModel(PillKind.STOPPED, "Stopped", PillTone.BAD_OUTLINE), statusPillOf(row("""{"id":"5","label":"Stopped","tone":"bad"}""")))
        assertEquals(StatusPillModel(PillKind.WORKING, "Working", PillTone.RUN), statusPillOf(row("""{"id":"6","label":"Working","tone":"run"}""")))
        assertEquals(StatusPillModel(PillKind.WAITING, "Waiting", PillTone.WAIT), statusPillOf(row("""{"id":"7","label":"Waiting","tone":"wait"}""")))
        assertEquals(StatusPillModel(PillKind.DONE, "Done", PillTone.OK), statusPillOf(row("""{"id":"8","label":"Done","tone":"ok"}""")))
    }

    @Test fun fallsBackToLegacyWordAndWantWhenLabelAbsent() {
        assertEquals(StatusPillModel(PillKind.APPROVE, "Approve", PillTone.BAD), statusPillOf(row("""{"id":"1","word":"needs","want":"approval"}""")))
        assertEquals(StatusPillModel(PillKind.SIGN_IN, "Sign in", PillTone.BAD), statusPillOf(row("""{"id":"2","word":"needs","want":"sign-in"}""")))
        assertEquals(StatusPillModel(PillKind.INPUT, "Input", PillTone.BAD), statusPillOf(row("""{"id":"3","word":"needs","want":"question"}""")))
        assertEquals(StatusPillModel(PillKind.FAILED, "Failed", PillTone.BAD_OUTLINE), statusPillOf(row("""{"id":"4","word":"failed"}""")))
        assertEquals(StatusPillModel(PillKind.STOPPED, "Stopped", PillTone.BAD_OUTLINE), statusPillOf(row("""{"id":"5","word":"cut"}""")))
        assertEquals(StatusPillModel(PillKind.WORKING, "Working", PillTone.RUN), statusPillOf(row("""{"id":"6","word":"running"}""")))
        assertEquals(StatusPillModel(PillKind.WAITING, "Waiting", PillTone.WAIT), statusPillOf(row("""{"id":"7","word":"limit"}""")))
        assertEquals(StatusPillModel(PillKind.DONE, "Done", PillTone.OK), statusPillOf(row("""{"id":"8","word":"done"}""")))
        assertNull(statusPillOf(row("""{"id":"9","word":"idle"}""")))
    }

    @Test fun chatsWithNoProjectAreInTheInboxAndMoveToAProject() {
        val sections = DSection.list(JSONArray(
            """[
                {"id":"chats","kind":"threads","title":"Chats","count":1,"rows":[{"id":"c1","title":"quick question","chat":true,"project":"Chats","projectKey":"",
                    "menu":[{"id":"pin","label":"Pin","action":"row-pin","value":"c1"},
                            {"id":"move","label":"Move to project…","action":"","value":"","sub":[{"id":"project","label":"amp","action":"row-move-project","value":"c1\u001fp1"}]}]}]},
                {"id":"snoozed","kind":"threads","title":"Snoozed","count":1,"fold":true,"open":false,"toggle":"sec:snoozed","rows":[]},
                {"id":"active","kind":"projects","title":"Recent","count":0,"rows":[]}
            ]"""
        ))
        val p = partitionSections(sections)
        assertEquals(listOf("chats", "snoozed"), p.inbox.map { it.id })
        assertTrue("chats alone do not end All clear", p.inboxEmpty)
        assertEquals(listOf("chats", "snoozed"), quietInbox(p.inbox).map { it.id })
        val menu = DMenuItem.thread(p.inbox[0].rows[0].menu)
        assertEquals(listOf("pin", "move"), menu.map { it.id })
        val move = menu[1]
        assertTrue(DMenuItem.isMove(move)); assertFalse(DMenuItem.isMove(menu[0]))
        assertEquals("row-move-project", move.sub[0].action); assertEquals("c1\u001fp1", move.sub[0].value)
        // no project to move to: the item is left out
        assertTrue(DMenuItem.thread(DMenuItem.list(JSONArray("""[{"id":"move","label":"Move to project…","sub":[]}]"""))).isEmpty())
    }

    @Test fun partitionsInboxAndBenchAndComputesQuickJump() {
        val raw = JSONArray(
            """[
                {"id":"bub","kind":"bub","title":"","rows":[]},
                {"id":"needs","kind":"threads","title":"Needs you","count":2,"rows":[{"id":"t1","title":"A"},{"id":"t2","title":"B"}]},
                {"id":"running","kind":"threads","title":"Running","count":1,"rows":[{"id":"t3","title":"C"}]},
                {"id":"snoozed","kind":"threads","title":"Snoozed","count":3,"fold":true,"open":false,"toggle":"sec:snoozed","rows":[]},
                {"id":"pinned","kind":"projects","title":"Pinned","count":1,"rows":[{"id":"p1","key":"k1","title":"SensorBoard","hasBoard":true}]},
                {"id":"active","kind":"projects","title":"Recent","count":3,"rows":[
                    {"id":"p1","key":"k1","title":"SensorBoard","hasBoard":true},
                    {"id":"p2","key":"k2","title":"Firmware"},
                    {"id":"p3","key":"k3","title":"PowerSupply","hasBoard":true},
                    {"id":"p4","key":"k4","title":"Enclosure"}
                ]},
                {"id":"all","kind":"projects","title":"All projects","count":30,"fold":true,"open":false,"toggle":"sec:all","rows":[]},
                {"id":"quiet","kind":"projects","title":"Quiet","count":9,"fold":true,"open":false,"toggle":"sec:quiet","hint":"Nothing for two weeks, or made quiet","rows":[]}
            ]"""
        )
        val sections = DSection.list(raw)
        val p = partitionSections(sections)

        assertEquals(listOf("needs", "running", "snoozed"), p.inbox.map { it.id })
        assertEquals(listOf("pinned", "active", "all", "quiet"), p.bench.map { it.id })
        assertEquals(2, p.needsCount)
        assertEquals(1, p.runningCount)
        assertEquals(3, p.snoozedCount)
        assertFalse(p.inboxEmpty)
        assertEquals("Nothing for two weeks, or made quiet", p.bench.last().hint)

        val quick = p.quickJumpProjects(3)
        assertEquals(listOf("k1", "k2", "k3"), quick.map { it.key })
    }

    @Test fun detectsAllClearInboxEvenWhenSnoozedPresent() {
        val raw = JSONArray(
            """[
                {"id":"snoozed","kind":"threads","title":"Snoozed","count":2,"fold":true,"open":false,"toggle":"sec:snoozed","rows":[]},
                {"id":"active","kind":"projects","title":"Recent","count":1,"rows":[{"id":"p1","key":"k1","title":"BoardA","hasBoard":true}]}
            ]"""
        )
        val p = partitionSections(DSection.list(raw))
        assertTrue(p.inboxEmpty)
        assertEquals(0, p.needsCount)
        assertEquals(2, p.snoozedCount)
        assertEquals(1, p.quickJumpProjects(3).size)
    }
}
