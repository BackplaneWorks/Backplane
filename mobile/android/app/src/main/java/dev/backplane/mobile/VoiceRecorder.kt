package dev.backplane.mobile

import android.annotation.SuppressLint
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.net.Uri
import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Executors

// Dictation from this phone's microphone (docs/voice.md): the hub cannot
// hear the person, so the phone records 16 kHz mono s16le and uploads it,
// in order, in ~200 ms pieces to the address the hub's start reply named
// (/voice/in?s=<key>, the bridge's "voice-audio" command), with the pairing
// token; the last POST carries &end=1. Recording starts on the press and
// what is heard before the address comes waits for it. Nothing here decides
// anything about dictation (src/web/voice-mic.js is the web's twin):
// asks the hub, the hub answers, and its strip is drawn from the screen.
class VoiceRecorder {
    companion object {
        const val RATE = 16000
        const val PIECE_BYTES = RATE * 2 / 5 // 200 ms of s16le mono
    }

    private val lock = Object()
    @Volatile var active = false
        private set
    // the hub (host:port) the session belongs to
    var hub = ""
        private set
    private var link = ""
    private var url: String? = null
    private var token: String? = null
    private val held = ArrayList<ByteArray>()
    private var ending = false
    private var record: AudioRecord? = null
    private var reader: Thread? = null
    private var started = 0L
    // in order: one POST at a time, each after the one before it
    private val posts = Executors.newSingleThreadExecutor()

    // the press: start recording into hub's session (false: no microphone);
    // the caller has the RECORD_AUDIO permission
    @SuppressLint("MissingPermission")
    fun start(hub: String, link: String): Boolean { synchronized(lock) {
        if (active) return true
        url = null; token = null; held.clear(); ending = false
        val min = AudioRecord.getMinBufferSize(RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        if (min <= 0) return false
        val r = try {
            AudioRecord(MediaRecorder.AudioSource.VOICE_RECOGNITION, RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT, maxOf(min, PIECE_BYTES * 2))
        } catch (e: Exception) { return false }
        if (r.state != AudioRecord.STATE_INITIALIZED) { r.release(); return false }
        try { r.startRecording() } catch (e: Exception) { r.release(); return false }
        this.hub = hub
        this.link = link
        record = r
        active = true
        started = System.currentTimeMillis()
        reader = Thread({ read(r) }, "voice-mic").also { it.start() }
        return true
    } }

    private fun read(r: AudioRecord) {
        val chunk = ByteArray(PIECE_BYTES)
        val piece = ByteArrayOutputStream()
        while (active && record === r) {
            val n = r.read(chunk, 0, chunk.size)
            if (n <= 0) { if (n < 0) break else continue }
            piece.write(chunk, 0, n)
            if (piece.size() >= PIECE_BYTES) {
                val b = piece.toByteArray(); piece.reset()
                synchronized(lock) {
                    // a start the hub never answered (an older hub, a refused request)
                    if (url == null && System.currentTimeMillis() - started > 8000) { stopLocked(); held.clear(); return }
                    if (active) take(b, false)
                }
            }
        }
        // what was read before the stop goes with the end mark (stop())
        synchronized(lock) { if (piece.size() > 0) tail = piece.toByteArray() }
    }

    private var tail: ByteArray? = null

    // under lock: a piece out, or held until the address comes
    private fun take(b: ByteArray, last: Boolean) {
        if (url == null) { held.add(b); if (last) ending = true; return }
        post(b, last)
    }

    // the hub's start reply: where the audio goes
    fun address(hub: String, path: String) {
        synchronized(lock) {
            if (hub != this.hub || !(active || ending)) return
            val p = Uri.parse(path)
            url = Pairing.http(link, p.path ?: return, p.encodedQuery ?: "", token = false)
            token = Uri.parse(Pairing.socket(link) ?: return).getQueryParameter("token")
            for (b in held) post(b, false)
            held.clear()
            if (ending) { ending = false; post(ByteArray(0), true) }
        }
    }

    // the person stopped: what is left goes, with the end mark
    fun stop() {
        val t: Thread?
        synchronized(lock) {
            if (!active) return
            t = reader
            active = false
        }
        try { t?.join(500) } catch (e: InterruptedException) {}
        synchronized(lock) {
            val rest = tail ?: ByteArray(0)
            tail = null
            release()
            take(rest, true)
        }
    }

    // the hub ended the session (idle, an error): nothing more to send
    fun gone(hub: String) {
        synchronized(lock) {
            if (hub != this.hub) return
            stopLocked()
            held.clear()
            ending = false
        }
    }

    private fun stopLocked() {
        active = false
        release()
    }

    private fun release() {
        val r = record ?: return
        record = null
        try { r.stop() } catch (e: Exception) {}
        r.release()
    }

    private fun post(body: ByteArray, last: Boolean) {
        val base = url ?: return
        val to = base + (if (last) "&end=1" else "")
        val bearer = token
        posts.execute {
            try {
                val c = URL(to).openConnection() as HttpURLConnection
                c.requestMethod = "POST"
                c.doOutput = true
                c.connectTimeout = 10_000
                c.readTimeout = 30_000
                c.setFixedLengthStreamingMode(body.size)
                c.setRequestProperty("Content-Type", "application/octet-stream")
                bearer?.let { c.setRequestProperty("Authorization", "Bearer $it") }
                c.outputStream.use { it.write(body) }
                c.responseCode
                c.disconnect()
            } catch (e: Exception) {
            }
        }
    }
}
