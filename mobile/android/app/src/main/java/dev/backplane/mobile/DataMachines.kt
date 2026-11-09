package dev.backplane.mobile

import androidx.compose.runtime.*
import org.json.JSONArray
import org.json.JSONObject

/** Settings, Machines: the hub's rows (machines.page), drawn by the same PageRows as Voice. A press goes to machines.page.act {action, value, fields}. */
object DataMachines {
    var rows by mutableStateOf<List<JSONObject>>(emptyList()); private set
    var error by mutableStateOf(""); private set
    val available get() = DataClient.has("machines")

    suspend fun load() {
        try { rows = DataClient.call("machines.page").optJSONArray("rows").let { a -> if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) } }; error = "" }
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
    PageRows(Msg.android_machines_head, "machines", DataMachines.rows, DataMachines.error) { action, value, sent -> DataMachines.act(action, value, sent); null }
}
