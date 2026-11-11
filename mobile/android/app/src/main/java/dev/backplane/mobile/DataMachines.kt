package dev.backplane.mobile

import androidx.compose.runtime.*
import org.json.JSONArray
import org.json.JSONObject

/** Settings, Machines: the hub's rows (machines.page), drawn by the same PageRows as Voice. A press goes to machines.page.act {action, value, fields}. */
object DataMachines {
    var rows by mutableStateOf<List<JSONObject>>(emptyList()); private set
    var error by mutableStateOf(""); private set
    /** The hub says the page changes by itself now (a sign-in on another machine runs): read again every 2 s while it does. */
    var busy by mutableStateOf(false); private set
    val available get() = DataClient.has("machines")

    suspend fun load() {
        try { val r = DataClient.call("machines.page"); rows = r.optJSONArray("rows").let { a -> if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) } }; busy = r.optBoolean("busy"); error = "" }
        catch (e: Exception) { error = e.message ?: "" }
    }
    suspend fun act(action: String, value: String, fields: Map<String, String>) {
        try {
            DataClient.call("machines.page.act", JSONObject().put("action", action).put("value", value).put("fields", JSONObject(fields as Map<*, *>)))
            error = ""
        } catch (e: Exception) { error = e.message ?: "" }
        load()
    }
}

@Composable
internal fun MachinesSettings() {
    LaunchedEffect(DataClient.conn) { if (DataClient.conn == DataClient.Conn.ONLINE) DataMachines.load() }
    // while a sign-in there runs, its link, code and screen change without a press: the page is read again every 2 s, as the desktop redraws
    LaunchedEffect(DataClient.conn, DataMachines.busy) {
        while (DataClient.conn == DataClient.Conn.ONLINE && DataMachines.busy) { kotlinx.coroutines.delay(2000); DataMachines.load() }
    }
    // a masked box's Send (the sign-in panel's): its text is emptied once sent, never kept
    PageRows(Msg.android_machines_head, "machines", DataMachines.rows, DataMachines.error) { action, value, sent -> DataMachines.act(action, value, sent); if (action == "secret-save") (if (value == "@signin-sync") "sicode" else "sikeys") else null }
}
