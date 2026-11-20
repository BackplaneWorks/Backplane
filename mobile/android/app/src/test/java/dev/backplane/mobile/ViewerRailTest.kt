package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ViewerRailTest {
    @Test fun filters_tools_and_unsupported_ids_preserving_hub_order() {
        val json = JSONArray(
            """
            [
              {"id":"files","label":"Files","group":"work","state":"off"},
              {"id":"board","label":"Board","group":"design","state":"disabled","why":"No board in this project"},
              {"id":"schematic","label":"Schematic","group":"design","state":"on"},
              {"id":"gerbers","label":"Gerbers","group":"design","state":"off"},
              {"id":"simulation","label":"Simulation","group":"design","state":"off"},
              {"id":"terminal","label":"Terminal","group":"tools","state":"off"},
              {"id":"commands","label":"Commands","group":"tools","state":"off"},
              {"id":"unknown","label":"Unknown","group":"work","state":"off"}
            ]
            """
        )
        val entries = ViewerRail.parse(json)

        val noGerb = ViewerRail.phoneTabs(entries, gerbersSupported = false)
        assertEquals(listOf("Files", "Board", "Schematic"), noGerb.map { it.tab })
        assertEquals(listOf("Files", "Board", "Schematic"), noGerb.map { it.label })
        assertTrue(noGerb[1].disabled)
        assertEquals("No board in this project", noGerb[1].why)
        assertTrue(noGerb[2].on)
        assertFalse(noGerb[2].disabled)

        val withGerb = ViewerRail.phoneTabs(entries, gerbersSupported = true)
        assertEquals(listOf("Files", "Board", "Schematic", "Gerbers"), withGerb.map { it.tab })
    }

    @Test fun authoritative_empty_reply_stays_empty_and_distinguishes_loading_failure_and_no_feature() {
        assertTrue(ViewerRail.phoneTabs(emptyList(), gerbersSupported = false).isEmpty())
        assertTrue(
            ViewerRail.phoneTabs(
                listOf(
                    ViewerRailEntry("terminal", "Terminal", "tools", "off"),
                    ViewerRailEntry("simulation", "Simulation", "design", "off"),
                ),
                gerbersSupported = false,
            ).isEmpty(),
        )

        val staticTabs = listOf("Board", "Schematic", "3D", "Split", "Mech", "Files", "Diff", "Review", "Browser")

        val noFeat = ViewerRailState(threadId = "t1")
        assertEquals(staticTabs, noFeat.strip(hasRail = false, gerbersSupported = false).map { it.tab })

        val (loading, req1) = noFeat.beginRequest("t1", hasRail = true)
        assertTrue(loading.strip(hasRail = true, gerbersSupported = false).isEmpty())

        val failedNoGood = loading.onFailure(req1, hasRail = true)
        assertEquals(staticTabs, failedNoGood.strip(hasRail = true, gerbersSupported = false).map { it.tab })

        val (loading2, req2) = failedNoGood.beginRequest("t1", hasRail = true)
        val emptyOut = loading2.onReply(req2, emptyList(), hasRail = true, gerbersSupported = false, currentTab = "Board", hasFileLink = false)
        assertTrue(emptyOut.accepted)
        assertEquals("", emptyOut.tab)
        assertTrue(emptyOut.stopWatch)
        assertTrue(emptyOut.state.strip(hasRail = true, gerbersSupported = false).isEmpty())

        val good = listOf(
            ViewerRailEntry("files", "Files", "work", "on"),
            ViewerRailEntry("diff", "Diff", "work", "off"),
        )
        val (loading3, req3) = emptyOut.state.beginRequest("t1", hasRail = true)
        val goodOut = loading3.onReply(req3, good, hasRail = true, gerbersSupported = false, currentTab = "", hasFileLink = false)
        assertEquals(listOf("Files", "Diff"), goodOut.state.strip(hasRail = true, gerbersSupported = false).map { it.tab })
        assertEquals("Files", goodOut.tab)

        val (loading4, req4) = goodOut.state.beginRequest("t1", hasRail = true)
        val failedAfterGood = loading4.onFailure(req4, hasRail = true)
        assertEquals(listOf("Files", "Diff"), failedAfterGood.strip(hasRail = true, gerbersSupported = false).map { it.tab })
    }

    @Test fun reconciles_against_has_even_when_removed_gerbers_entry_is_marked_on() {
        val reply = ViewerRail.parseReply(
            JSONObject(
                """
                {
                  "rail": [
                    {"id":"gerbers","label":"Gerbers","group":"design","state":"on"},
                    {"id":"files","label":"Files","group":"work","state":"off"}
                  ],
                  "has": {"board": false, "schematic": false, "mech": false, "gerbers": false}
                }
                """
            )
        )
        val (s1, req1) = ViewerRailState(threadId = "t1").beginRequest("t1", hasRail = true)
        val out = s1.onReply(req1, reply.entries, has = reply.has, hasRail = true, gerbersSupported = true, currentTab = "Gerbers", hasFileLink = false)
        assertTrue(out.accepted)
        assertEquals("Files", out.tab)
        assertTrue(out.stopWatch)
    }

    @Test fun drops_reply_with_older_local_selection_seq_and_requests_refetch() {
        val entries = listOf(
            ViewerRailEntry("board", "Board", "design", "disabled", "No board"),
            ViewerRailEntry("schematic", "Schematic", "design", "on"),
            ViewerRailEntry("files", "Files", "work", "off"),
        )
        val (s1, req1) = ViewerRailState(threadId = "t1").beginRequest("t1", hasRail = true)
        val afterLocal = s1.onLocalSelect()
        // Even if current tab is "Board" (disabled in the stale reply), an older localSelectionSeq reply is dropped and triggers refetch
        val staleLocal = afterLocal.onReply(req1, entries, hasRail = true, gerbersSupported = false, currentTab = "Board", hasFileLink = false)
        assertFalse(staleLocal.accepted)
        assertTrue(staleLocal.refetch)
        assertFalse(staleLocal.stopWatch)
        assertEquals("Board", staleLocal.tab)

        // Fresh request with matching localSelectionSeq reconciles normally
        val (s2, req2) = afterLocal.beginRequest("t1", hasRail = true)
        val fresh = s2.onReply(req2, entries, hasRail = true, gerbersSupported = false, currentTab = "Board", hasFileLink = false)
        assertTrue(fresh.accepted)
        assertFalse(fresh.refetch)
        assertEquals("Schematic", fresh.tab)
        // Moving watch-owning Board -> watch-owning Schematic does NOT fire a separate stopWatch
        assertFalse(fresh.stopWatch)

        // Older request id, another thread, and feature loss are dropped with refetch = false
        val (s3, req3) = fresh.state.beginRequest("t1", hasRail = true)
        val (s4, req4) = s3.beginRequest("t1", hasRail = true)
        assertFalse(s4.onReply(req3, emptyList(), hasRail = true, gerbersSupported = false, currentTab = "Schematic", hasFileLink = false).accepted)
        val (otherThread, _) = s4.beginRequest("t2", hasRail = true)
        assertFalse(otherThread.onReply(req4, entries, hasRail = true, gerbersSupported = false, currentTab = "Schematic", hasFileLink = false).accepted)
        val lost = s4.onFeatureChanged(hasRail = false)
        assertEquals(ViewerRail.fallbackTabs, lost.strip(hasRail = false, gerbersSupported = false))
        assertFalse(lost.onReply(req4, entries, hasRail = false, gerbersSupported = false, currentTab = "Schematic", hasFileLink = false).accepted)
    }

    @Test fun disabled_tab_tap_issues_zero_watches_and_records_why() {
        val tabs = ViewerRail.phoneTabs(
            listOf(
                ViewerRailEntry("board", "Board", "design", "disabled", "No board in this project"),
                ViewerRailEntry("files", "Files", "work", "on"),
            ),
            gerbersSupported = false,
        )
        val s0 = ViewerRailState(threadId = "t1", status = ViewerRailStatus.LOADED, hasGood = true)
        val tapDisabled = s0.tap(tabs[0], currentTab = "Files")
        assertEquals("Files", tapDisabled.tab)
        assertFalse(tapDisabled.tabChanged)
        assertFalse(tapDisabled.shouldWatch)
        assertFalse(tapDisabled.stopWatch)
        assertEquals("No board in this project", tapDisabled.state.disabledWhy)
        assertEquals(0, tapDisabled.state.localSelectionSeq)

        val tapEnabled = tapDisabled.state.tap(tabs[1], currentTab = "Board")
        assertEquals("Files", tapEnabled.tab)
        assertTrue(tapEnabled.tabChanged)
        assertFalse(tapEnabled.shouldWatch)
        assertTrue(tapEnabled.stopWatch)
        assertEquals("", tapEnabled.state.disabledWhy)
        assertEquals(1, tapEnabled.state.localSelectionSeq)
    }

    @Test fun controller_orders_watch_replacement_and_refetches_stale_local_selection_over_fake_transport() {
        val log = mutableListOf<String>()
        var nextFetchReply: ViewerRailReply? = null
        val transport = object : ViewerRailTransport {
            override fun fetchRail(threadId: String, page: String, diff: Boolean): ViewerRailReply? {
                log += "fetch:$threadId:$page"
                return nextFetchReply
            }
            override fun watch(threadId: String, kind: String) {
                log += "watch:$threadId:$kind"
            }
            override fun stopWatch(threadId: String) {
                log += "stop:$threadId"
            }
        }
        val ctrl = ViewerRailController(transport, ViewerRailState(threadId = "t1"), tab = "Board")

        // 1. Reconcile Board -> Schematic when has.board becomes false: destination replaces watch directly, ZERO stopWatch calls
        val (s1, req1) = ctrl.state.beginRequest("t1", hasRail = true)
        ctrl.state = s1
        val r1 = ViewerRailReply(
            entries = listOf(
                ViewerRailEntry("board", "Board", "design", "on"),
                ViewerRailEntry("schematic", "Schematic", "design", "off"),
                ViewerRailEntry("files", "Files", "work", "off"),
            ),
            has = ViewerRailHas(board = false, schematic = true, mech = false, gerbers = false),
        )
        ctrl.applyReply(req1, r1, hasRail = true, gerbersSupported = true)
        assertEquals("Schematic", ctrl.tab)
        assertEquals(listOf("watch:t1:schematic"), log)

        // 2. Local tap moves to Gerbers while a request is in flight; stale reply is dropped and triggers refetch;
        //    fresh reply reports has.gerbers == false -> awaits stopWatch before landing on Files (non-watch tab)
        log.clear()
        val (s2, req2) = ctrl.state.beginRequest("t1", hasRail = true)
        ctrl.state = s2
        ctrl.selectLocal("Gerbers")
        assertEquals(listOf("watch:t1:gerbers"), log)
        log.clear()

        nextFetchReply = ViewerRailReply(
            entries = listOf(
                ViewerRailEntry("gerbers", "Gerbers", "design", "on"),
                ViewerRailEntry("files", "Files", "work", "off"),
            ),
            has = ViewerRailHas(board = false, schematic = false, mech = false, gerbers = false),
        )
        ctrl.applyReply(req2, r1, hasRail = true, gerbersSupported = true)
        assertEquals("Files", ctrl.tab)
        assertEquals(listOf("fetch:t1:gerbers", "stop:t1"), log)
    }
}
