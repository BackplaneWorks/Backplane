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
    /** Replace the `back` characters before the mark and the `del` after it with `ins`; the mark goes `adv`, the caret `to` past where that began (the
     *  old mark less `back`; `back` is a voice command taking words back). A mark past the end is the end. */
    fun edit(text: String, mark: Int, del: Int, ins: String, adv: Int, to: Int, back: Int = 0): Applied {
        val n = length(text); val m = mark.coerceIn(0, n); val end = minOf(m + del, n); val s = maxOf(0, m - back)
        val out = text.substring(0, index(text, s)) + ins + text.substring(index(text, end))
        return Applied(out, s + adv, s + to)
    }
    /** Undo of a voice send: the taken text back in front of what was dictated since; the mark moves as typing before it would. */
    fun undo(taken: String, now: String, mark: Int): Pair<String, Int> {
        if (now.isBlank()) return taken to length(taken)
        val t = taken + (if (taken.endsWith("\n")) "" else " ") + now
        return t to (mark + length(t) - length(now))
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
    /** This session's sends while dictating, and the number of the last edit written, per thread (the hub's `Reader::cut`). */
    private val cuts = HashMap<String, Int>(); private val seqs = HashMap<String, Int>()
    private val recorder: VoiceRecorder = VoiceRecorder { th -> scope.launch { ended(th) } }

    val available get() = DataClient.has("voice")
    fun on(tid: String) = views[tid]?.optBoolean("on") == true
    fun hasFixes(tid: String): Boolean { val v = views[tid] ?: return false; return v.optJSONArray("fixes")?.length().let { it != null && it > 0 } || (v.has("ask") && !v.isNull("ask")) }

    private fun show(tid: String, v: JSONObject) { views = views + (tid to v) }
    private fun local(tid: String, note: String) = show(tid, JSONObject().put("on", false).put("note", note).put("bad", true))
    private fun ended(tid: String) { marks.remove(tid); cuts.remove(tid); seqs.remove(tid); level = 0 }

    /** The mic chip: start listening into the thread (the caller holds the RECORD_AUDIO permission), or stop. */
    fun press(tid: String) {
        if (on(tid) || (recorder.active && recorder.thread == tid)) { stop(tid); return }
        val link = DataClient.link ?: return
        val tf = DataClient.field(tid)
        val caret = VoiceText.length(tf.text.substring(0, tf.selection.start.coerceIn(0, tf.text.length)))
        marks[tid] = caret; cuts.remove(tid); seqs.remove(tid)
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
    /** The message went: what was left to clarify in it goes too. While dictating, the words written so far went with it and what is said next starts the
     *  empty draft: the hub is told up to which edit (`seq`), and edits it made before it heard (`cut` behind ours) are dropped. */
    fun sent(tid: String) {
        if (views[tid]?.optBoolean("on") == true) {
            val n = (cuts[tid] ?: 0) + 1; cuts[tid] = n; marks[tid] = 0
            val p = JSONObject().put("thread", tid).put("action", "voice-clear").put("value", "").put("draft", "").put("cut", n).put("seq", seqs[tid] ?: 0)
            scope.launch { runCatching { DataClient.call("voice.act", p) } }
            return
        }
        if (hasFixes(tid)) act(tid, "voice-clear", "")
    }
    /** The person typed: the mark moves with what changed before it. */
    fun typed(tid: String, old: String, new: String) { marks[tid]?.let { marks[tid] = VoiceText.shift(old, new, it) }; if (meta.containsKey(tid)) typedIn.add(tid) }

    /** After opening a thread or a reconnect: what the hub shows now. */
    suspend fun refresh(tid: String) {
        val r = runCatching { DataClient.call("voice.view", JSONObject().put("thread", tid)) }.getOrNull() ?: return
        r.optJSONObject("view")?.let { view(tid, it) }
    }

    private fun view(tid: String, v: JSONObject) {
        v.optJSONObject("always")?.let { if (it.optBoolean("on")) alwaysPaused = it.optString("paused") }
        if (!v.optBoolean("on")) { if (recorder.thread == tid) recorder.gone(); ended(tid) }
        show(tid, v)
        // hold-to-talk let go: the session ended with its last words written; the draft goes, with the undo window
        if (!v.optBoolean("on") && sendOnEnd.remove(tid)) command(tid, "send", 0, JSONObject((meta[tid] ?: JSONObject()).toString()).put("via", "hold"))
    }
    private val sendOnEnd = HashSet<String>(); private var holdStarted: String? = null
    /** Always on (the hub's always.rs), while the app is open: this phone runs it; Settings has it on; why it is paused; the thread open now. */
    var always by mutableStateOf(false); private set
    var alwaysWanted by mutableStateOf(false); private set
    var alwaysPaused by mutableStateOf(""); private set
    private var openThread = ""; private var resumeAlways = false
    /** What the hub knows of each thread's dictated draft (its `meta` frame), and whether the person typed in it since. */
    private val meta = HashMap<String, JSONObject>(); private val typedIn = HashSet<String>()
    /** The message just sent by voice (thread, client id, words): its bubble offers Undo for a moment. */
    data class Recent(val tid: String, val cid: String, val text: String)
    var recent by mutableStateOf<Recent?>(null); private set
    /** The mark a dictated message goes with: the hub's (`base`, or its last `meta`), "edited" when the person typed in it. */
    private fun voiceOf(tid: String, base: JSONObject?): JSONObject { val v = JSONObject((base ?: meta[tid] ?: JSONObject()).toString()); if (typedIn.remove(tid)) v.put("edited", true); meta.remove(tid); return v }
    /** The Send button on a dictated draft: it goes marked as spoken. Null when the draft is not dictated. */
    fun dictated(tid: String): JSONObject? { val m = meta[tid] ?: return null; if (!m.has("conf") || m.isNull("conf")) return null; return voiceOf(tid, m).put("via", "button") }
    /** Undo on the bubble of a message just sent by voice: the hub takes it back (out of the queue, or its turn stopped) and its words return to the draft. */
    fun unsend() {
        val r = recent ?: return; recent = null
        scope.launch { runCatching { DataClient.call("voice.act", JSONObject().put("thread", r.tid).put("action", "voice-undo").put("value", r.cid).put("draft", "")) } }
        scope.launch { runCatching { DataClient.call("voice.act", JSONObject().put("thread", r.tid).put("action", "voice-undid").put("value", "send").put("draft", "")) } }
        val (t, m) = VoiceText.undo(r.text, DataClient.field(r.tid).text, marks[r.tid] ?: 0)
        if (marks.containsKey(r.tid)) marks[r.tid] = m
        DataClient.setField(r.tid, TextFieldValue(t, TextRange(t.length)), typed = false)
    }
    /** The app connected: the hub says whether always-on is wanted (its `always` frame starts it). */
    fun connected() { if (available) scope.launch { runCatching { DataClient.call("voice.always") } } }
    /** The thread the person has open ("" none): the next message goes there (Bub's chat when none). */
    fun open(tid: String) {
        if (openThread == tid) return; openThread = tid
        if (always) scope.launch { runCatching { DataClient.call("voice.act", JSONObject().put("thread", "").put("action", "voice-target").put("value", tid).put("draft", "")) } }
    }
    fun alwaysStart() {
        val link = DataClient.link ?: return
        if (recorder.active) return
        val tid = openThread
        if (!recorder.start(tid, link)) { always = false; return }
        always = true; alwaysPaused = ""; marks[tid] = 0
        scope.launch {
            val path = runCatching { DataClient.call("voice.start", JSONObject().put("thread", tid).put("draft", "").put("caret", 0).put("always", true)).optString("path") }.getOrDefault("")
            if (path.isEmpty()) { recorder.gone(); always = false } else recorder.address(path)
        }
    }
    /** The pill: stop listening (always-on is muted until pressed again), or listen. */
    fun alwaysToggle() {
        if (always) { always = false; scope.launch { withContext(Dispatchers.IO) { recorder.stop() }; runCatching { DataClient.call("voice.stop", JSONObject()) } } } else alwaysStart()
    }
    /** The app went to the background (it stops listening) or came back (it listens again if it was). */
    fun background(away: Boolean) { if (away) { resumeAlways = always; if (always) alwaysToggle() } else if (resumeAlways) { resumeAlways = false; alwaysStart() } }
    /** Hold-to-talk: the hold begins (`start` starts dictation when it is off), or is let go (dictation stops and the message goes once its last words are in). */
    fun held(tid: String, begins: Boolean, start: () -> Unit) {
        if (begins) { if (!on(tid)) { start(); holdStarted = tid } ; return }
        if (holdStarted == tid) { holdStarted = null; sendOnEnd.add(tid); stop(tid) }
    }

    /** A `{"t":"voice"}` frame, to the client that started the session. */
    fun frame(o: JSONObject) {
        val tid = o.optString("thread")
        when (o.optString("ev")) {
            "view" -> o.optJSONObject("view")?.let { view(tid, it) }
            "level" -> level = o.optInt("v").coerceIn(0, 100)
            "edit" -> {
                if (o.has("cut") && o.optInt("cut") < (cuts[tid] ?: 0)) return // made before this phone's send: it belongs to nothing now
                if (o.has("seq")) seqs[tid] = o.optInt("seq")
                val tf = DataClient.field(tid)
                val r = VoiceText.edit(tf.text, marks[tid] ?: VoiceText.length(tf.text), o.optInt("del"), o.optString("ins"), o.optInt("adv"), o.optInt("to"), o.optInt("back"))
                marks[tid] = r.mark
                DataClient.setField(tid, TextFieldValue(r.text, TextRange(VoiceText.index(r.text, r.caret))), typed = false)
            }
            "draft" -> {
                val old = DataClient.field(tid).text; val new = o.optString("text")
                marks[tid]?.let { marks[tid] = VoiceText.shift(old, new, it) }
                DataClient.setField(tid, TextFieldValue(new, TextRange(new.length)), typed = false)
            }
            "command" -> { countdown = null; command(tid, o.optString("cmd"), o.optInt("cut"), o.optJSONObject("voice")) }
            "meta" -> o.optJSONObject("voice")?.let { meta[tid] = it }
            "countdown" -> countdown = if (o.optInt("ms") > 0) tid else null
            "always" -> { alwaysWanted = o.optBoolean("want"); if (alwaysWanted && !o.optBoolean("running") && !always) alwaysStart() else if (!alwaysWanted) { always = false; alwaysPaused = "" } }
            // always on started again by the hub: the microphone goes on into the new session
            "audio" -> { val path = o.optString("path"); val link = DataClient.link; if (always && path.isNotEmpty() && link != null) { if (!recorder.active) recorder.start(tid, link); recorder.address(path) } }
            "page" -> rows = objects(o.optJSONArray("rows"))
        }
    }

    /** A voice command's draft in its undo window (cmds.rs on the hub): a send goes when it ends, a cancel is gone; and what happened, shown a moment. */
    data class Pending(val id: Int, val tid: String, val text: String, val cmd: String)
    var pending by mutableStateOf<Pending?>(null); private set
    var said by mutableStateOf<Pair<String, String>?>(null); private set
    /** The thread whose end-of-turn countdown runs (the hub's `countdown`): it sends when that ends unless more is said or Keep talking is pressed. */
    var countdown by mutableStateOf<String?>(null); private set
    /** Keep talking: no end-of-turn send for this pause. */
    fun hold() { val t = countdown ?: return; countdown = null; scope.launch { runCatching { DataClient.call("voice.act", JSONObject().put("thread", t).put("action", "voice-hold").put("value", "").put("draft", "")) } } }
    private var nextPending = 0
    /** The hub's `command` frame: the whole draft is taken now (dictation goes on into the empty draft), and sent (or dropped, for cancel) after the
     *  undo window. A send goes as the Send button's does: turn.start, which steers a running turn. */
    private fun command(tid: String, cmd: String, cut: Int, voice: JSONObject? = null) {
        if (cmd != "send" && cmd != "cancel") return
        cuts[tid] = maxOf(cuts[tid] ?: 0, cut); marks[tid] = 0
        pending?.let { deliver(it.id) }
        val text = DataClient.field(tid).text; if (text.isBlank()) return
        DataClient.setField(tid, TextFieldValue(""), typed = false)
        if (cmd == "send") {
            // it goes now, shown at once like a typed message; Undo is on its bubble for a moment
            val v = voiceOf(tid, voice); if (!v.has("via")) v.put("via", "command")
            val cid = DataClient.send(tid, text, voice = v); val r = Recent(tid, cid, text); recent = r
            scope.launch { delay(4000); if (recent == r) recent = null }
            return
        }
        val id = ++nextPending; pending = Pending(id, tid, text, cmd)
        scope.launch { delay(3000); deliver(id) }
    }
    fun deliver(id: Int) {
        val p = pending?.takeIf { it.id == id } ?: return
        pending = null
        if (p.cmd == "send") DataClient.send(p.tid, p.text)
        val line = if (p.cmd == "send") Msg.voice_cmd_sent else Msg.voice_cmd_cleared; said = p.tid to line
        scope.launch { delay(2000); if (said?.second == line) said = null }
    }
    /** Undo: the message stays; its text goes back in front of whatever was dictated since. */
    fun undo() {
        val p = pending ?: return; pending = null
        // the hub hears of it: an end-of-turn send undone teaches its threshold to wait a little more (on the hub only)
        scope.launch { runCatching { DataClient.call("voice.act", JSONObject().put("thread", p.tid).put("action", "voice-undid").put("value", p.cmd).put("draft", "")) } }
        val (t, m) = VoiceText.undo(p.text, DataClient.field(p.tid).text, marks[p.tid] ?: 0)
        if (marks.containsKey(p.tid)) marks[p.tid] = m
        DataClient.setField(p.tid, TextFieldValue(t, TextRange(t.length)), typed = false)
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
            else DataClient.call("voice.page.act", JSONObject().put("action", action).put("value", when (action) { "voice-term" -> fields["vterm"] ?: ""; "voice-phrases" -> fields["vphrase"] ?: ""; else -> value }))
            pageError = ""
        } catch (e: Exception) { pageError = e.message ?: "" }
        loadPage()
    }
}
