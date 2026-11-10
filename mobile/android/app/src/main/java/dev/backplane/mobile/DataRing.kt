package dev.backplane.mobile

import androidx.compose.runtime.*
import org.json.JSONObject

/** Settings, Ring (docs/ring.md): the hub's rows (ring.page), drawn by the same PageRows as Voice and Notifications: the Pebble Index ring's Set up, the
 *  webhook's URL and header to copy or share into the Pebble app on this phone, when the test event came, and the recordings with Play. A press the
 *  phone does itself (copy, share-text, open-url) never leaves it; What happened (link-open) opens the thread here; the rest goes to ring.page.act
 *  {action, value, fields}, whose answer is the page again. */
object DataRing {
    var rows by mutableStateOf<List<JSONObject>>(emptyList()); private set
    var error by mutableStateOf(""); private set
    val available get() = DataClient.has("ring")

    private fun parse(r: JSONObject) = r.optJSONArray("rows").let { a -> if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) } }
    suspend fun load() {
        try { rows = parse(DataClient.call("ring.page")); error = "" } catch (e: Exception) { error = e.message ?: "" }
    }
    suspend fun act(action: String, value: String, fields: Map<String, String>) {
        if (action == "link-open") { DataClient.openLink(value, ""); return }
        try { rows = parse(DataClient.call("ring.page.act", JSONObject().put("action", action).put("value", value).put("fields", JSONObject(fields as Map<*, *>)))); error = "" }
        catch (e: Exception) { val why = e.message ?: ""; load(); error = why }
    }
}

@Composable
internal fun RingSettings() {
    LaunchedEffect(DataClient.conn) { if (DataClient.conn == DataClient.Conn.ONLINE) DataRing.load() }
    PageRows(Msg.android_ring_head, "ring", DataRing.rows, DataRing.error) { action, value, sent -> DataRing.act(action, value, sent); null }
}
