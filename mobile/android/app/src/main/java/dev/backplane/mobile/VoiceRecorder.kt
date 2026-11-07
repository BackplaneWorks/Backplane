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

// The phone's microphone for dictation (docs/PHONE-DATA.md, Dictation): 16 kHz mono s16le in about 200 ms pieces, each POSTed in order to the path
// voice.start answered (with the pairing's bearer token), the last with &end=1; a 404 means the session is over. Recording starts on the press and what
// is heard before the path comes waits for it. Nothing here decides anything about dictation.
class VoiceRecorder(private val ended: (String) -> Unit) {
    companion object {
        const val RATE = 16000
        const val PIECE_BYTES = RATE * 2 / 5 // 200 ms of s16le mono
    }

    private val lock = Object()
    @Volatile var active = false
        private set
    // the thread the session belongs to
    @Volatile var thread = ""
        private set
    private var link = ""
    private var url: String? = null
    private var token: String? = null
    private val held = ArrayList<ByteArray>()
    private var ending = false
    private var dead = false
    private var record: AudioRecord? = null
    private var reader: Thread? = null
    private var started = 0L
    private var tail: ByteArray? = null
    // in order: one POST at a time, each after the one before it
    private val posts = Executors.newSingleThreadExecutor()

    // the press: start recording for a thread's session (false: no microphone); the caller has the RECORD_AUDIO permission
    @SuppressLint("MissingPermission")
    fun start(thread: String, link: String): Boolean { synchronized(lock) {
        if (active) return true
        url = null; token = null; held.clear(); ending = false; dead = false; tail = null
        if (BuildConfig.DEBUG && java.io.File("/data/local/tmp/voice-fake").exists()) {
            // an emulator with a silent microphone (the hub drops pure silence): a 440 Hz tone, so the rest of the path can be driven. Debug builds only
            this.thread = thread; this.link = link; active = true; started = System.currentTimeMillis()
            reader = Thread({ fakeTone() }, "voice-fake").also { it.start() }
            return true
        }
        val min = AudioRecord.getMinBufferSize(RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        if (min <= 0) return false
        val r = try {
            AudioRecord(MediaRecorder.AudioSource.VOICE_RECOGNITION, RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT, maxOf(min, PIECE_BYTES * 2))
        } catch (e: Exception) { return false }
        if (r.state != AudioRecord.STATE_INITIALIZED) { r.release(); return false }
        try { r.startRecording() } catch (e: Exception) { r.release(); return false }
        this.thread = thread
        this.link = link
        record = r
        active = true
        started = System.currentTimeMillis()
        reader = Thread({ read(r) }, "voice-mic").also { it.start() }
        return true
    } }

    private fun fakeTone() {
        var n = 0L
        while (active) {
            val b = ByteArray(PIECE_BYTES)
            for (i in 0 until PIECE_BYTES / 2) {
                val v = (Math.sin(2 * Math.PI * 440.0 * (n++) / RATE) * 8000).toInt()
                b[2 * i] = v.toByte(); b[2 * i + 1] = (v shr 8).toByte()
            }
            synchronized(lock) {
                if (url == null && System.currentTimeMillis() - started > 8000) { stopLocked(); held.clear(); return }
                if (active) take(b, false)
            }
            try { Thread.sleep(200) } catch (e: InterruptedException) { return }
        }
    }

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

    // under lock: a piece out, or held until the address comes
    private fun take(b: ByteArray, last: Boolean) {
        if (dead) return
        if (url == null) { held.add(b); if (last) ending = true; return }
        post(b, last)
    }

    // voice.start's reply: where the audio goes
    fun address(path: String) {
        synchronized(lock) {
            if (!(active || ending) || dead) return
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

    // the hub ended the session (idle, an error, a 404): nothing more to send
    fun gone() {
        synchronized(lock) {
            dead = true
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
        val th = thread
        posts.execute {
            if (dead) return@execute
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
                val code = c.responseCode
                c.disconnect()
                if (code == 404) { gone(); ended(th) }
            } catch (e: Exception) {
            }
        }
    }
}
