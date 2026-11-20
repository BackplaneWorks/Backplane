package dev.backplane.mobile

import org.json.JSONArray
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
        // phoneTabs itself never falls back to the static strip
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

        // No feature -> static strip
        val noFeat = ViewerRailState(threadId = "t1")
        assertEquals(staticTabs, noFeat.strip(hasRail = false, gerbersSupported = false).map { it.tab })

        // Loading before first reply -> empty strip (not static strip)
        val (loading, req1) = noFeat.beginRequest("t1", hasRail = true)
        assertTrue(loading.strip(hasRail = true, gerbersSupported = false).isEmpty())

        // Failure with no prior good list -> static strip fallback
        val failedNoGood = loading.onFailure(req1, hasRail = true)
        assertEquals(staticTabs, failedNoGood.strip(hasRail = true, gerbersSupported = false).map { it.tab })

        // Authoritative empty reply -> stays empty, stops watch, clears tab
        val (loading2, req2) = failedNoGood.beginRequest("t1", hasRail = true)
        val emptyOut = loading2.onReply(req2, emptyList(), hasRail = true, gerbersSupported = false, currentTab = "Board", hasFileLink = false)
        assertTrue(emptyOut.accepted)
        assertEquals("", emptyOut.tab)
        assertTrue(emptyOut.stopWatch)
        assertTrue(emptyOut.state.strip(hasRail = true, gerbersSupported = false).isEmpty())

        // Good list followed by failure -> keeps last good list
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

    @Test fun stale_replies_dropped_and_reconciles_newer_tap_file_link_disabled_and_feature_loss() {
        val entries = listOf(
            ViewerRailEntry("board", "Board", "design", "disabled", "No board"),
            ViewerRailEntry("schematic", "Schematic", "design", "on"),
            ViewerRailEntry("files", "Files", "work", "off"),
        )

        // Fresh reply after a newer local tap or file-link open does NOT displace an enabled tab
        val (s1, req1) = ViewerRailState(threadId = "t1").beginRequest("t1", hasRail = true)
        val afterLocal = s1.onLocalSelect()
        val keepLocal = afterLocal.onReply(req1, entries, hasRail = true, gerbersSupported = false, currentTab = "Files", hasFileLink = false)
        assertTrue(keepLocal.accepted)
        assertEquals("Files", keepLocal.tab)
        assertFalse(keepLocal.stopWatch)

        val keepLink = s1.onReply(req1, entries, hasRail = true, gerbersSupported = false, currentTab = "Files", hasFileLink = true)
        assertEquals("Files", keepLink.tab)
        assertFalse(keepLink.stopWatch)

        // Fresh reply when current tab became disabled DOES displace it and stops its watch
        val displaced = afterLocal.onReply(req1, entries, hasRail = true, gerbersSupported = false, currentTab = "Board", hasFileLink = true)
        assertEquals("Schematic", displaced.tab)
        assertTrue(displaced.stopWatch)

        // Older request id and another thread's late reply are dropped
        val (s2, req2) = keepLocal.state.beginRequest("t1", hasRail = true)
        val (s3, req3) = s2.beginRequest("t1", hasRail = true)
        val staleSeq = s3.onReply(req2, emptyList(), hasRail = true, gerbersSupported = false, currentTab = "Files", hasFileLink = false)
        assertFalse(staleSeq.accepted)
        assertEquals("Files", staleSeq.tab)

        val (otherThread, _) = s3.beginRequest("t2", hasRail = true)
        val staleThread = otherThread.onReply(req3, entries, hasRail = true, gerbersSupported = false, currentTab = "Files", hasFileLink = false)
        assertFalse(staleThread.accepted)

        // Feature loss reverts to static strip and drops in-flight replies
        val lost = s3.onFeatureChanged(hasRail = false)
        assertEquals(ViewerRail.fallbackTabs, lost.strip(hasRail = false, gerbersSupported = false))
        val afterLoss = lost.onReply(req3, entries, hasRail = false, gerbersSupported = false, currentTab = "Files", hasFileLink = false)
        assertFalse(afterLoss.accepted)
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
        assertEquals("No board in this project", tapDisabled.state.disabledWhy)
        assertEquals(0, tapDisabled.state.localSelectionSeq)

        val tapEnabled = tapDisabled.state.tap(tabs[1], currentTab = "Board")
        assertEquals("Files", tapEnabled.tab)
        assertTrue(tapEnabled.tabChanged)
        assertTrue(tapEnabled.shouldWatch)
        assertEquals("", tapEnabled.state.disabledWhy)
        assertEquals(1, tapEnabled.state.localSelectionSeq)
    }
}
