package dev.backplane.mobile

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat

/**
 * While threads work, as 0.14's app did (its LiveService): a foreground service that keeps the process, and so the hub's socket, alive when the app leaves the screen,
 * so the alert of a thread that finishes or asks reaches the phone (data/1 `notify`); Android freezes a backgrounded app within seconds otherwise. Its ongoing
 * notification says what works. It starts only while the app is in front (Android 12+ refuses one started from the background) and ends when nothing works.
 * Main thread only.
 */
class LiveService : Service() {
    companion object {
        private const val ID = 0x6c697665
        private const val CHANNEL = "live"
        private var working: List<String> = emptyList()
        private var instance: LiveService? = null
        private var pending = false

        /** `titles`: the threads working now; `front`: the app is on screen. */
        fun sync(ctx: Context, titles: List<String>, front: Boolean) {
            working = titles
            val live = instance
            when {
                live != null -> if (titles.isNotEmpty()) live.show() else live.finish()
                titles.isNotEmpty() && front && !pending -> try {
                    pending = true
                    ContextCompat.startForegroundService(ctx, Intent(ctx, LiveService::class.java))
                } catch (_: IllegalStateException) { pending = false } // not allowed now: again when in front
            }
        }
        /** What the ongoing notification says: one thread's title, or how many work and their titles. */
        fun text(titles: List<String>): Pair<String, String> =
            if (titles.size == 1) "Working" to titles[0].ifEmpty { "New thread" } else "${titles.size} threads working" to titles.joinToString(" · ") { it.ifEmpty { "New thread" } }
    }

    override fun onBind(intent: Intent?): IBinder? = null
    override fun onCreate() { super.onCreate(); instance = this }
    override fun onDestroy() { if (instance === this) instance = null; pending = false; super.onDestroy() }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        pending = false
        try {
            ServiceCompat.startForeground(this, ID, notification(), if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC else 0)
        } catch (_: RuntimeException) { stopSelf(); return START_NOT_STICKY }
        if (working.isEmpty()) finish()
        return START_NOT_STICKY
    }
    // Android 15 ends a data-sync service after its daily time: stop as asked
    override fun onTimeout(startId: Int, fgsType: Int) { finish() }

    private fun show() { (getSystemService(NOTIFICATION_SERVICE) as NotificationManager).notify(ID, notification()) }
    private fun finish() { ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE); stopSelf() }

    private fun notification(): Notification {
        val nm = getSystemService(NOTIFICATION_SERVICE) as NotificationManager
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "Threads working", NotificationManager.IMPORTANCE_LOW))
        val open = PendingIntent.getActivity(this, ID, Intent(this, DataActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val (title, body) = text(working)
        return Notification.Builder(this, CHANNEL).setSmallIcon(android.R.drawable.stat_notify_sync).setContentTitle(title).setContentText(body)
            .setOngoing(true).setOnlyAlertOnce(true).setContentIntent(open).build()
    }
}
