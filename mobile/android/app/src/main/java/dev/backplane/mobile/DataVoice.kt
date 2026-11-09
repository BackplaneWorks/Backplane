package dev.backplane.mobile

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import kotlinx.coroutines.*
import org.json.JSONArray
import org.json.JSONObject

/** The text rules of dictation, in characters (code points), pure: the hub's `edit` frame applied at the thread's mark (place::apply). */
object VoiceText {
    data class Applied(val text: String, val mark: Int, val caret: Int)
    fun length(s: String) = s.codePointCount(0, s.length)
    /** UTF-16 index of the c-th character (the end when c is past it). */
    fun index(s: String, c: Int): Int = s.offsetByCodePoints(0, c.coerceIn(0, length(s)))
    /** Replace the `del` characters after the mark with `ins`; the caret goes to mark + to, the mark to mark + adv. A mark past the end is the end. */
    fun edit(text: String, mark: Int, del: Int, ins: String, adv: Int, to: Int): Applied {
        val n = length(text); val m = mark.coerceIn(0, n); val end = minOf(m + del, n)
        val out = text.substring(0, index(text, m)) + ins + text.substring(index(text, end))
        return Applied(out, m + adv, m + to)
    }
    /** The person typed: the mark moves with what changed before it. */
    fun shift(old: String, new: String, mark: Int): Int {
        val a = old.codePoints().toArray(); val b = new.codePoints().toArray()
        var p = 0
        while (p < a.size && p < b.size && a[p] == b[p]) p++
        if (p >= mark) return mark
        return maxOf(mark + b.size - a.size, p)
    }
}

/** Dictation on the phone (docs/PHONE-DATA.md): the hub decides everything; this keeps the mark per thread, the recorder and what the hub last drew. */
object DataVoice {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    var views by mutableStateOf<Map<String, JSONObject>>(emptyMap()); private set
    var level by mutableIntStateOf(0); private set
    /** Settings, Voice: the hub's rows. */
    var rows by mutableStateOf<List<JSONObject>>(emptyList()); private set
    var pageError by mutableStateOf(""); private set
    private val marks = HashMap<String, Int>()
    private val recorder: VoiceRecorder = VoiceRecorder { th -> scope.launch { ended(th) } }

    val available get() = DataClient.has("voice")
    fun on(tid: String) = views[tid]?.optBoolean("on") == true
    fun hasFixes(tid: String): Boolean { val v = views[tid] ?: return false; return v.optJSONArray("fixes")?.length().let { it != null && it > 0 } || (v.has("ask") && !v.isNull("ask")) }

    private fun show(tid: String, v: JSONObject) { views = views + (tid to v) }
    private fun local(tid: String, note: String) = show(tid, JSONObject().put("on", false).put("note", note).put("bad", true))
    private fun ended(tid: String) { marks.remove(tid); level = 0 }

    /** The mic chip: start listening into the thread (the caller holds the RECORD_AUDIO permission), or stop. */
    fun press(tid: String) {
        if (on(tid) || (recorder.active && recorder.thread == tid)) { stop(tid); return }
        val link = DataClient.link ?: return
        val tf = DataClient.field(tid)
        val caret = VoiceText.length(tf.text.substring(0, tf.selection.start.coerceIn(0, tf.text.length)))
        marks[tid] = caret
        if (!recorder.start(tid, link)) { local(tid, Msg.android_mic_failed); return }
        scope.launch {
            try {
                val path = DataClient.call("voice.start", JSONObject().put("thread", tid).put("draft", tf.text).put("caret", caret)).optString("path")
                if (path.isEmpty()) recorder.gone() else recorder.address(path)
            } catch (e: Exception) { recorder.gone(); local(tid, e.message ?: Msg.android_dictation_failed) }
        }
    }
    fun stop(tid: String) {
        scope.launch {
            withContext(Dispatchers.IO) { recorder.stop() }
            runCatching { DataClient.call("voice.stop", JSONObject().put("thread", tid)) }
        }
    }
    /** A chip, the ask's buttons or the fix field's Save: the hub's action with the draft as it is now. */
    fun act(tid: String, action: String, value: String) {
        scope.launch {
            runCatching { DataClient.call("voice.act", JSONObject().put("thread", tid).put("action", action).put("value", value).put("draft", DataClient.field(tid).text)) }
                .onFailure { local(tid, it.message ?: "") }
        }
    }
    /** The message went: what was left to clarify in it goes too. */
    fun sent(tid: String) { if (hasFixes(tid)) act(tid, "voice-clear", "") }
    /** The person typed: the mark moves with what changed before it. */
    fun typed(tid: String, old: String, new: String) { marks[tid]?.let { marks[tid] = VoiceText.shift(old, new, it) } }

    /** After opening a thread or a reconnect: what the hub shows now. */
    suspend fun refresh(tid: String) {
        val r = runCatching { DataClient.call("voice.view", JSONObject().put("thread", tid)) }.getOrNull() ?: return
        r.optJSONObject("view")?.let { view(tid, it) }
    }

    private fun view(tid: String, v: JSONObject) {
        if (!v.optBoolean("on")) { if (recorder.thread == tid) recorder.gone(); ended(tid) }
        show(tid, v)
    }

    /** A `{"t":"voice"}` frame, to the client that started the session. */
    fun frame(o: JSONObject) {
        val tid = o.optString("thread")
        when (o.optString("ev")) {
            "view" -> o.optJSONObject("view")?.let { view(tid, it) }
            "level" -> level = o.optInt("v").coerceIn(0, 100)
            "edit" -> {
                val tf = DataClient.field(tid)
                val r = VoiceText.edit(tf.text, marks[tid] ?: VoiceText.length(tf.text), o.optInt("del"), o.optString("ins"), o.optInt("adv"), o.optInt("to"))
                marks[tid] = r.mark
                DataClient.setField(tid, TextFieldValue(r.text, TextRange(VoiceText.index(r.text, r.caret))), typed = false)
            }
            "draft" -> {
                val old = DataClient.field(tid).text; val new = o.optString("text")
                marks[tid]?.let { marks[tid] = VoiceText.shift(old, new, it) }
                DataClient.setField(tid, TextFieldValue(new, TextRange(new.length)), typed = false)
            }
            "page" -> rows = objects(o.optJSONArray("rows"))
        }
    }

    private fun objects(a: JSONArray?): List<JSONObject> = if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) }

    // ---- Settings, Voice
    suspend fun loadPage() {
        try { rows = objects(DataClient.call("voice.page").optJSONArray("rows")); pageError = "" } catch (e: Exception) { pageError = e.message ?: "" }
    }
    /** A button of the page. `secret-save` is voice.key with the key field's text; `voice-term` sends the term field's text. */
    suspend fun pageAct(action: String, value: String, fields: Map<String, String>) {
        try {
            if (action == "secret-save") DataClient.call("voice.key", JSONObject().put("key", fields["kval"] ?: ""))
            else DataClient.call("voice.page.act", JSONObject().put("action", action).put("value", if (action == "voice-term") fields["vterm"] ?: "" else value))
            pageError = ""
        } catch (e: Exception) { pageError = e.message ?: "" }
        loadPage()
    }
}
