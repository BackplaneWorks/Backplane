package dev.backplane.mobile

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.net.InetAddress
import java.net.ServerSocket
import java.net.URLDecoder
import kotlin.concurrent.thread

// Google's sign-in from the phone (docs/GOOGLE.md "Sign-in", data/1): a listener on a loopback port of the app's own, the hub asked to begin with
// that redirect (account.act `redirect`), Google's page in the browser, and the address the browser came back to sent to the hub as `code`.
// The page it answers sends the browser back to the app (backplane://google, which the app's link handler ignores).
object GoogleLoop {
    // pure parts (GoogleLoopTest)
    fun redirect(port: Int) = "http://127.0.0.1:$port/oauth/google"
    /** Whether `link` is the hub's sign-in link and sends Google back to exactly `redirect`. */
    fun matches(link: String, redirect: String): Boolean {
        val q = link.substringAfter('?', "").substringBefore('#')
        return q.split('&').any { it.startsWith("redirect_uri=") && runCatching { URLDecoder.decode(it.removePrefix("redirect_uri="), "UTF-8") }.getOrNull() == redirect }
    }
    /** The target of a `GET` request line, else null. */
    fun requestPath(line: String): String? {
        val p = line.trim().split(" ").filter { it.isNotEmpty() }
        return if (p.size >= 2 && p[0] == "GET") p[1] else null
    }
    /** The one request that ends a sign-in: /oauth/google with a query. */
    fun isCatch(path: String?): Boolean = path != null && path.startsWith("/oauth/google?") && path.length > "/oauth/google?".length
    /** Whether an account's sign-in is caught here: its view says so, or it is Google's. */
    fun wants(id: String, loopback: Boolean) = loopback || id.startsWith("google:")

    /** Bumped when an address was caught and sent: the open account screen reads its view again. */
    var tick by mutableIntStateOf(0)
    /** The account id the hub answered the caught address with (google:<id>, once signed in; google:new until then). */
    @Volatile var landed = ""
    private var server: ServerSocket? = null
    private var port = 0
    private var opened = false
    /** The account the caught address belongs to (its id from the hub's reply). */
    @Volatile var profile = ""
    private val io = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    /** Opens the listener; its port, or null (then no redirect is sent and the hub's paste path works as it is). */
    @Synchronized
    fun listen(): Int? {
        stop()
        val s = runCatching { ServerSocket(0, 1, InetAddress.getByName("127.0.0.1")) }.getOrNull() ?: return null
        server = s
        port = s.localPort
        val p = port
        thread(isDaemon = true, name = "google-loop") {
            while (true) {
                val c = runCatching { s.accept() }.getOrNull() ?: break
                val path = runCatching { c.soTimeout = 10000; requestPath(c.getInputStream().bufferedReader().readLine() ?: "") }.getOrNull()
                if (!isCatch(path)) {
                    runCatching { c.getOutputStream().write("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".toByteArray()); c.close() }
                    continue
                }
                val back = "${BuildConfig.URL_SCHEME}://google"
                val body = ("<!doctype html><meta name=viewport content=\"width=device-width\"><title>Backplane</title>" +
                    "<p style=\"font:16px sans-serif;margin:24px\">Signed in. <a href=\"$back\">Back to Backplane</a></p>" +
                    "<script>location.href='$back'</script>").toByteArray()
                runCatching {
                    c.getOutputStream().write(("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: ${body.size}\r\n" +
                        "Cache-Control: no-store\r\nConnection: close\r\n\r\n").toByteArray() + body)
                    c.close()
                }
                stopIf(s)
                val who = profile
                io.launch {
                    val r = runCatching { DataClient.call("account.act", JSONObject().put("profile", who).put("provider", "google").put("action", "code").put("code", "http://127.0.0.1:$p$path")) }.getOrNull()
                    landed = r?.optJSONObject("account")?.optString("id") ?: ""
                    tick++
                }
                break
            }
        }
        return p
    }

    /** The hub's sign-in link, opened once and only when it sends Google back to this listener. */
    @Synchronized
    fun open(ctx: Context, link: String) {
        if (server == null || opened || port == 0 || link.isEmpty() || !matches(link, redirect(port))) return
        opened = true
        runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(link)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
    }

    @Synchronized private fun stopIf(s: ServerSocket) { if (server === s) stop() }

    @Synchronized
    fun stop() {
        runCatching { server?.close() }
        server = null; port = 0; opened = false
    }
}
