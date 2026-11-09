package dev.backplane.mobile

import androidx.compose.runtime.*
import org.json.JSONObject

/**
 * The devices page's opaque state (devices.page, devices.page.act): the latest one the hub returned goes back with every call;
 * another hub starts with none, and a reply that comes back from the hub the phone has left is dropped. Pure: DevicesPageTest.
 */
class DevicesState {
    var hub = ""; private set
    var state = ""; private set
    /** The state to send to [h]: none when the phone has moved to another hub since. */
    fun to(h: String): String { if (h != hub) { hub = h; state = "" }; return state }
    /** A reply from [h]: its state kept (a reply without one keeps the last); false when the phone is on another hub now. */
    fun took(h: String, reply: JSONObject): Boolean {
        if (h != hub) return false
        if (reply.has("state")) state = reply.optString("state")
        return true
    }
}

/** Settings, Devices: the hub's rows (devices.page), drawn by PageRows: the address chips, the pairing link with its QR, the local network switch,
 *  the paired devices and open links with Revoke. The hub decides; a press goes to devices.page.act {action, value, state} and its rows replace these. */
object DataDevices {
    var rows by mutableStateOf<List<JSONObject>>(emptyList()); private set
    var error by mutableStateOf(""); private set
    val available get() = DataClient.has("devices")
    private val keep = DevicesState()

    private fun took(h: String, r: JSONObject) {
        if (!keep.took(h, r)) return
        rows = r.optJSONArray("rows").let { a -> if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) } }; error = ""
    }
    private fun hubNow(): String { val h = DataClient.hubName; if (h != keep.hub) { rows = emptyList(); error = "" }; return h }

    // the state along, once the hub has given one
    private fun params(h: String, p: JSONObject = JSONObject()) = p.apply { keep.to(h).takeIf { it.isNotEmpty() }?.let { put("state", it) } }

    suspend fun load() {
        val h = hubNow()
        try { took(h, DataClient.call("devices.page", params(h))) }
        catch (e: Exception) { if (h == keep.hub) error = e.message ?: "" }
    }
    suspend fun act(action: String, value: String) {
        val h = hubNow()
        try { took(h, DataClient.call("devices.page.act", params(h, JSONObject().put("action", action).put("value", value)))) }
        catch (e: Exception) { if (h == keep.hub) error = e.message ?: "" }
    }
}

@Composable
internal fun DevicesSettings() {
    LaunchedEffect(DataClient.conn, DataClient.hubName) { if (DataClient.conn == DataClient.Conn.ONLINE) DataDevices.load() }
    PageRows(Msg.android_devices_head, "devices", DataDevices.rows, DataDevices.error) { action, value, _ -> DataDevices.act(action, value); null }
}
