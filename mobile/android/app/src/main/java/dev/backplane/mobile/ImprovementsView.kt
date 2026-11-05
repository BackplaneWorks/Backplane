package dev.backplane.mobile

import android.content.Context
import android.content.Intent
import android.graphics.BitmapFactory
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.ui.semantics.Role
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File

internal class ImprovementsStore(private val context: Context) {
    private val prefs = context.getSharedPreferences("improvements", Context.MODE_PRIVATE)
    val imageFile get() = File(context.filesDir, "improvements-screenshot")
    fun read(): ImprovementsDraft {
        val text = prefs.getString("draft", null) ?: return ImprovementsDraft()
        val o = JSONObject(text)
        return ImprovementsDraft.restored(o.optString("target").takeIf { o.has("target") }, o.optString("text"),
            o.optBoolean("screenshotConsent"), o.optBoolean("opened")).copy(report = o.optString("report"),
                evidenceDetails = o.optString("evidenceDetails"), evidenceReviewed = o.optBoolean("evidenceReviewed"))
    }
    fun save(draft: ImprovementsDraft) {
        check(prefs.edit().putString("draft", JSONObject().put("target", draft.target).put("text", draft.text)
            .put("screenshotConsent", draft.screenshotConsent).put("opened", draft.opened).put("report", draft.report)
            .put("evidenceDetails", draft.evidenceDetails).put("evidenceReviewed", draft.evidenceReviewed).toString()).commit())
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ImprovementsDialog(m: AppModel, close: () -> Unit) {
    val context = LocalContext.current
    val store = remember { ImprovementsStore(context) }
    val restored = remember { runCatching { store.read() } }
    var draft by remember { mutableStateOf(restored.getOrDefault(ImprovementsDraft())) }
    var preview by remember { mutableStateOf<ImprovementsPreview?>(null) }
    var busy by remember { mutableStateOf(false) }
    var confirmNew by remember { mutableStateOf(false) }
    var confirmRetry by remember { mutableStateOf(false) }
    var imageRevision by remember { mutableIntStateOf(0) }
    var note by remember { mutableStateOf(if (restored.isFailure) "Could not restore the saved draft. It has been kept unchanged; close and try again." else "") }
    val scope = rememberCoroutineScope()
    val failedLoad = restored.isFailure
    fun save(next: ImprovementsDraft): Boolean {
        draft = next
        return runCatching { store.save(next) }.fold({ true }, {
            note = "Could not save this draft. Free device storage, then try again."; false
        })
    }
    val image = remember(imageRevision, draft.screenshotConsent) {
        if (draft.screenshotConsent && store.imageFile.exists()) {
            // Downsample previews. Never send this file or a local URI to GitHub.
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeFile(store.imageFile.path, bounds)
            val sample = BitmapFactory.Options().apply { inSampleSize = 1; while (maxOf(bounds.outWidth, bounds.outHeight) / inSampleSize > 1200) inSampleSize *= 2 }
            BitmapFactory.decodeFile(store.imageFile.path, sample)?.asImageBitmap()
        } else null
    }
    val choose = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { uri ->
        if (uri != null && draft.screenshotConsent && !busy) scope.launch {
            busy = true
            val selected = withContext(Dispatchers.IO) { runCatching {
                val bytes = context.contentResolver.openInputStream(uri)?.use { input ->
                    val output = java.io.ByteArrayOutputStream()
                    val buffer = ByteArray(8192)
                    while (output.size() <= 10 * 1024 * 1024) {
                        val count = input.read(buffer, 0, minOf(buffer.size, 10 * 1024 * 1024 + 1 - output.size()))
                        if (count < 0) break
                        output.write(buffer, 0, count)
                    }
                    output.toByteArray()
                }
                    ?: error("No image")
                check(bytes.isNotEmpty() && bytes.size <= 10 * 1024 * 1024)
                val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
                check(bounds.outWidth in 1..16384 && bounds.outHeight in 1..16384)
                val staging = File(context.filesDir, "improvements-screenshot.tmp")
                staging.writeBytes(bytes)
                check(staging.renameTo(store.imageFile))
            } }
            imageRevision++
            if (selected.isSuccess) save(draft.copy(evidenceReviewed = false))
            note = if (selected.isSuccess) "Local preview only. Attach manually on GitHub after reviewing it."
                else "Choose a readable image up to 10 MB. Your draft is kept."
            busy = false
        }
    }
    fun open(url: String, public: Boolean) {
        if (busy || (public && (draft.opened || !draft.evidenceReviewed || (image == null && draft.evidenceDetails.isBlank())))) return
        // Persist the receipt BEFORE crossing to another app. No effects or
        // lifecycle callbacks replay the open, even after process death.
        val previous = draft
        if (!save(if (public) draft.copy(opened = true) else draft)) { draft = previous; return }
        busy = true
        val success = runCatching {
            val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url)).addCategory(Intent.CATEGORY_BROWSABLE)
            // Browser selection is explicit if an installed GitHub app would
            // otherwise intercept the form. No account lookup or auth change.
            context.startActivity(if (public) Intent.createChooser(intent, "Open in a browser to review and sign in") else intent)
        }.isSuccess
        if (!success) {
            if (public) save(draft.copy(opened = false))
            note = "Could not open the browser. Your draft is kept; retry opening."
        } else note = if (public) "Continue on GitHub. You must press Submit there." else "Continue in the hub’s internal Improvements form."
        busy = false
    }
    fun prepare(fill: Boolean) {
        if (busy) return
        scope.launch {
            busy = true; preview = null
            if (save(draft)) {
                val result = runCatching {
                    val json = if (fill) {
                        val version = context.packageManager.getPackageInfo(context.packageName, 0).versionName ?: "unknown"
                        m.improvementsPreview(draft.text, "Android", version, draft.evidenceDetails)
                    } else m.improvementsPreviewBody(draft.report)
                    val o = JSONObject(json)
                    ImprovementsPreview(o.getString("body"), o.getString("url")).also {
                        check(it.url.isEmpty() || it.valid())
                        if (!fill) check(it.body == draft.report)
                    }
                }
                if (fill && result.isSuccess) {
                    save(draft.copy(report = result.getOrThrow().body, opened = false, evidenceReviewed = false))
                    note = "Report filled. Edit the complete body and review its evidence, then preview it."
                } else {
                    preview = result.getOrNull()
                    note = if (result.isFailure) "Public preview is unavailable in this build. Your draft is kept; try again."
                        else if (preview?.url.isNullOrEmpty()) "This body is too long for a prefilled GitHub URL. Shorten your request and preview again. Nothing was truncated."
                        else "Review the exact body before opening GitHub."
                }
            }
            busy = false
        }
    }
    Dialog(onDismissRequest = { if (!busy) close() }, properties = DialogProperties(usePlatformDefaultWidth = false, dismissOnClickOutside = false)) {
        Scaffold(topBar = {
            TopAppBar(title = { Text("Improvements") }, navigationIcon = { TextButton(onClick = close, enabled = !busy) { Text("Close") } })
        }) { padding ->
            Column(Modifier.fillMaxSize().padding(padding).imePadding().verticalScroll(rememberScrollState()).padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text("Send to")
                Row(Modifier.fillMaxWidth().selectable(selected = draft.target == ImprovementsDraft.PUBLIC, enabled = !busy && !failedLoad, role = Role.RadioButton,
                    onClick = { if (save(draft.copy(target = ImprovementsDraft.PUBLIC))) preview = null })) {
                    RadioButton(draft.target == ImprovementsDraft.PUBLIC, null, enabled = !busy && !failedLoad)
                    Text("Public GitHub issue", Modifier.padding(top = 12.dp))
                }
                Row(Modifier.fillMaxWidth().selectable(selected = draft.target == ImprovementsDraft.INTERNAL, enabled = !busy && !failedLoad, role = Role.RadioButton,
                    onClick = { save(draft.copy(target = ImprovementsDraft.INTERNAL)); preview = null })) {
                    RadioButton(draft.target == ImprovementsDraft.INTERNAL, null, enabled = !busy && !failedLoad)
                    Text("Internal agent thread", Modifier.padding(top = 12.dp))
                }
                if (draft.target == ImprovementsDraft.INTERNAL) {
                    Text("Continue the existing internal Improvements workflow in your hub’s browser. Its saved drafts stay internal. Your public draft is kept on this device.")
                    Button(onClick = {
                        val base = m.web("/")
                        if (base == null) note = "Connect to your hub before opening internal Improvements."
                        else open(Uri.parse(base).buildUpon().fragment("improvements-internal").build().toString(), false)
                    }, enabled = !busy && !failedLoad) { Text("Open internal Improvements") }
                } else {
                    OutlinedTextField(draft.text, { preview = null; save(draft.edited(it)) },
                        label = { Text("What would you like to improve?") }, minLines = 5,
                        modifier = Modifier.fillMaxWidth().testTag("improvements.text"), enabled = !busy && !failedLoad)
                    Text("Optional. This opens a public issue draft on BackplaneWorks/Backplane. You review and press Submit on GitHub. The report is attributed to your GitHub account; sign in there using your own account if needed.")
                    OutlinedButton(onClick = {
                        val base = m.web("/")
                        val fragment = "improvements-issue=" + Uri.encode(JSONObject().put("description", draft.text).toString())
                        if (base == null) note = "Connect to your hub before opening the report writer."
                        else if (fragment.length > 24000) note = "Description is too long for a browser handoff. Shorten it, or paste it into the hub’s Improvements form."
                        else open(Uri.parse(base).buildUpon().encodedFragment(fragment).build().toString(), false)
                    }, enabled = !busy && !failedLoad && draft.text.isNotBlank()) { Text("Write report with agent in hub browser") }
                    Text("Hands off only this description to your hub’s public report form. Press its writer button to generate a filled template with writing guidance. Review and edit it there; you can also paste the completed body below. No screenshot is handed off.")
                    Row(Modifier.fillMaxWidth().toggleable(value = draft.screenshotConsent, enabled = !busy && !failedLoad, role = Role.Checkbox,
                        onValueChange = { consent ->
                            if (!consent) { store.imageFile.delete(); imageRevision++ }
                            save(draft.copy(screenshotConsent = consent))
                        }).testTag("improvements.screenshotConsent")) {
                        Checkbox(draft.screenshotConsent, null, enabled = !busy && !failedLoad)
                        Text("I may add a screenshot publicly", Modifier.padding(top = 12.dp))
                    }
                    Text("Screenshots may expose private information. Nothing is captured automatically. A chosen image stays on this device; review it and manually attach it on GitHub if you want it public.")
                    if (draft.screenshotConsent) {
                        OutlinedButton(onClick = { choose.launch("image/*") }, enabled = !busy) { Text("Choose screenshot") }
                        image?.let {
                            Image(it, "Local screenshot preview; not uploaded", Modifier.fillMaxWidth().heightIn(max = 220.dp))
                            TextButton(onClick = { store.imageFile.delete(); imageRevision++; save(draft.copy(evidenceReviewed = false)) }, enabled = !busy) { Text("Remove screenshot") }
                        }
                    }
                    OutlinedTextField(draft.evidenceDetails, { preview = null; save(draft.copy(evidenceDetails = it, opened = false, evidenceReviewed = false)) },
                        label = { Text("Thorough behavior description") }, minLines = 3, modifier = Modifier.fillMaxWidth(), enabled = !busy && !failedLoad)
                    Text("Include steps, expected and actual behavior. Supply a thorough description here or choose an evidence screenshot to attach manually on GitHub.")
                    OutlinedButton(onClick = { prepare(true) }, enabled = !busy && !failedLoad && draft.text.isNotBlank()) { Text("Fill report from description") }
                    OutlinedTextField(draft.report, { preview = null; save(draft.copy(report = it, opened = false, evidenceReviewed = false)) },
                        label = { Text("Editable complete issue body") }, minLines = 8, modifier = Modifier.fillMaxWidth().testTag("improvements.report"), enabled = !busy && !failedLoad)
                    Row(Modifier.fillMaxWidth().toggleable(value = draft.evidenceReviewed, enabled = !busy && !failedLoad, role = Role.Checkbox,
                        onValueChange = { save(draft.copy(evidenceReviewed = it)) })) {
                        Checkbox(draft.evidenceReviewed, null, enabled = !busy && !failedLoad)
                        Text("I reviewed the report’s evidence: an image to attach manually, or a thorough behavior description", Modifier.padding(top = 12.dp))
                    }
                    Text("The body below is sent exactly as edited. Review its OS family and app version too.")
                    Button(onClick = { prepare(false) }, enabled = !busy && !failedLoad && draft.report.isNotBlank()) { Text("Preview exact public issue body") }
                    preview?.let { value ->
                        SelectionContainer { Text(value.body, style = TextStyle(fontFamily = BpMono, fontFeatureSettings = NO_LIGATURES), modifier = Modifier.testTag("improvements.preview")) }
                        Text("Only this body is prefilled. The selected screenshot is not attached. Your hub, project, paths, thread and account details are not added.")
                        Button(onClick = { if (draft.canOpen(value, image != null)) open(value.url, true) }, enabled = !busy && draft.canOpen(value, image != null), modifier = Modifier.testTag("improvements.open")) {
                            Text(if (draft.opened) "GitHub draft already opened" else "Open draft on GitHub")
                        }
                    }
                    if (draft.opened) {
                        Text("Opening GitHub does not confirm submission. Check GitHub before retrying to avoid duplicate issues.")
                        TextButton(onClick = { confirmRetry = true }, enabled = !busy) { Text("Retry opening…") }
                    }
                }
                if (note.isNotEmpty()) Text(note, Modifier.testTag("improvements.status"))
                TextButton(onClick = { confirmNew = true }, enabled = !busy && !failedLoad) { Text("New draft…") }
            }
        }
        if (confirmNew) AlertDialog(onDismissRequest = { confirmNew = false }, title = { Text("Replace the saved draft?") },
            confirmButton = { TextButton(onClick = { confirmNew = false; store.imageFile.delete(); imageRevision++; preview = null; save(ImprovementsDraft()) }) { Text("New public draft") } },
            dismissButton = { TextButton(onClick = { confirmNew = false }) { Text("Cancel") } })
        if (confirmRetry) AlertDialog(onDismissRequest = { confirmRetry = false }, title = { Text("Check GitHub first. Open another copy of this draft?") },
            confirmButton = { TextButton(onClick = { confirmRetry = false; save(draft.copy(opened = false)) }) { Text("Allow another open") } },
            dismissButton = { TextButton(onClick = { confirmRetry = false }) { Text("Cancel") } })
    }
}
