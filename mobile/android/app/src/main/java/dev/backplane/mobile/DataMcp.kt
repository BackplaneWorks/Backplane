package dev.backplane.mobile

import androidx.compose.runtime.*
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.CoroutineScope
import org.json.JSONArray
import org.json.JSONObject

/** Settings > MCP servers (feature `mcpsettings`): the hub's rows (native/host/src/mcpx/page.rs). The page keeps the `ui` the hub hands back and what the
 *  person typed; `mcp.page {ui, fields}` draws it, `mcp.page.act {action, value, ui, fields}` presses. The hub parses a paste as it is typed, so a typed form
 *  field asks for the page again after a pause. A vault field's text is never part of `fields`: its Save is `secret.set`, as the chat's masked box. */
object DataMcp {
    val available get() = DataClient.has("mcpsettings")
    var rows by mutableStateOf<List<JSONObject>>(emptyList()); private set
    /** A refused press, said at the top until the next press that works; what was typed stays. */
    var error by mutableStateOf(""); private set
    val form = McpForm()
    private var ui: Map<String, String> = emptyMap()
    // a reply older than the newest request is dropped: a slow refresh never undoes a press
    private var asked = 0; private var busy = 0

    fun reset() { rows = emptyList(); error = ""; ui = emptyMap(); form.clearAll(); asked = 0 }

    private fun uiJson() = JSONObject().also { o -> ui.forEach { (k, v) -> o.put(k, v) } }

    suspend fun load() {
        val n = ++asked
        try {
            val r = DataClient.call("mcp.page", JSONObject().put("ui", uiJson()).put("fields", form.sent()))
            if (n == asked) rows = McpPage.objs(r.optJSONArray("rows"))
        } catch (e: kotlinx.coroutines.CancellationException) { throw e   // a newer keystroke took over: not a failure
        } catch (e: Exception) { if (n == asked) error = e.message ?: "" }
    }

    /** A press of the page (not a vault Save, not open-url: [press] keeps those here). */
    suspend fun act(action: String, value: String) {
        val n = ++asked; busy++
        try {
            val r = DataClient.call("mcp.page.act", JSONObject().put("action", action).put("value", value).put("ui", uiJson()).put("fields", form.sent()))
            ui = McpPage.merge(r.optJSONObject("ui"))
            error = ""
            val cleared = form.after(action, value)
            if (n == asked) rows = McpPage.objs(r.optJSONArray("rows"))
            // the hub drew these rows with the fields as they were sent: once some are emptied, ask again so the form shows them empty
            if (cleared) load()
        } catch (e: kotlinx.coroutines.CancellationException) { throw e
        } catch (e: Exception) { error = e.message ?: "" } finally { busy-- }
    }

    /** The vault's Save: `name\u001fpurpose\u001f` and the row's masked text, as `secret.set`; the text is dropped once the vault has it. */
    suspend fun saveSecret(packed: String, key: String) {
        val (name, purpose) = McpPage.unpack(packed)
        val v = form.vault(key)
        if (name.isEmpty() || v.isEmpty()) return
        try {
            DataClient.call("secret.set", JSONObject().put("name", name).put("value", v).put("purpose", purpose).put("thread", ""))
            form.forgetVault(key); error = ""
        } catch (e: Exception) { error = e.message ?: Msg.android_secret_refused }
        load()
    }

    // ---- live parse and the refresh
    private var typing: Job? = null
    /** A form field changed: ask for the page once typing pauses (McpPage.LIVE_MS), so the hub's preview follows the paste. */
    fun typed(scope: CoroutineScope, name: String, v: String) {
        form.type(name, v)
        if (!McpPage.live(name)) return
        typing?.cancel(); typing = scope.launch { delay(McpPage.LIVE_MS); load() }
    }
    /** The ~4 s refresh while the page is open (a check finishing turns "Checking" into "Ready"); it waits while a press or a pause in typing is pending. */
    suspend fun tick() { if (busy == 0 && typing?.isActive != true) load() }
}

/** What the person typed on the page: plain fields (sent with every request) and vault fields (held here only, by row and field, never sent). */
class McpForm {
    private val plain = mutableStateMapOf<String, String>()
    private val sealed = mutableStateMapOf<String, String>()
    /** The text a plain field shows: the person's own, else the hub's value. */
    fun shown(name: String, hub: String) = plain[name] ?: hub
    fun type(name: String, v: String) { plain[name] = v }
    fun has(name: String) = name in plain
    fun vault(key: String) = sealed[key] ?: ""
    fun typeVault(key: String, v: String) { sealed[key] = v }
    fun forgetVault(key: String) { sealed.remove(key) }
    fun clearAll() { plain.clear(); sealed.clear() }
    /** `fields` as sent: only what was typed (an Edit form shows the server's values until a field is changed), never a vault field. */
    fun sent(): JSONObject = JSONObject().also { o -> plain.forEach { (k, v) -> o.put(k, v) } }
    fun sentMap(): Map<String, String> = plain.toMap()
    /** After a press that worked: what it empties (McpPage.clears). True when anything was emptied. */
    fun after(action: String, value: String): Boolean {
        val c = McpPage.clears(action, value)
        if (c == null) { val any = plain.isNotEmpty() || sealed.isNotEmpty(); clearAll(); return any }
        val any = c.any { it in plain }; c.forEach { plain.remove(it) }; return any
    }
}

/** The page's rules that are not drawing. Pure: McpPageTest. */
object McpPage {
    const val LIVE_MS = 250L
    const val REFRESH_MS = 4000L
    /** The fields of the Add and Edit forms: typing in one asks the hub to parse again. */
    val FORM = setOf("mpaste", "mname", "mcmd", "menv", "mcwd", "murl", "mhdr")
    /** Presses after which every typed field is spent. */
    val SPENT = setOf("mcp-add", "mcp-import", "mcp-gallery", "mcp-save", "mcp-remove", "mcp-dup")
    /** The kinds this app draws; any other row (or none) is a plain row. */
    val KINDS = setOf("head", "group", "card", "tool", "notice", "code", "empty", "panel", "paste", "actions")

    fun live(name: String) = name in FORM

    /** What a press that worked empties: null = everything; the form's fields when the view moves to the list, Add or an Edit (as the hub's page.rs `step`). */
    fun clears(action: String, value: String): Set<String>? = when {
        action in SPENT -> null
        action == "mcp-view" && (value.isEmpty() || value == "add" || value.startsWith("edit:")) -> FORM - "mpaste"
        else -> emptySet()
    }

    /** The page state as the hub returned it: its keys and strings (a null or missing key is gone). */
    fun merge(ui: JSONObject?): Map<String, String> {
        if (ui == null) return emptyMap()
        val m = linkedMapOf<String, String>()
        ui.keys().forEach { k -> val v = ui.opt(k); if (v is String) m[k] = v else if (v != null && v != JSONObject.NULL) m[k] = v.toString() }
        return m
    }

    fun objs(a: JSONArray?): List<JSONObject> = if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) }

    /** A row's kind when this app draws it, else "" (a plain row, PageRows' look). */
    fun kind(r: JSONObject): String = r.optString("kind").takeIf { it in KINDS } ?: ""
    fun inside(r: JSONObject) = r.optBoolean("in")

    /** The rows as drawn: each card with the rows inside it ("in": true, directly under it) as one block. A stray inside row stands alone. */
    data class Block(val head: JSONObject, val inner: List<JSONObject>)
    fun blocks(rows: List<JSONObject>): List<Block> {
        val out = mutableListOf<Block>()
        for (r in rows) {
            val last = out.lastOrNull()
            if (inside(r) && last != null && kind(last.head) == "card") out[out.size - 1] = last.copy(inner = last.inner + r)
            else out += Block(r, emptyList())
        }
        return out
    }

    /** A card's buttons drawn beside it: not those its switch or its tap already are (an older client's stand-ins). */
    fun cardButtons(r: JSONObject): List<JSONObject> {
        val t = r.optJSONObject("toggle")?.optString("action").orEmpty(); val p = r.optJSONObject("press")?.optString("action").orEmpty()
        return objs(r.optJSONArray("buttons")).filter { b -> val a = b.optString("action"); a.isNotEmpty() && a != t && a != p }
    }

    /** The head's main button (`primary`), or -1. */
    fun primary(r: JSONObject): Int = if (r.has("primary")) r.optInt("primary", -1) else -1

    /** "#rrggbb" as an ARGB colour, or null. */
    fun hex(s: String): Long? {
        val h = s.removePrefix("#"); if (!s.startsWith("#") || h.length != 6) return null
        return h.toLongOrNull(16)?.let { 0xFF000000L or it }
    }

    /** A vault Save's value: "name\u001fpurpose\u001f" -> (name, purpose). */
    fun unpack(v: String): Pair<String, String> { val p = v.split('\u001f'); return (p.getOrNull(0) ?: "") to (p.getOrNull(1) ?: "") }

    /** Where a vault field's text is held: per row, so two keys of one server never share a box. */
    fun vaultKey(r: JSONObject, field: String) = r.optString("label") + "\u001f" + field
    fun isVault(f: JSONObject) = f.optBoolean("vault")

    /** The tile's letters' size: two letters smaller than one. */
    fun tileScale(text: String) = if (text.codePointCount(0, text.length) > 1) 0.36f else 0.46f
}
