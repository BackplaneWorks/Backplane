package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

// test/mobile/hubs_cases.json holds this and iOS's HubList.swift to the same answers (scripts/mobile/ios-hub-list-test.sh runs the Swift side,
// test/mobile/hub_list_ref.py the table itself).
class HubListTest {
    private val cases: JSONObject by lazy {
        var f = File("").absoluteFile
        while (!File(f, "test/mobile/hubs_cases.json").exists()) f = f.parentFile
        JSONObject(File(f, "test/mobile/hubs_cases.json").readText())
    }
    private fun list(a: JSONArray) = (0 until a.length()).map { a.getString(it) }
    private fun each(kind: String, check: (JSONObject) -> Unit) { val a = cases.getJSONArray(kind); assertTrue(a.length() > 0); for (i in 0 until a.length()) check(a.getJSONObject(i)) }
    private val key: (String) -> String? = { HubList.authority(it) }

    @Test fun adopt() = each("adopt") { c -> assertEquals(c.getString("name"), list(c.getJSONArray("want")), HubList.adopt(list(c.getJSONArray("links")), c.getString("link"), key)) }
    @Test fun use() = each("use") { c -> assertEquals(c.getString("name"), list(c.getJSONArray("want")), HubList.use(list(c.getJSONArray("links")), c.getString("key"), key)) }
    @Test fun forget() = each("forget") { c -> assertEquals(c.getString("name"), list(c.getJSONArray("want")), HubList.forget(list(c.getJSONArray("links")), c.getString("key"), key)) }
    @Test fun routes() = each("routes") { c ->
        val r = HubList.routes(c.getString("text"))
        assertEquals(c.getString("name"), list(c.getJSONArray("want")), r)
        assertEquals(c.getString("name"), c.getBoolean("local"), HubList.local(r))
    }
    @Test fun storedListsSurviveAnUpgrade() {
        assertEquals(listOf("http://a:1/#token=x"), HubList.decode(null, "http://a:1/#token=x"))
        assertEquals(emptyList<String>(), HubList.decode(null, null))
        val two = listOf("http://a:1/#token=x", "http://b:2/#token=y")
        assertEquals(two, HubList.decode(HubList.encode(two), "http://old:3/#token=z"))
    }
    @Test fun anUpdateFrom014BringsItsPairingsOnceAndNeverOverTheAppsOwn() {
        val old = "[\"http://box:3787/#token=abc\",\"http://10.0.0.2:3787/#token=def\"]"
        org.junit.Assert.assertEquals(listOf("http://box:3787/#token=abc", "http://10.0.0.2:3787/#token=def"), HubList.carried(emptyList(), old, false))
        org.junit.Assert.assertEquals(listOf("mine"), HubList.carried(listOf("mine"), old, false))
        org.junit.Assert.assertEquals(emptyList<String>(), HubList.carried(emptyList(), old, true))
        org.junit.Assert.assertEquals(emptyList<String>(), HubList.carried(emptyList(), null, false))
    }
}
