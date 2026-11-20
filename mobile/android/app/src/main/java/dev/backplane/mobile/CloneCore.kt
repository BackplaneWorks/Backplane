package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject

// Clone from git's pure part (CloneCoreTest): the hub's answers as values (docs/CLONE.md, data/1's clone.* methods), the parameters the screen
// sends, and what the screen does when a clone ends. The clone itself runs in the hub; the screen only draws these.

private fun JSONArray?.strs(): List<String> = if (this == null) emptyList() else (0 until length()).map { optString(it) }.filter { it.isNotEmpty() }

/** A clone job as the hub tells it (`clone.start`/`status`/`retry` and the `clone` push). `pct` null: the bar is indeterminate. */
data class CloneJob(
    val id: String, val repo: String, val origin: String, val dest: String, val destShown: String, val name: String, val state: String,
    val phase: String, val pct: Int?, val detail: String, val error: String, val signin: Boolean, val notes: List<String>,
    val project: String, val thread: String, val ended: Long = 0,
) {
    val running get() = state == "running"
    val stopped get() = state == "failed" || state == "cancelled"
    val terminal get() = state != "running"
    companion object {
        fun of(o: JSONObject?): CloneJob? {
            if (o == null || o.optString("id").isEmpty()) return null
            return CloneJob(o.optString("id"), o.optString("repo"), o.optString("origin"), o.optString("dest"), o.optString("destShown"), o.optString("name"),
                o.optString("state"), o.optString("phase"), if (o.isNull("pct")) null else o.optInt("pct").coerceIn(0, 100), o.optString("detail"), o.optString("error"),
                o.optBoolean("signin"), o.optJSONArray("notes").strs(), o.optString("project"), o.optString("thread"), o.optLong("ended"))
        }
    }
}

/** What the screen does with a clone: stay on it (running, failed, cancelled, or done with notes to read), else go to its thread, else home. */
sealed interface CloneAfter {
    object Stay : CloneAfter
    data class Thread(val id: String) : CloneAfter
    object Home : CloneAfter
}
/** Where a finished clone leads (the button under its notes, or at once when there are none). */
fun cloneLeads(j: CloneJob): CloneAfter = if (j.thread.isNotEmpty()) CloneAfter.Thread(j.thread) else CloneAfter.Home
/** At once, without the person: only a clone done with nothing to read. */
fun cloneAfter(j: CloneJob): CloneAfter = if (j.state != "done" || j.notes.isNotEmpty()) CloneAfter.Stay else cloneLeads(j)

data class CloneRepo(val name: String, val value: String, val description: String, val private: Boolean, val account: String)
/** `clone.repos`: the person's GitHub repositories matching what is typed; `signedout`/`nogh` carry the hub's hint. */
data class CloneRepos(val repos: List<CloneRepo>, val more: Int, val state: String, val loading: Boolean, val hint: String) {
    val offersSignin get() = state == "signedout"
    /** the hint is drawn for these states only (an `error` says so too) */
    val showsHint get() = hint.isNotEmpty() && state != "ok"
    companion object {
        fun of(o: JSONObject): CloneRepos {
            val a = o.optJSONArray("repos")
            val rows = if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) }.map {
                CloneRepo(it.optString("name"), it.optString("value").ifEmpty { it.optString("name") }, it.optString("description"), it.optBoolean("private"), it.optString("account"))
            }.filter { it.value.isNotEmpty() }
            return CloneRepos(rows, o.optInt("more"), o.optString("state"), o.optBoolean("loading"), o.optString("hint"))
        }
    }
}

/** `clone.dest`: where the repository goes and whether it may (`ok`; `same`: a clone of it already, added as it is). `repo` empty: not an address. */
data class CloneDest(
    val ok: Boolean, val error: String, val same: Boolean, val repo: String, val name: String, val dest: String, val destShown: String,
    val parent: String, val folder: String, val defaultParent: String,
) {
    /** the destination step is shown: the text is a repository address and a folder was worked out */
    val known get() = repo.isNotEmpty() && dest.isNotEmpty()
    companion object {
        fun of(o: JSONObject) = CloneDest(o.optBoolean("ok"), o.optString("error"), o.optBoolean("same"), o.optString("repo"), o.optString("name"),
            o.optString("dest"), o.optString("destShown"), o.optString("parent"), o.optString("folder"), o.optString("defaultParent"))
    }
}

/** gh's device sign-in on the hub's machine (`clone.signin` and its push): the code the person enters at `url` on any device. */
data class CloneSignin(val state: String, val code: String, val url: String, val error: String) {
    val open get() = state == "starting" || state == "waiting"
    companion object { fun of(o: JSONObject?): CloneSignin? = o?.let { CloneSignin(it.optString("state"), it.optString("code"), it.optString("url"), it.optString("error")) }?.takeIf { it.state.isNotEmpty() } }
}

/** The repository as `clone.dest`/`clone.start` take it: a picked suggestion's `value`, else the typed text. */
fun cloneRepoArg(picked: CloneRepo?, typed: String): String = picked?.value ?: typed.trim()
/** `clone.dest`'s parameters: the default when nothing was changed; a parent picked or a name typed are sent as they are. */
fun cloneDestParams(repo: String, parent: String, name: String?): JSONObject = JSONObject().put("repo", repo).apply {
    if (parent.isNotEmpty()) put("parent", parent)
    if (name != null) put("name", name.trim())
}
/** `clone.start`'s parameters: the folder `clone.dest` agreed to, the thread choice and /project's first message. */
fun cloneStartParams(repo: String, d: CloneDest, thread: Boolean, text: String, id: String): JSONObject =
    JSONObject().put("id", id).put("repo", repo).put("parent", d.parent).put("name", d.folder).put("thread", thread).apply { if (text.isNotBlank()) put("text", text.trim()) }

/** Two snapshots of one job, whichever order they came in (the start's reply, its pushes, a status after a reconnect): an ended job beats a
 *  running one, whatever came last; two ended ones go by `ended` (the later wins, a tie the newer); two running ones, the newer. */
fun cloneMerge(old: CloneJob?, new: CloneJob): CloneJob = when {
    old == null || old.id != new.id -> new
    old.terminal && !new.terminal -> old
    !old.terminal && new.terminal -> new
    old.terminal && old.ended > new.ended -> old
    else -> new
}
/** A job id this phone names before it asks (`clone.start`'s `id`), so every push for it is known from the first: `a<ms>-<random>`, [A-Za-z0-9_-], at most 64. */
fun cloneNewId(nowMs: Long, rnd: kotlin.random.Random = kotlin.random.Random.Default): String {
    val abc = "abcdefghijklmnopqrstuvwxyz0123456789"
    return "a$nowMs-" + (1..10).map { abc[rnd.nextInt(abc.length)] }.joinToString("")
}
/** What a start asked for, kept by its id for Try again (the job carries the hub's form of the repository, not what was sent). */
data class CloneSpec(val repo: String, val thread: Boolean, val text: String)
/** Try again: a new start of the same clone into the same folder, under a new id. */
fun cloneRetryParams(j: CloneJob, spec: CloneSpec?, id: String): JSONObject =
    JSONObject().put("id", id).put("repo", spec?.repo?.ifEmpty { null } ?: j.repo).put("dest", j.dest).put("thread", spec?.thread ?: true)
        .apply { val t = spec?.text.orEmpty(); if (t.isNotBlank()) put("text", t.trim()) }
