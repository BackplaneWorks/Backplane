package dev.backplane.mobile

import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.os.SystemClock
import android.system.Os
import android.system.OsConstants
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import okio.ByteString.Companion.toByteString
import org.json.JSONObject
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.Socket
import java.net.SocketAddress
import javax.net.SocketFactory

// A pairing link as the desktop shows it (http://host:3787/#token=abc)
// becomes the hub's socket address (ws://host:3787/ws?token=abc).
object Pairing {
    /** routes: every address to try it at, the link's own first (HubList.routes) */
    data class Invitation(val origin: String, val secret: String, val routes: List<String> = emptyList())
    fun invitation(text: String): Invitation? {
        var raw = text.trim()
        if ((raw.startsWith("backplane://") || raw.startsWith(BuildConfig.URL_SCHEME + "://"))) raw = Uri.parse(raw).getQueryParameter("url") ?: return null
        val u = Uri.parse(raw)
        if (u.scheme !in setOf("http", "https") || u.host.isNullOrEmpty() || u.userInfo != null) return null
        val f = Uri.parse("https://pair/?" + (u.encodedFragment ?: ""))
        val secret = f.getQueryParameter("invite") ?: f.getQueryParameter("code") ?: return null
        if (!Regex("(?:[a-f0-9]{32}|[A-Z2-9]{8})").matches(secret)) return null
        val origin = u.buildUpon().path("").clearQuery().fragment(null).build().toString().trimEnd('/')
        return Invitation(origin, secret, (listOf(origin) + HubList.routes(raw)).distinct())
    }
    fun malformedInvitation(text: String): Boolean {
        var raw = text.trim()
        if ((raw.startsWith("backplane://") || raw.startsWith(BuildConfig.URL_SCHEME + "://"))) raw = Uri.parse(raw).getQueryParameter("url") ?: return false
        val fields = Uri.parse("https://pair/?" + (Uri.parse(raw).encodedFragment ?: ""))
        return fields.queryParameterNames.any { it == "invite" || it == "code" }
    }
    /** A short code on its own: it names no hub, and must not be taken for an address. */
    fun bareCode(text: String): Boolean = Regex("[A-Z2-9]{4}[- ]?[A-Z2-9]{4}").matches(text.trim())
    fun codeLink(address: String, code: String): String? {
        val compact = code.replace(" ", "").replace("-", "").uppercase()
        if (!Regex("[A-Z2-9]{8}").matches(compact)) return null
        val u = Uri.parse(if (address.contains("://")) address else "http://$address")
        if (u.scheme !in setOf("http", "https") || u.host.isNullOrEmpty() || u.userInfo != null) return null
        // (encodedFragment: fragment() would encode the "=", and invitation() would find no code)
        return u.buildUpon().path("/pair").clearQuery().encodedFragment("code=$compact").build().toString()
    }
    fun message(error: String): String = when(error) {
        "wrong_code" -> "That code is not correct. Check it and try again."
        "expired_code" -> "This invitation expired. Create another in Settings → Devices."
        "used_code" -> "This invitation has already been used. Create a new one."
        "revoked_code" -> "This invitation was revoked. Create a new one in Settings → Devices."
        "rate_limited" -> "Too many attempts. Wait a minute and try again."
        "pairing_required" -> "Re-pair this device with a new invitation."
        else -> "Hub unreachable. Check its address, LAN or tailnet connection."
    }
    /** The invitation tried at each address in turn: one that cannot be reached gives way to the next; the first hub that answers decides
     *  (a refusal is final: the next address is the same hub, and every guess counts against its limit). The link kept names the address that answered. */
    suspend fun exchange(invite: Invitation, at: List<String> = listOf(invite.origin)): String {
        var last: Exception = IllegalArgumentException(message("hub_unreachable"))
        for (origin in at.ifEmpty { listOf(invite.origin) }) {
            try { return exchangeAt(invite, origin) } catch (e: java.io.IOException) { last = IllegalArgumentException(message("hub_unreachable")) }
        }
        throw last
    }
    private suspend fun exchangeAt(invite: Invitation, origin: String): String = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
        val r = okhttp3.Request.Builder().url(origin + "/auth/enroll")
            .post(okhttp3.RequestBody.create(null, ByteArray(0))).header("Authorization", "Bearer " + invite.secret)
            .header("X-Backplane-Client", "native").header("X-Backplane-Name", android.os.Build.MODEL.filter { it.code in 32..126 }).build()
        val client = okhttp3.OkHttpClient.Builder().followRedirects(false).followSslRedirects(false).callTimeout(8, java.util.concurrent.TimeUnit.SECONDS).build()
        client.newCall(r).execute().use { response ->
            val json = org.json.JSONObject(response.body?.string() ?: "{}")
            val token = json.optString("token")
            if (!response.isSuccessful || !Regex("[a-f0-9]{32}").matches(token)) throw IllegalArgumentException(message(json.optString("error")))
            origin + "/#token=" + token
        }
    }
    fun socket(link: String): String? {
        var s = link.trim()
        if (s.isEmpty()) return null
        if ((s.startsWith("backplane://") || s.startsWith(BuildConfig.URL_SCHEME + "://"))) s = Uri.parse(s).getQueryParameter("url") ?: return null
        if (!s.contains("://")) s = "http://$s"
        val u = Uri.parse(s)
        val host = u.host ?: return null
        val token = Regex("token=([0-9a-f]+)").find((u.fragment ?: "") + "&" + (u.query ?: ""))?.groupValues?.get(1)
        val scheme = if (u.scheme == "https") "wss" else "ws"
        val port = if (u.port > 0) ":${u.port}" else ""
        return "$scheme://$host$port/ws" + (token?.let { "?token=$it" } ?: "")
    }

    fun viewSocket(link: String): String? = socket(link)?.let {
        Uri.parse(it).buildUpon().appendQueryParameter("since", "0").appendQueryParameter("origin", "")
            .appendQueryParameter("enc", "cbor").appendQueryParameter("view", "1").appendQueryParameter("mobile", "1").build().toString()
    }

    // an HTTP address on the hub (its token along, unless token is false)
    fun http(link: String, path: String, query: String = "", token: Boolean = true): String? {
        val w = Uri.parse(socket(link) ?: return null)
        val scheme = if (w.scheme == "wss") "https" else "http"
        val port = if (w.port > 0) ":${w.port}" else ""
        val q = listOfNotNull(if (token) w.getQueryParameter("token")?.let { "token=$it" } else null, query.ifEmpty { null })
        return "$scheme://${w.host}$port$path" + (if (q.isEmpty()) "" else "?" + q.joinToString("&"))
    }

    fun token(link: String): String? = socket(link)?.let { Uri.parse(it).getQueryParameter("token") }

    // the hub's key: host:port, the same for every link to it
    fun key(link: String): String? {
        val u = Uri.parse(socket(link) ?: return null)
        val host = u.host ?: return null
        return if (u.port > 0) "$host:${u.port}" else host
    }
}

// A socket that tells a dead link by TCP keepalive: probed after 15 s with
// nothing heard, every 10 s, given up after 4 unanswered (about a minute).
// The kernels answer the probes beside the data, so a 3D model of many MB
// coming over a slow link is never taken for a dead one. A WebSocket ping
// did that: its pong waits behind the model, OkHttp dropped the link when
// it came late, and the phone asked again, never to see the parts.
private class LiveSocket : Socket() {
    override fun connect(endpoint: SocketAddress?, timeout: Int) {
        super.connect(endpoint, timeout)
        keepAlive = true
        // TCP_KEEPIDLE, TCP_KEEPINTVL, TCP_KEEPCNT (Linux's; OsConstants lacks them)
        runCatching {
            ParcelFileDescriptor.fromSocket(this).use { p ->
                Os.setsockoptInt(p.fileDescriptor, OsConstants.IPPROTO_TCP, 4, 15)
                Os.setsockoptInt(p.fileDescriptor, OsConstants.IPPROTO_TCP, 5, 10)
                Os.setsockoptInt(p.fileDescriptor, OsConstants.IPPROTO_TCP, 6, 4)
            }
        }
    }
}

private object LiveSockets : SocketFactory() {
    override fun createSocket(): Socket = LiveSocket()
    override fun createSocket(host: String, port: Int): Socket = LiveSocket().apply { connect(InetSocketAddress(host, port)) }
    override fun createSocket(host: String, port: Int, local: InetAddress, localPort: Int): Socket =
        LiveSocket().apply { bind(InetSocketAddress(local, localPort)); connect(InetSocketAddress(host, port)) }
    override fun createSocket(host: InetAddress, port: Int): Socket = LiveSocket().apply { connect(InetSocketAddress(host, port)) }
    override fun createSocket(host: InetAddress, port: Int, local: InetAddress, localPort: Int): Socket =
        LiveSocket().apply { bind(InetSocketAddress(local, localPort)); connect(InetSocketAddress(host, port)) }
}

// Alive.limit in core/desk, in ms
private const val ALIVE_LIMIT_MS = 40_000L

// What to do on each (re)connect: open this address, try again after the
// backoff (the hub is down or restarting), or stop.
sealed class Next {
    data class Go(val url: String) : Next()
    object Later : Next()
    object Stop : Next()
}

// The socket to the hub, reconnecting with backoff like host.js. The
// address is asked for afresh on every attempt, so each reconnect resumes
// from what the client already holds.
class Hub(
    private val scope: CoroutineScope,
    private val url: suspend () -> Next,
    private val onOpen: () -> Unit,
    private val onMessage: (ByteArray) -> Unit,
    private val onClose: () -> Unit,
) {
    private val client = OkHttpClient.Builder().socketFactory(LiveSockets).build()
    private val main = Handler(Looper.getMainLooper())
    private var socket: WebSocket? = null
    private var backoff = 250L
    private var stopped = false
    // the attempt waiting out its backoff, which wake() runs at once
    private var retry: Runnable? = null
    // whether this socket has said anything yet: the backoff starts over
    // only then, so a hub that accepts and drops is not hammered
    private var spoke = false

    // When a frame (a heartbeat included) last arrived. The hub sends one
    // every 15 s; a link silent for more
    // than Alive.limit (40 s) is dead, whatever TCP says, and is dropped
    // and reconnected from the client's sequence.
    private var heard = SystemClock.elapsedRealtime()
    private val watch = object : Runnable {
        override fun run() {
            if (stopped) return
            val ws = socket
            if (ws != null && SystemClock.elapsedRealtime() - heard > ALIVE_LIMIT_MS) {
                Log.w("Backplane", "hub silent for ${ALIVE_LIMIT_MS / 1000} s: reconnecting")
                ws.cancel() // onFailure follows and reconnects
            }
            main.postDelayed(this, 5000)
        }
    }

    fun start() {
        stopped = false
        heard = SystemClock.elapsedRealtime()
        main.removeCallbacks(watch)
        main.postDelayed(watch, 5000)
        connect()
    }

    // Try now rather than after the backoff: the network came back or the
    // app is in front again. A socket quiet for quietMs (it may have died
    // while the app was away) is replaced.
    fun wake(quietMs: Long = 3000) {
        if (stopped) return
        backoff = 250
        val ws = socket
        if (ws != null) {
            if (SystemClock.elapsedRealtime() - heard > quietMs) ws.cancel() // onFailure follows and reconnects
            return
        }
        retry?.let {
            main.removeCallbacks(it)
            retry = null
            connect()
        }
    }

    fun stop() {
        stopped = true
        retry?.let { main.removeCallbacks(it) }
        retry = null
        main.removeCallbacks(watch)
        socket?.close(1000, null)
        socket = null
    }

    fun send(bytes: ByteArray) {
        socket?.send(bytes.toByteString())
    }

    private fun connect() {
        if (stopped) return
        retry?.let { main.removeCallbacks(it) }
        retry = null
        scope.launch {
            when (val next = url()) {
                is Next.Go -> if (!stopped) open(next.url)
                Next.Later -> main.post { if (!stopped && socket == null) { onClose(); later() } }
                Next.Stop -> {}
            }
        }
    }

    // the next attempt after the backoff, with equal jitter: after a
    // deploy the phones spread out, none at zero
    private fun later() {
        val wait = backoff / 2 + (Math.random() * (backoff / 2)).toLong()
        backoff = minOf(backoff * 2, 3000)
        val r = Runnable { retry = null; connect() }
        retry?.let { main.removeCallbacks(it) }
        retry = r
        main.postDelayed(r, wait)
    }

    private fun open(u: String) {
        client.newWebSocket(Request.Builder().url(u).build(), object : WebSocketListener() {
            override fun onOpen(ws: WebSocket, response: Response) {
                main.post {
                    if (stopped) {
                        ws.close(1000, null)
                    } else {
                        socket = ws
                        heard = SystemClock.elapsedRealtime()
                        spoke = false
                        onOpen()
                    }
                }
            }

            // every frame is binary CBOR, both ways
            override fun onMessage(ws: WebSocket, bytes: ByteString) {
                val b = bytes.toByteArray()
                main.post {
                    if (!stopped && socket === ws) {
                        heard = SystemClock.elapsedRealtime()
                        if (!spoke) { spoke = true; backoff = 250 }
                        onMessage(b)
                    }
                }
            }

            override fun onClosed(ws: WebSocket, code: Int, reason: String) = lost(ws)
            override fun onFailure(ws: WebSocket, t: Throwable, response: Response?) {
                Log.w("Backplane", "hub socket lost: $t")
                lost(ws)
            }
        })
    }

    private fun lost(ws: WebSocket) {
        main.post {
            if (stopped) {
                if (socket === ws) socket = null
            } else if (socket === ws || socket == null) {
                socket = null
                onClose()
                later()
            }
        }
    }
}
