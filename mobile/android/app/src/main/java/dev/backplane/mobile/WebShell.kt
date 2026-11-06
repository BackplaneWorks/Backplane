package dev.backplane.mobile

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.app.DownloadManager
import android.content.ActivityNotFoundException
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.Message
import android.provider.MediaStore
import android.provider.Settings as AndroidSettings
import android.util.Base64
import android.util.Log
import android.view.HapticFeedbackConstants
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.PermissionRequest
import android.webkit.URLUtil
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import androidx.core.view.WindowCompat
import androidx.webkit.JavaScriptReplyProxy
import androidx.webkit.ScriptHandler
import androidx.webkit.WebMessageCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import kotlin.math.roundToInt

// The app's main screen: the paired hub's phone web client in a WebView
// (docs/phone-app-shell.md). The page draws everything; this keeps what a
// page can't: the pairing (Core's links), the bridge window.backplaneApp
// (mobile/shell/backplane-app.js, injected into the hub's origin only), the
// camera and file pickers, downloads, links out, Back, the text size, and
// the screens around the page (can't reach the hub, the hub list, the
// app's settings). Notifications stay Core's.
// One per activity; the activity keeps it through rotation (configChanges).
class WebShell(private val activity: ComponentActivity, private val model: AppModel) {
    sealed interface Load {
        data object Loading : Load
        data object Ready : Load
        data class Failed(val why: String) : Load
    }
    enum class Over { None, Settings, Hubs, Pair, Repair }

    var load by mutableStateOf<Load>(Load.Loading)
        private set
    var over by mutableStateOf(Over.None)
    // the page's theme (setState), else the phone's
    var light by mutableStateOf<Boolean?>(null)
        private set
    var bg by mutableStateOf<Int?>(null)
        private set
    // the hub on show (Pairing.key) and the WebView drawing it
    var hub by mutableStateOf("")
        private set
    var generation by mutableStateOf(0)
        private set
    private var web: WebView? = null
    private var origin = ""
    private var script: ScriptHandler? = null
    private var listening = false
    // the page's side of the bridge, from its "hello"; events wait for it
    private var proxy: JavaScriptReplyProxy? = null
    private val waiting = mutableListOf<String>()
    private var fileCallback: ValueCallback<Array<Uri>>? = null
    private var cameraOut: Uri? = null
    private var permissionDone: ((Map<String, Boolean>) -> Unit)? = null
    private val shim by lazy { activity.assets.open("backplane-app.js").bufferedReader().readText() }
    private val version by lazy {
        runCatching { activity.packageManager.getPackageInfo(activity.packageName, 0).let { it.versionName.orEmpty() to it.longVersionCode.toString() } }.getOrDefault("" to "")
    }

    // registered before the activity starts (ActivityResult's rule)
    private val files: ActivityResultLauncher<Intent> = activity.registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { r ->
        val cb = fileCallback ?: return@registerForActivityResult
        fileCallback = null
        val picked = if (r.resultCode != Activity.RESULT_OK) null else {
            val data = r.data
            val clip = data?.clipData
            when {
                clip != null && clip.itemCount > 0 -> Array(clip.itemCount) { clip.getItemAt(it).uri }
                data?.data != null -> arrayOf(data.data!!)
                // the camera wrote its photo where it was asked to
                cameraOut != null && cameraFileWritten() -> arrayOf(cameraOut!!)
                else -> null
            }
        }
        cameraOut = null
        cb.onReceiveValue(picked)
    }
    private val permissions: ActivityResultLauncher<Array<String>> = activity.registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { r ->
        val done = permissionDone
        permissionDone = null
        done?.invoke(r)
    }
    private var cameraFile: File? = null
    private fun cameraFileWritten() = (cameraFile?.length() ?: 0L) > 0L

    companion object {
        // a debuggable build, or one started with `am start ... --ez webdebug true`
        // (only adb can): chrome://inspect reaches the page
        var debug = false
        val FEATURES = listOf("camera", "share", "open", "haptic", "hubs", "notify", "files", "settings")
        fun originOf(link: String): String? = Pairing.http(link, "", token = false)?.trimEnd('/')
        private fun tokenOf(link: String): String? = Regex("token=([0-9a-f]{32})").find(link)?.groupValues?.get(1)
    }

    fun webView(): WebView = web ?: create().also { web = it }

    @SuppressLint("SetJavaScriptEnabled")
    private fun create(): WebView {
        if (debug || (activity.applicationInfo.flags and android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE) != 0) WebView.setWebContentsDebuggingEnabled(true)
        val w = WebView(activity)
        w.layoutParams = ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
        w.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            mediaPlaybackRequiresUserGesture = false
            allowFileAccess = false
            allowContentAccess = false
            setSupportMultipleWindows(true)
            javaScriptCanOpenWindowsAutomatically = true
            userAgentString = userAgentString + " BackplaneApp/" + version.first
        }
        textZoom(w)
        // the page's socket brings the hub's alerts (op "notify"): its renderer
        // stays up while the app is away
        w.setRendererPriorityPolicy(WebView.RENDERER_PRIORITY_IMPORTANT, false)
        CookieManager.getInstance().setAcceptCookie(true)
        bg?.let { w.setBackgroundColor(it) }
        w.webViewClient = Client()
        w.webChromeClient = Chrome()
        w.setDownloadListener { url, ua, disposition, mime, _ -> download(url, ua, disposition, mime) }
        listening = false
        script = null
        return w
    }

    // the system's text size (Settings, Display): Android leaves a page at 100%
    fun textZoom(w: WebView? = web) {
        w?.settings?.textZoom = (activity.resources.configuration.fontScale * 100).roundToInt()
    }

    // the hub to show: the one asked for, else the one shown last, else the first paired
    fun pick(links: List<String>): String {
        val keys = links.mapNotNull { Pairing.key(it) }
        return when {
            hub in keys -> hub
            AppSettings.webHub in keys -> AppSettings.webHub
            else -> keys.firstOrNull() ?: ""
        }
    }

    fun show(key: String, again: Boolean = false) {
        val link = model.links.firstOrNull { Pairing.key(it) == key } ?: return
        val o = originOf(link) ?: return
        if (!again && key == hub && o == origin && web != null) return
        val w = webView()
        if (o != origin || !listening) bridge(w, o, key)
        hub = key
        origin = o
        if (AppSettings.webHub != key) AppSettings.putWebHub(key)
        load = Load.Loading
        proxy = null
        // the page trades the token for its own cookie (/auth/pair) and drops
        // it from the address; asking each launch keeps a replaced token working
        val token = tokenOf(link)
        val url = "$o/?phone=1" + (token?.let { "#token=$it" } ?: "")
        // the hub first: a WebView with no network shows the page it cached,
        // which then says it is not paired; this says the hub is out of reach
        val asked = ++probes
        reach(o) { up ->
            if (asked != probes || key != hub) return@reach
            if (up) w.loadUrl(url) else load = Load.Failed("")
        }
    }

    private var probes = 0

    // GET /hello within 4 s, answered on the main thread
    private fun reach(o: String, done: (Boolean) -> Unit) {
        Thread {
            val up = runCatching {
                val h = java.net.URL("$o/hello").openConnection() as java.net.HttpURLConnection
                h.connectTimeout = 4000; h.readTimeout = 4000
                try { h.responseCode in 200..499 } finally { h.disconnect() }
            }.getOrDefault(false)
            activity.runOnUiThread { done(up) }
        }.start()
    }

    // while the hub is out of reach: tried again every few seconds, quietly
    fun retryQuietly() {
        if (load !is Load.Failed || hub.isEmpty()) return
        val link = model.links.firstOrNull { Pairing.key(it) == hub } ?: return
        val o = originOf(link) ?: return
        reach(o) { up -> if (up && load is Load.Failed) retry() }
    }

    fun retry() { if (hub.isNotEmpty()) show(hub, again = true) }

    // window.backplaneApp for this origin only
    private fun bridge(w: WebView, o: String, key: String) {
        if (!WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER) ||
            !WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            Log.i("Backplane", "web shell: this WebView has no message listener; the page runs without the bridge")
            return
        }
        script?.remove()
        if (listening) WebViewCompat.removeWebMessageListener(w, "BackplaneNative")
        val name = model.screen?.hubs?.firstOrNull { it.key == key }?.name ?: key
        val info = JSONObject()
            .put("platform", "android").put("version", version.first).put("build", version.second)
            .put("features", JSONArray(FEATURES))
            .put("hub", JSONObject().put("key", key).put("name", name))
            .put("textScale", activity.resources.configuration.fontScale.toDouble())
        script = WebViewCompat.addDocumentStartJavaScript(w, shim.replace("/*@info*/null", info.toString()), setOf(o))
        WebViewCompat.addWebMessageListener(w, "BackplaneNative", setOf(o)) { _, message: WebMessageCompat, source, main, reply ->
            if (main && source.toString().trimEnd('/') == origin) receive(message.data.orEmpty(), reply)
        }
        listening = true
    }

    private fun receive(text: String, reply: JavaScriptReplyProxy) {
        val msg = runCatching { JSONObject(text) }.getOrNull() ?: return
        val id = msg.optInt("id", 0)
        val args = msg.optJSONObject("args") ?: JSONObject()
        fun answer(value: Any?) { if (id != 0) reply.postMessage(JSONObject().put("id", id).put("ok", true).put("value", value ?: JSONObject.NULL).toString()) }
        fun fail(why: String) { if (id != 0) reply.postMessage(JSONObject().put("id", id).put("ok", false).put("error", why).toString()) }
        when (msg.optString("op")) {
            "hello" -> {
                proxy = reply
                val queued = waiting.toList()
                waiting.clear()
                for (m in queued) reply.postMessage(m)
            }
            "requestCamera" -> ask(listOf(Manifest.permission.CAMERA)) { answer(it[Manifest.permission.CAMERA] == true) }
            "setState" -> state(args)
            "share" -> answer(share(args))
            "openExternal" -> answer(external(Uri.parse(args.optString("url"))))
            "haptic" -> haptic(args.optString("kind"))
            "hubs" -> answer(hubs())
            "switchHub" -> {
                val key = args.optString("key")
                val ok = model.links.any { Pairing.key(it) == key }
                answer(ok)
                if (ok) activity.window.decorView.post { show(key) }
            }
            "addHub" -> over = Over.Pair
            "openAppSettings" -> over = Over.Settings
            "notifications" -> answer(JSONObject().put("allowed", Notes.allowed(activity)).put("asked", asked()))
            "requestNotifications" -> notifications { answer(JSONObject().put("allowed", it)) }
            "pairingFailed" -> if (args.optString("cause") == "hub_unreachable") load = Load.Failed("") else over = Over.Repair
            "notify" -> activity.core.webAlert(Cmd("notify", "", hub = hub, thread = hub + "|" + args.optString("thread"),
                title = args.optString("title"), body = args.optString("body"), kind = args.optString("kind"), key = args.optString("key")))
            else -> fail("unknown op")
        }
    }

    // an event for the page; held until its bridge says hello
    fun emit(event: String, data: JSONObject = JSONObject()) {
        val m = JSONObject().put("event", event).put("data", data).toString()
        val p = proxy
        if (p != null) p.postMessage(m) else waiting += m
    }

    // a tapped alert: its hub's page, then the thread
    fun openThread(key: String, thread: String) {
        if (key.isNotEmpty() && key != hub && model.links.any { Pairing.key(it) == key }) show(key)
        over = Over.None
        emit("open-thread", JSONObject().put("thread", thread))
    }

    private fun state(s: JSONObject) {
        if (s.has("thread")) {
            val t = s.optString("thread")
            activity.core.webThread(if (t.isEmpty()) "" else "$hub|$t")
        }
        if (s.has("theme")) light = when (s.optString("theme")) { "light" -> true; "dark" -> false; else -> null }
        if (s.has("bg")) runCatching { android.graphics.Color.parseColor(s.optString("bg")) }.getOrNull()?.let {
            if (bg != it) { bg = it; web?.setBackgroundColor(it) }
        }
    }

    private fun hubs(): JSONArray {
        val rows = model.screen?.hubs.orEmpty()
        val a = JSONArray()
        for (l in model.links) {
            val k = Pairing.key(l) ?: continue
            val row = rows.firstOrNull { it.key == k }
            a.put(JSONObject().put("key", k).put("name", row?.name ?: k).put("origin", originOf(l) ?: "")
                .put("current", k == hub).put("online", row?.online ?: model.linkUp(k)))
        }
        return a
    }

    private fun share(s: JSONObject): Boolean {
        val text = listOf(s.optString("text"), s.optString("url")).filter { it.isNotBlank() }.joinToString("\n")
        if (text.isEmpty()) return false
        val i = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, text)
        s.optString("title").takeIf { it.isNotBlank() }?.let { i.putExtra(Intent.EXTRA_SUBJECT, it) }
        return runCatching { activity.startActivity(Intent.createChooser(i, s.optString("title").ifBlank { null })) }.isSuccess
    }

    fun external(u: Uri): Boolean {
        if (u.scheme !in setOf("http", "https", "mailto", "tel")) return false
        return try { activity.startActivity(Intent(Intent.ACTION_VIEW, u).addCategory(Intent.CATEGORY_BROWSABLE)); true }
        catch (_: ActivityNotFoundException) { false }
    }

    private fun haptic(kind: String) {
        if (!AppSettings.haptics) return
        val c = when (kind) {
            "medium" -> HapticFeedbackConstants.LONG_PRESS
            "selection" -> HapticFeedbackConstants.CLOCK_TICK
            "success" -> if (Build.VERSION.SDK_INT >= 30) HapticFeedbackConstants.CONFIRM else HapticFeedbackConstants.VIRTUAL_KEY
            "warning", "error" -> if (Build.VERSION.SDK_INT >= 30) HapticFeedbackConstants.REJECT else HapticFeedbackConstants.LONG_PRESS
            else -> HapticFeedbackConstants.VIRTUAL_KEY
        }
        web?.performHapticFeedback(c)
    }

    private fun asked() = activity.getSharedPreferences("backplane", Activity.MODE_PRIVATE).getBoolean("asked-notify", false)

    // the system's question once, then its settings page
    private fun notifications(done: (Boolean) -> Unit) {
        if (Notes.allowed(activity)) { done(true); return }
        if (Build.VERSION.SDK_INT >= 33 && !asked()) {
            activity.getSharedPreferences("backplane", Activity.MODE_PRIVATE).edit().putBoolean("asked-notify", true).apply()
            ask(listOf(Manifest.permission.POST_NOTIFICATIONS)) { done(Notes.allowed(activity)) }
            return
        }
        runCatching {
            activity.startActivity(Intent(AndroidSettings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(AndroidSettings.EXTRA_APP_PACKAGE, activity.packageName))
        }
        done(false)
    }

    // runtime permissions: those held at once, the rest asked (one question at a time)
    private fun ask(wanted: List<String>, done: (Map<String, Boolean>) -> Unit) {
        val held = wanted.associateWith { ContextCompat.checkSelfPermission(activity, it) == PackageManager.PERMISSION_GRANTED }
        val missing = held.filterValues { !it }.keys.toList()
        if (missing.isEmpty() || permissionDone != null) { done(held); return }
        permissionDone = { r -> done(held + r) }
        permissions.launch(missing.toTypedArray())
    }

    private fun sameOrigin(u: Uri): Boolean = u.scheme in setOf("http", "https") &&
        "${u.scheme}://${u.host}${if (u.port > 0) ":" + u.port else ""}" == origin

    // a hub file (its own origin) to the phone's downloads, with the page's cookie
    private fun download(url: String, ua: String, disposition: String?, mime: String?) {
        val u = Uri.parse(url)
        if (!sameOrigin(u)) { external(u); return }
        runCatching {
            val name = URLUtil.guessFileName(url, disposition, mime)
            val r = DownloadManager.Request(u)
                .addRequestHeader("Cookie", CookieManager.getInstance().getCookie(url).orEmpty())
                .addRequestHeader("User-Agent", ua)
                .setMimeType(mime)
                .setTitle(name)
                .setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED)
                .setDestinationInExternalPublicDir(Environment.DIRECTORY_DOWNLOADS, name)
            activity.getSystemService(DownloadManager::class.java).enqueue(r)
            Toast.makeText(activity, "Downloading $name", Toast.LENGTH_SHORT).show()
        }.onFailure { Toast.makeText(activity, "Couldn't download this file", Toast.LENGTH_SHORT).show() }
    }

    // the system Back: the page first (window.backplaneBack: true when it
    // shut something or went back a screen), then its history, then out
    fun back() {
        val w = web
        if (w == null || load != Load.Ready) { activity.moveTaskToBack(true); return }
        w.evaluateJavascript("(function(){try{return window.backplaneBack?String(window.backplaneBack()):'none'}catch(e){return 'none'}})()") { r ->
            when (r?.trim('"')) {
                "true" -> {}
                "false" -> activity.moveTaskToBack(true)
                else -> if (w.canGoBack()) w.goBack() else activity.moveTaskToBack(true)
            }
        }
    }

    // Away: the page must know it is hidden (WebView.onPause makes it so; the
    // WebView left alone stayed "visible"), else it stays the hub's focused
    // desk on its thread and the hub holds this phone's alerts for it
    fun paused() {
        CookieManager.getInstance().flush()
        emit("pause")
        web?.onPause()
    }

    fun resumed(awayMs: Long) {
        web?.onResume()
        if (awayMs > 0) emit("resume", JSONObject().put("awayMs", awayMs))
    }

    fun destroy() {
        web?.destroy()
        web = null
    }

    private inner class Client : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
            val u = request.url
            if (sameOrigin(u)) return false
            // a pairing link inside the page pairs the app
            if (u.scheme == BuildConfig.URL_SCHEME) { model.pair(u.toString()); return true }
            if (!external(u)) Toast.makeText(activity, "No app opens this link", Toast.LENGTH_SHORT).show()
            return true
        }

        override fun onPageStarted(view: WebView, url: String?, favicon: Bitmap?) {
            if (url != null && sameOrigin(Uri.parse(url))) proxy = null
        }

        override fun onPageCommitVisible(view: WebView, url: String?) {
            if (load == Load.Loading) load = Load.Ready
        }

        override fun onPageFinished(view: WebView, url: String?) {
            if (load == Load.Loading) load = Load.Ready
            // the cookie the page just got is on disk now: a kill keeps the pairing
            CookieManager.getInstance().flush()
        }

        override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
            if (request.isForMainFrame) load = Load.Failed(error.description?.toString().orEmpty())
        }

        override fun onReceivedHttpError(view: WebView, request: WebResourceRequest, response: WebResourceResponse) {
            if (request.isForMainFrame && response.statusCode >= 500) load = Load.Failed("HTTP ${response.statusCode}")
        }

        // the page's renderer died (memory): a fresh WebView, the page again
        override fun onRenderProcessGone(view: WebView, detail: android.webkit.RenderProcessGoneDetail): Boolean {
            if (view === web) {
                web = null
                listening = false
                origin = ""
                generation += 1
                load = Load.Failed("The page stopped")
            }
            (view.parent as? ViewGroup)?.removeView(view)
            view.destroy()
            return true
        }
    }

    private inner class Chrome : WebChromeClient() {
        override fun onShowFileChooser(view: WebView, callback: ValueCallback<Array<Uri>>, params: FileChooserParams): Boolean {
            fileCallback?.onReceiveValue(null)
            fileCallback = callback
            val accept = params.acceptTypes.filter { it.isNotBlank() }
            val images = accept.isEmpty() || accept.any { it.startsWith("image/") || it == "*/*" || it.startsWith(".") && it in setOf(".png", ".jpg", ".jpeg") }
            val pick = Intent(Intent.ACTION_GET_CONTENT).addCategory(Intent.CATEGORY_OPENABLE)
                .setType(if (accept.size == 1 && accept[0].contains('/')) accept[0] else "*/*")
                .putExtra(Intent.EXTRA_ALLOW_MULTIPLE, params.mode == FileChooserParams.MODE_OPEN_MULTIPLE)
            if (accept.size > 1) pick.putExtra(Intent.EXTRA_MIME_TYPES, accept.filter { it.contains('/') }.toTypedArray())
            val camera = if (images) cameraIntent() else null
            val intent = when {
                camera != null && params.isCaptureEnabled -> camera
                camera != null -> Intent.createChooser(pick, null).putExtra(Intent.EXTRA_INITIAL_INTENTS, arrayOf(camera))
                else -> pick
            }
            return try { files.launch(intent); true } catch (_: ActivityNotFoundException) { fileCallback = null; false }
        }

        override fun onPermissionRequest(request: PermissionRequest) {
            if (request.origin.toString().trimEnd('/') != origin) { request.deny(); return }
            val need = mutableMapOf<String, String>()
            for (r in request.resources) when (r) {
                PermissionRequest.RESOURCE_VIDEO_CAPTURE -> need[r] = Manifest.permission.CAMERA
                PermissionRequest.RESOURCE_AUDIO_CAPTURE -> need[r] = Manifest.permission.RECORD_AUDIO
            }
            if (need.isEmpty()) { request.deny(); return }
            ask(need.values.distinct()) { got ->
                val ok = need.filter { got[it.value] == true }.keys.toTypedArray()
                if (ok.isEmpty()) request.deny() else request.grant(ok)
            }
        }

        // target=_blank and window.open: a hub file is downloaded, anything else opens outside
        override fun onCreateWindow(view: WebView, dialog: Boolean, gesture: Boolean, result: Message): Boolean {
            val probe = WebView(activity)
            probe.webViewClient = object : WebViewClient() {
                override fun shouldOverrideUrlLoading(v: WebView, r: WebResourceRequest): Boolean {
                    opened(r.url)
                    v.destroy()
                    return true
                }
            }
            (result.obj as WebView.WebViewTransport).webView = probe
            result.sendToTarget()
            return true
        }

        override fun onConsoleMessage(m: android.webkit.ConsoleMessage): Boolean {
            if (m.messageLevel() == android.webkit.ConsoleMessage.MessageLevel.ERROR) Log.w("BackplaneWeb", "${m.message()} (${m.sourceId()}:${m.lineNumber()})")
            return true
        }
    }

    private fun opened(u: Uri) {
        if (sameOrigin(u)) download(u.toString(), web?.settings?.userAgentString.orEmpty(), null, null) else external(u)
    }

    private fun cameraIntent(): Intent? {
        if (!activity.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY)) return null
        return runCatching {
            val dir = File(activity.cacheDir, "camera").apply { mkdirs() }
            val f = File(dir, "photo-${System.currentTimeMillis()}.jpg")
            cameraFile = f
            val uri = FileProvider.getUriForFile(activity, activity.packageName + ".files", f)
            cameraOut = uri
            Intent(MediaStore.ACTION_IMAGE_CAPTURE).putExtra(MediaStore.EXTRA_OUTPUT, uri)
                .addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }.getOrNull()
    }

    // shared into the app (ACTION_SEND): the page puts it in the composer
    fun shareIn(intent: Intent) {
        val text = listOfNotNull(intent.getStringExtra(Intent.EXTRA_SUBJECT), intent.getStringExtra(Intent.EXTRA_TEXT)).filter { it.isNotBlank() }.joinToString("\n")
        val uris = if (intent.action == Intent.ACTION_SEND_MULTIPLE)
            (if (Build.VERSION.SDK_INT >= 33) intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java) else @Suppress("DEPRECATION") intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM)).orEmpty()
        else listOfNotNull(if (Build.VERSION.SDK_INT >= 33) intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java) else @Suppress("DEPRECATION") intent.getParcelableExtra(Intent.EXTRA_STREAM))
        if (text.isEmpty() && uris.isEmpty()) return
        Thread {
            val files = JSONArray()
            for (u in uris.take(10)) runCatching {
                val cr = activity.contentResolver
                val name = cr.query(u, arrayOf(android.provider.OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c -> if (c.moveToFirst()) c.getString(0) else null } ?: u.lastPathSegment ?: "file"
                val bytes = cr.openInputStream(u)?.use { s -> s.readBytes().takeIf { it.size <= 25 shl 20 } } ?: return@runCatching
                files.put(JSONObject().put("name", name).put("type", cr.getType(u) ?: "application/octet-stream").put("data", Base64.encodeToString(bytes, Base64.NO_WRAP)))
            }
            activity.runOnUiThread { emit("share-in", JSONObject().put("text", text).put("files", files)) }
        }.start()
    }
}

// The shell on screen: the page (edge to edge between the system bars and
// above the keyboard), and what covers it while it can't show
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WebShellScreen(shell: WebShell, m: AppModel) {
    val links = m.links
    val view = LocalView.current
    val dark = shell.light?.not() ?: when (AppSettings.appearance) { "light" -> false; "dark" -> true; else -> androidx.compose.foundation.isSystemInDarkTheme() }
    val back = shell.bg?.let { Color(it) } ?: if (dark) Color(0xFF0D0F12) else Color(0xFFF6F7F9)
    // a new hub paired (a link, the camera, the pairing screen): show it
    var known by remember { mutableStateOf(links.mapNotNull { Pairing.key(it) }.toSet()) }
    LaunchedEffect(links) {
        val keys = links.mapNotNull { Pairing.key(it) }
        val added = keys.lastOrNull { it !in known }
        known = keys.toSet()
        if (added != null) { shell.over = WebShell.Over.None; shell.show(added) }
        else shell.show(shell.pick(links))
    }
    SideEffect {
        (view.context as? Activity)?.window?.let { w ->
            WindowCompat.getInsetsController(w, view).apply {
                isAppearanceLightStatusBars = !dark
                isAppearanceLightNavigationBars = !dark
            }
        }
    }
    Box(Modifier.fillMaxSize().background(back)) {
        Box(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            val gen = shell.generation
            androidx.compose.runtime.key(gen) {
                AndroidView(factory = { shell.webView().also { (it.parent as? ViewGroup)?.removeView(it) } }, modifier = Modifier.fillMaxSize())
            }
            when (val l = shell.load) {
                WebShell.Load.Loading -> Box(Modifier.fillMaxSize().background(back), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
                is WebShell.Load.Failed -> Unreachable(shell, m, l.why, back)
                WebShell.Load.Ready -> {}
            }
        }
        when (shell.over) {
            WebShell.Over.None -> {}
            WebShell.Over.Settings -> { BackHandler { shell.over = WebShell.Over.None }; ShellSettings(shell, m) }
            WebShell.Over.Hubs -> {
                BackHandler { shell.over = WebShell.Over.None }
                val s = m.screen
                if (s != null) HubsScreen(m, s) { shell.over = WebShell.Over.None } else ShellSettings(shell, m)
            }
            WebShell.Over.Pair, WebShell.Over.Repair -> {
                BackHandler { shell.over = WebShell.Over.None }
                Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
                    PairScreen(m.screen?.locale ?: "en", cancel = { shell.over = WebShell.Over.None }, status = m.pairingError,
                        repair = shell.over == WebShell.Over.Repair) { m.pair(it) }
                }
            }
        }
    }
}

// the first load did not come: say which hub, and the ways on
@Composable
private fun Unreachable(shell: WebShell, m: AppModel, why: String, back: Color) {
    LaunchedEffect(shell.hub) { while (true) { kotlinx.coroutines.delay(5000); shell.retryQuietly() } }
    val name = m.screen?.hubs?.firstOrNull { it.key == shell.hub }?.name ?: shell.hub
    Column(Modifier.fillMaxSize().background(back).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterVertically),
        horizontalAlignment = Alignment.CenterHorizontally) {
        Text("Can't reach $name", style = MaterialTheme.typography.headlineSmall, color = MaterialTheme.colorScheme.onBackground, textAlign = TextAlign.Center)
        Text("Check that the computer is on and that this phone is on its network or tailnet." + (if (why.isNotBlank()) "\n($why)" else ""),
            style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onBackground.copy(alpha = 0.75f), textAlign = TextAlign.Center)
        Button(onClick = { shell.retry() }, Modifier.fillMaxWidth()) { Text("Try again") }
        if (m.links.size > 1) OutlinedButton(onClick = { shell.over = WebShell.Over.Settings }, Modifier.fillMaxWidth()) { Text("Another hub") }
        OutlinedButton(onClick = { shell.over = WebShell.Over.Settings }, Modifier.fillMaxWidth()) { Text("App settings") }
    }
}

// The app's own settings, over the page: the hubs (tap shows one, the
// cross unpairs), a new hub, notifications, Classic screens, the version
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ShellSettings(shell: WebShell, m: AppModel) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    var unpair by remember { mutableStateOf<String?>(null) }
    val rows = m.screen?.hubs.orEmpty()
    val about = remember { runCatching { ctx.packageManager.getPackageInfo(ctx.packageName, 0).let { "${it.versionName} (${it.longVersionCode})" } }.getOrDefault("?") }
    Scaffold(topBar = {
        TopAppBar(title = { Text("App settings") }, navigationIcon = {
            IconButton(onClick = { shell.over = WebShell.Over.None }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
        })
    }) { pad ->
        LazyColumn(Modifier.fillMaxSize(), contentPadding = pad) {
            item { SectionTitle("Hubs") }
            items(m.links.mapNotNull { Pairing.key(it) }, key = { "hub:$it" }) { k ->
                val row = rows.firstOrNull { it.key == k }
                ListItem(
                    headlineContent = { Text(row?.name ?: k) },
                    supportingContent = { Text(k + if (row?.online == false || !m.linkUp(k)) " · offline" else "") },
                    leadingContent = { if (k == shell.hub) Icon(Icons.Filled.Check, "Shown") },
                    trailingContent = { IconButton(onClick = { unpair = k }) { Icon(Icons.Filled.Close, "Unpair") } },
                    modifier = Modifier.clickable { shell.over = WebShell.Over.None; shell.show(k) },
                )
            }
            item {
                ListItem(headlineContent = { Text("Add a hub") }, leadingContent = { Icon(Icons.Filled.Add, null) },
                    modifier = Modifier.clickable { shell.over = WebShell.Over.Pair })
            }
            item { SectionTitle("Notifications") }
            item {
                val allowed = Notes.allowed(ctx)
                ListItem(headlineContent = { Text("Notifications") },
                    supportingContent = { Text(if (allowed) "On: a thread that needs you or finishes, and agents at work" else "Off") },
                    trailingContent = {
                        OutlinedButton(onClick = {
                            ctx.startActivity(Intent(AndroidSettings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(AndroidSettings.EXTRA_APP_PACKAGE, ctx.packageName))
                        }) { Text("System settings") }
                    })
            }
            item { SectionTitle("Screens") }
            item {
                ListItem(headlineContent = { Text("Classic screens") },
                    supportingContent = { Text("The app's previous native screens instead of the hub's phone page. Kept for one release.") },
                    trailingContent = { Switch(AppSettings.classic, { AppSettings.putClassic(it) }) })
            }
            item { SectionTitle("About") }
            item { ListItem(headlineContent = { Text("Backplane") }, supportingContent = { Text(about) }) }
        }
    }
    unpair?.let { k ->
        AlertDialog(onDismissRequest = { unpair = null },
            title = { Text("Unpair ${rows.firstOrNull { it.key == k }?.name ?: k}?") },
            text = { Text("This phone forgets the hub. Pair again with a new invitation.") },
            confirmButton = { TextButton(onClick = { unpair = null; m.unpair(k) }) { Text("Unpair") } },
            dismissButton = { TextButton(onClick = { unpair = null }) { Text("Cancel") } })
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(text, Modifier.padding(start = 16.dp, top = 20.dp, bottom = 4.dp), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
}
