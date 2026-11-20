package dev.backplane.mobile

import androidx.compose.runtime.*
import org.json.JSONObject

/** Settings, Browser: the hub's rows (browser.page), drawn by the same PageRows as Notifications: the start page a new or empty Browser tab opens, with Save,
 *  Reset to default and Blank. The hub owns the setting and checks the address; a press goes to browser.page.act {action, value, fields} and the page is read again. */
object DataBrowserStart {
    var rows by mutableStateOf<List<JSONObject>>(emptyList()); private set
    var error by mutableStateOf(""); private set
    val available get() = DataClient.has("browserstart")

    suspend fun load() {
        try { rows = DataClient.call("browser.page").optJSONArray("rows").let { a -> if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) } }; error = "" }
        catch (e: Exception) { error = e.message ?: "" }
    }
    /** A press, then the page again; a refusal (an address that is not http or https) stays on the page after the reload. */
    suspend fun act(action: String, value: String, fields: Map<String, String>) {
        val refused = try {
            DataClient.call("browser.page.act", JSONObject().put("action", action).put("value", value).put("fields", JSONObject(fields as Map<*, *>)))
            ""
        } catch (e: Exception) { e.message ?: "" }
        load()
        if (refused.isNotEmpty()) error = refused
    }
}

@Composable
internal fun BrowserStartSettings() {
    LaunchedEffect(DataClient.conn) { if (DataClient.conn == DataClient.Conn.ONLINE) DataBrowserStart.load() }
    PageRows(Msg.android_browser_head, "browser", DataBrowserStart.rows, DataBrowserStart.error) { action, value, sent ->
        // a kept press shows the hub's address again (a bare host with https://, the default after Reset); a refused one keeps what was typed
        DataBrowserStart.act(action, value, sent); if (DataBrowserStart.error.isEmpty()) "bstart" else null }
}
