package dev.backplane.mobile

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONObject

// Add project's Clone from git (docs/CLONE.md; feature "clone"): the repository with the person's GitHub repositories suggested, where it goes
// (clone.dest, the folder browser for the parent), Start a thread, then the clone's progress from the hub's `clone` pushes. The clone runs in the
// hub, on its machine; nothing here touches git or a credential. GitHub's sign-in is gh's device sign-in on the hub's machine: its code is shown here.

/** `/project <address> [first message]` (composer.run's kind "clone"): Add project opens on Clone from git with it. */
data class CloneAsk(val repo: String, val text: String)

object Clones {
    /** every job this hub told about since connecting, by id */
    var jobs by mutableStateOf<Map<String, CloneJob>>(emptyMap()); private set
    /** the clone this phone started (or tried again) and shows: Add project opens on it again while it runs */
    var following by mutableStateOf("")
    var signin by mutableStateOf<CloneSignin?>(null)
    /** the repository list was read again (`clone.repos` push): the suggestions are asked for again */
    var reposTick by mutableStateOf(0); private set
    var ask by mutableStateOf<CloneAsk?>(null)

    fun pushed(o: JSONObject) {
        when (o.optString("k")) {
            "clone" -> CloneJob.of(o.optJSONObject("clone"))?.let(::keep)
            "clone.repos" -> reposTick++
            "clone.signin" -> signin = CloneSignin.of(o.optJSONObject("signin"))
        }
    }
    /** each snapshot merged by recency (cloneMerge): a reply that lands after its job's last push never takes the job back to running */
    private fun keep(j: CloneJob) { val m = cloneMerge(jobs[j.id], j); jobs = jobs + (j.id to m) }
    private suspend fun job(m: String, p: JSONObject): CloneJob = CloneJob.of(DataClient.call(m, p))?.also(::keep) ?: throw DataClient.Refused(Msg.android_failed_generic)
    /** what each start asked for, by id (Try again sends it again) */
    private val specs = mutableMapOf<String, CloneSpec>()

    suspend fun repos(q: String) = CloneRepos.of(DataClient.call("clone.repos", JSONObject().put("q", q.trim()).put("limit", 20)))
    suspend fun dest(p: JSONObject) = CloneDest.of(DataClient.call("clone.dest", p))
    /** The id is named here and followed before the request goes, so a clone that ends before the reply lands is still seen. A refused start
     *  given an id leaves a failed job under it: that job is read and shown like any failure; Err only when there is none. */
    suspend fun start(p: JSONObject, spec: CloneSpec) {
        val id = p.optString("id").ifEmpty { cloneNewId(System.currentTimeMillis()).also { p.put("id", it) } }
        specs[id] = spec; following = id
        try { job("clone.start", p) } catch (e: DataClient.Refused) {
            if (runCatching { status(id) }.isFailure) { following = ""; throw e }
        }
    }
    /** Try again: a new start of the same clone into the same folder (`clone.start` under a new id, not clone.retry). */
    suspend fun retry(j: CloneJob) {
        val spec = specs[j.id]; val id = cloneNewId(System.currentTimeMillis())
        // refused with no job left for it: the failure shown stays, with the refusal under it
        try { start(cloneRetryParams(j, spec, id), spec ?: CloneSpec(j.repo, true, "")) } catch (e: Exception) { if (following.isEmpty()) following = j.id; throw e }
    }
    /** After a reconnect or back on the screen: the followed job as the hub has it now. A hub that no longer knows it: nothing followed. */
    suspend fun reconcile() {
        val id = following; if (id.isEmpty() || DataClient.conn != DataClient.Conn.ONLINE) return
        try { status(id) } catch (e: DataClient.Refused) { if (following == id && jobs[id]?.running != false) following = "" } catch (_: Exception) {}
    }
    suspend fun status(id: String) = job("clone.status", JSONObject().put("id", id))
    suspend fun cancel(id: String) = job("clone.cancel", JSONObject().put("id", id))
    suspend fun signIn() { signin = CloneSignin.of(DataClient.call("clone.signin", JSONObject().put("restart", true))) }
}

/** The hub's words for a phase, a hint or a refusal, in the app's language where the catalog has them. */
private fun hubWords(s: String) = Tr.t(s)

@Composable
internal fun ModeSwitch(clone: Boolean, set: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DBtn(Msg.android_clone_mode_folder, !clone, modifier = Modifier.weight(1f).testTag("proj.mode.folder")) { set(false) }
        DBtn(Msg.android_clone_mode_git, clone, modifier = Modifier.weight(1f).testTag("proj.mode.clone")) { set(true) }
    }
}

/** Clone from git's body under the switch. [opened]: a finished clone's thread (or null: home). */
@Composable
internal fun CloneBody(ask: CloneAsk?, opened: (String?) -> Unit) {
    val scope = rememberCoroutineScope()
    var typed by remember { mutableStateOf(ask?.repo ?: "") }
    var picked by remember { mutableStateOf<CloneRepo?>(null) }
    val first = remember { ask?.text ?: "" }
    var repos by remember { mutableStateOf<CloneRepos?>(null) }
    var parent by remember { mutableStateOf("") }
    var name by remember { mutableStateOf<String?>(null) }   // null: the hub's default name
    var dest by remember { mutableStateOf<CloneDest?>(null) }
    var thread by remember { mutableStateOf(true) }
    var browsing by remember { mutableStateOf(false) }
    var busy by remember { mutableStateOf(false) }
    var err by remember { mutableStateOf("") }
    var signing by remember { mutableStateOf(false) }
    val repo = cloneRepoArg(picked, typed)
    val job = Clones.following.takeIf { it.isNotEmpty() }?.let { Clones.jobs[it] }

    // back on the screen, or back online: the job as the hub has it now (its pushes may have gone by meanwhile)
    LaunchedEffect(Clones.following, DataClient.conn, DataClient.foreground) { if (DataClient.foreground) Clones.reconcile() }
    // the suggestions: asked as the person types (debounced), and again when the hub read the list again
    LaunchedEffect(typed, picked, Clones.reposTick, DataClient.conn) {
        if (picked != null || DataClient.conn != DataClient.Conn.ONLINE) return@LaunchedEffect
        delay(300); repos = runCatching { Clones.repos(typed) }.getOrNull() ?: repos
    }
    // where it goes: the hub's default, or what was picked and typed; again on each change
    LaunchedEffect(repo, parent, name, DataClient.conn) {
        if (repo.isEmpty()) { dest = null; return@LaunchedEffect }
        delay(250); dest = runCatching { Clones.dest(cloneDestParams(repo, parent, name)) }.getOrNull() ?: dest
    }
    // done with nothing to read: on to its thread, else home
    LaunchedEffect(job?.state) { val j = job ?: return@LaunchedEffect; when (val a = cloneAfter(j)) { is CloneAfter.Thread -> { Clones.following = ""; opened(a.id) }; CloneAfter.Home -> { Clones.following = ""; opened(null) }; CloneAfter.Stay -> {} } }

    // Back from the folder browser: back to the destination, not out of Add project
    androidx.activity.compose.BackHandler(browsing) { browsing = false }
    if (browsing) { FolderBrowser(dest?.parent?.ifEmpty { null } ?: dest?.defaultParent?.ifEmpty { null }, Msg.android_clone_folder_use, "clone.parent.use") { parent = it; browsing = false }; return }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        if (job != null) { CloneProgress(job, signing, { signing = it }, opened) { picked = null; typed = job.repo; Clones.following = "" }; return@Column }
        DField(typed, { typed = it; picked = null; name = null; err = "" }, Msg.android_clone_repo_hint, Modifier.testTag("clone.repo"))
        val r = repos
        if (picked == null && r != null) {
            if (r.showsHint) DMono(hubWords(r.hint), dfaint, 12, Modifier.testTag("clone.hint"), lines = 3)
            if (r.offersSignin) DBtn(Msg.android_clone_signin, false, modifier = Modifier.testTag("clone.signin")) { signing = true; scope.launch { runCatching { Clones.signIn() }.onFailure { err = it.message ?: "" } } }
            if (r.loading && r.repos.isEmpty()) DMono(Msg.android_clone_loading, dfaint, 12)
            r.repos.forEach { x -> RepoRow(x) { picked = x; typed = x.name; name = null; err = "" } }
        }
        if (signing) SigninPanel()
        val d = dest
        if (d != null && d.known) {
            DLine()
            DMono(Msg.android_clone_into, dfaint, 11)
            DMono(d.destShown, dcs.onSurface, 13, Modifier.testTag("clone.dest"), bold = true, lines = 2)
            DBtn(Msg.android_clone_folder_change, false, modifier = Modifier.testTag("clone.parent")) { browsing = true }
            DField(name ?: d.folder, { name = it }, Msg.android_clone_name_hint, Modifier.testTag("clone.name"))
            if (d.same) DMono(Msg.android_clone_same, dwarn, 12, lines = 3)
            Row(Modifier.fillMaxWidth().clickable { thread = !thread }.testTag("clone.thread"), verticalAlignment = Alignment.CenterVertically) {
                Text(Msg.android_clone_thread, Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp)
                Switch(thread, { thread = it })
            }
        }
        // a refusal for this folder (or an address the hub can't read, once nothing is suggested for it)
        if (d != null && d.error.isNotEmpty() && (d.known || r?.repos.isNullOrEmpty())) Text(hubWords(d.error), color = dcs.error, fontSize = 13.sp, modifier = Modifier.testTag("clone.dest.error"))
        if (err.isNotEmpty()) Text(hubWords(err), color = dcs.error, fontSize = 13.sp, modifier = Modifier.testTag("clone.error"))
        DBtn(Msg.android_clone_go, true, d != null && d.known && d.ok && d.repo.isNotEmpty() && !busy, Modifier.testTag("clone.go")) {
            val go = d ?: return@DBtn
            busy = true; err = ""
            scope.launch { try { Clones.start(cloneStartParams(repo, go, thread, first, cloneNewId(System.currentTimeMillis())), CloneSpec(repo, thread, first)) } catch (e: Exception) { err = e.message ?: Msg.android_failed_generic } finally { busy = false } }
        }
    }
}

@Composable
private fun RepoRow(r: CloneRepo, pick: () -> Unit) {
    Column(Modifier.fillMaxWidth().clickable(onClick = pick).testTag("clone.repo.${r.value}")) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(vertical = 8.dp)) {
            Column(Modifier.weight(1f)) {
                DMono(r.name, dcs.onSurface, 13, bold = true)
                if (r.description.isNotEmpty()) DMono(r.description, dcs.onSurfaceVariant, 12, lines = 2)
            }
            if (r.private) DMono(Msg.android_clone_private, dfaint, 11, Modifier.border(BorderStroke(1.dp, dcs.outlineVariant)).padding(horizontal = 6.dp, vertical = 2.dp))
        }
        DLine()
    }
}

/** gh's device sign-in: the code to enter (copyable) and GitHub's page (opened in the browser), as the hub's `clone.signin` push moves it. */
@Composable
private fun SigninPanel() {
    val ctx = LocalContext.current
    val s = Clones.signin
    Column(Modifier.fillMaxWidth().border(BorderStroke(1.dp, dcs.outlineVariant)).padding(12.dp).testTag("clone.signin.panel"), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        when {
            s == null || (s.state == "starting" && s.code.isEmpty()) -> DMono(Msg.android_clone_signin_wait, dfaint, 12)
            s.state == "done" -> DMono(Msg.android_clone_signin_done, dok, 13, Modifier.testTag("clone.signin.done"))
            s.state == "failed" || s.state == "cancelled" -> DMono(if (s.error.isNotEmpty()) hubWords(s.error) else Msg.android_clone_signin_failed, dcs.error, 12, lines = 3)
            else -> {
                DMono(Msg.android_clone_signin_code, dcs.onSurfaceVariant, 12, lines = 2)
                if (s.code.isNotEmpty()) {
                    Text(s.code, color = dcs.onSurface, fontFamily = BpMono, fontSize = 26.sp, fontWeight = FontWeight.Bold, modifier = Modifier.testTag("clone.signin.code"))
                    CopyChip(Msg.android_copy_code, s.code, Modifier.testTag("clone.signin.copy"))
                }
                if (s.url.isNotEmpty()) DBtn(Msg.android_signin_open, true, modifier = Modifier.testTag("clone.signin.open")) {
                    runCatching { ctx.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(s.url)).addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)) }
                }
            }
        }
    }
}

/** A clone under way or ended: its phase, bar, detail; Cancel while it runs; the error, Try again and Sign in when it failed; the notes when done. */
@Composable
private fun CloneProgress(j: CloneJob, signing: Boolean, setSigning: (Boolean) -> Unit, opened: (String?) -> Unit, back: () -> Unit) {
    val scope = rememberCoroutineScope()
    var err by remember(j.id) { mutableStateOf("") }
    Text(j.name.ifEmpty { j.repo }, color = dcs.onSurface, fontFamily = BpMono, fontSize = 15.sp, fontWeight = FontWeight.Bold, modifier = Modifier.testTag("clone.job.title"))
    if (j.destShown.isNotEmpty()) DMono(j.destShown, dfaint, 12, lines = 2)
    // a failure says why in its error; the phase it stopped in is not news
    if (j.phase.isNotEmpty() && j.state != "failed") DMono(hubWords(j.phase), dcs.onSurfaceVariant, 13, Modifier.testTag("clone.job.phase"))
    if (j.running) { val p = j.pct; if (p == null) LinearProgressIndicator(Modifier.fillMaxWidth().testTag("clone.job.bar")) else LinearProgressIndicator({ p / 100f }, Modifier.fillMaxWidth().testTag("clone.job.bar")) }
    if (j.detail.isNotEmpty()) DMono(j.detail, dfaint, 12, Modifier.testTag("clone.job.detail"))
    if (j.error.isNotEmpty()) Text(hubWords(j.error), color = dcs.error, fontSize = 13.sp, modifier = Modifier.testTag("clone.job.error"))
    j.notes.forEach { DMono(hubWords(it), dcs.onSurface, 12, Modifier.testTag("clone.job.note"), lines = 6) }
    if (err.isNotEmpty()) Text(hubWords(err), color = dcs.error, fontSize = 13.sp)
    if (signing) SigninPanel()
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        when {
            j.running -> DBtn(Msg.common_cancel, false, modifier = Modifier.testTag("clone.cancel"), danger = true) { scope.launch { runCatching { Clones.cancel(j.id) }.onFailure { err = it.message ?: "" } } }
            j.state == "done" -> DBtn(Msg.android_open, true, modifier = Modifier.testTag("clone.open")) {
                Clones.following = ""; opened((cloneLeads(j) as? CloneAfter.Thread)?.id)
            }
            else -> {
                DBtn(Msg.android_clone_retry, true, modifier = Modifier.testTag("clone.retry")) { setSigning(false); scope.launch { runCatching { Clones.retry(j) }.onFailure { err = it.message ?: "" } } }
                if (j.signin) DBtn(Msg.android_clone_signin, false, modifier = Modifier.testTag("clone.job.signin")) { setSigning(true); scope.launch { runCatching { Clones.signIn() }.onFailure { err = it.message ?: "" } } }
                DBtn(Msg.tour_back, false, modifier = Modifier.testTag("clone.back"), onClick = back)
            }
        }
    }
}
