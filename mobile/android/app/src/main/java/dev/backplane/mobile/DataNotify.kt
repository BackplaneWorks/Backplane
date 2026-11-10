package dev.backplane.mobile

import androidx.compose.runtime.*
import org.json.JSONObject

/** Settings, Notifications: the hub's rows (notify.page), drawn by the same PageRows as Voice and Machines: which alerts reach the person, quiet hours, urgent asks,
 *  each phone's mute. The hub owns the settings and the rules; a press goes to notify.page.act {action, value, fields} and the page is read again. */
object DataNotify {
    var rows by mutableStateOf<List<JSONObject>>(emptyList()); private set
    var error by mutableStateOf(""); private set
    val available get() = DataClient.has("notifysettings")

    suspend fun load() {
        try { rows = DataClient.call("notify.page").optJSONArray("rows").let { a -> if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) } }; error = "" }
        catch (e: Exception) { error = e.message ?: "" }
    }
    /** A press, then the page again. A refusal (a quiet time like 25:00) stays on the page after the reload: the reload succeeding is not the press succeeding. */
    suspend fun act(action: String, value: String, fields: Map<String, String>) {
        val refused = try {
            DataClient.call("notify.page.act", JSONObject().put("action", action).put("value", value).put("fields", JSONObject(fields as Map<*, *>)))
            ""
        } catch (e: Exception) { e.message ?: "" }
        load()
        if (refused.isNotEmpty()) error = refused
    }
}

@Composable
internal fun NotifySettings() {
    LaunchedEffect(DataClient.conn) { if (DataClient.conn == DataClient.Conn.ONLINE) DataNotify.load() }
    PageRows(Msg.android_notify_head, "notify", DataNotify.rows, DataNotify.error) { action, value, sent -> DataNotify.act(action, value, sent); null }
}
