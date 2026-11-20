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
        assertTrue(noGerb[1].disabled)
        assertEquals("No board in this project", noGerb[1].why)
        assertTrue(noGerb[2].on)
        assertFalse(noGerb[2].disabled)

        val withGerb = ViewerRail.phoneTabs(entries, gerbersSupported = true)
        assertEquals(listOf("Files", "Board", "Schematic", "Gerbers"), withGerb.map { it.tab })
    }

    @Test fun fallback_when_empty_and_active_tab_respects_local_press() {
        val fb = ViewerRail.phoneTabs(emptyList(), gerbersSupported = false)
        assertEquals(
            listOf("Board", "Schematic", "3D", "Split", "Mech", "Files", "Diff", "Review", "Browser"),
            fb.map { it.tab },
        )

        val tabs = ViewerRail.phoneTabs(
            listOf(
                ViewerRailEntry("board", "Board", "design", "disabled", "No board"),
                ViewerRailEntry("schematic", "Schematic", "design", "on"),
                ViewerRailEntry("files", "Files", "work", "off"),
            ),
            gerbersSupported = false,
        )
        assertEquals("Schematic", ViewerRail.activeTab(tabs, "Files", userPressed = false, hasFileLink = false))
        assertEquals("Files", ViewerRail.activeTab(tabs, "Files", userPressed = true, hasFileLink = false))
        assertEquals("Files", ViewerRail.activeTab(tabs, "Files", userPressed = false, hasFileLink = true))
    }
}
