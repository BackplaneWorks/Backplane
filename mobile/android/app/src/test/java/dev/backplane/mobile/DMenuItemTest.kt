package dev.backplane.mobile

import org.json.JSONArray
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** A sections project row's long press is the hub's menu (present/sections.rs `project_menu`): 11 Nov, it had lost Remove, Rename and Archive. */
class DMenuItemTest {
    private val menu = DMenuItem.list(JSONArray("""[
        {"id":"pin","label":"Pin project","action":"project-pin","value":"k1"},{"id":"swarm","label":"New swarm","action":"swarm-new","value":"p1"},
        {"id":"rename","label":"Rename","action":"proj-rename","value":"p1"},{"id":"badge","label":"Badge","action":"look","value":"p1"},
        {"id":"open","label":"Open folder","action":"open-folder","value":"/w/p1"},
        {"id":"move","label":"Move to group","action":"","value":"","sub":[{"id":"group","label":"Nordic","action":"group-move","value":"k1\u001ffolder:/w/n"},{"id":"new-group","label":"New group…","action":"group-new-start","value":"k1"}]},
        {"id":"archive","label":"Archive","action":"project-archive","value":"k1"},{"id":"remove","label":"Remove project","action":"proj-remove","value":"p1","confirm":true}]"""))

    @Test fun thePhoneListsTheHubsMenuWithoutOpenFolder() {
        assertEquals(listOf("pin", "swarm", "rename", "badge", "move", "archive", "remove"), DMenuItem.phone(menu, true).map { it.id })
        assertEquals(listOf("pin", "rename", "badge", "move", "archive", "remove"), DMenuItem.phone(menu, false).map { it.id }, )
        val rm = menu.last(); assertTrue(rm.confirm); assertEquals("p1", rm.value)
    }
    @Test fun moveToGroupListsTheOtherGroupsThenNewGroup() {
        val sub = menu.first { it.id == "move" }.sub
        assertEquals(listOf("Nordic", "New group…"), sub.map { it.label }); assertEquals("k1\u001ffolder:/w/n", sub[0].value)
        assertTrue(DMenuItem.list(null).isEmpty())
    }
}
