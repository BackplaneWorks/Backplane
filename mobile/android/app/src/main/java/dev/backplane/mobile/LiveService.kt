package dev.backplane.mobile

import android.app.Notification
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Bundle
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat

// The live status, Android's Dynamic Island: a foreground service that
// runs while the island says agents are working. It keeps the process, and
// so the socket, alive when the app leaves the screen, and shows what the
// agents are doing as an ongoing notification (a status-bar chip where
// Android 16 promotes it). What it says is .
class LiveService : Service() {
    companion object {
        const val ID = 0x6c697665

        // the island last seen, the running service, and whether one was
        // asked for and has not started yet. Main thread only.
        private var island = Island(0, "", emptyList())
        private var instance: LiveService? = null
        private var pending = false

        // Called with every screen. Starts the service when agents begin
        // working (only while the app is in front: Android 12+ refuses a
        // foreground service started from the background), updates it while
        // they work, and ends it when they stop.
        fun sync(ctx: Context, next: Island, foreground: Boolean) {
            island = next
            val live = instance
            when {
                live != null -> if (next.running > 0) live.show() else live.finish()
                next.running > 0 && foreground && !pending -> try {
                    pending = true
                    ContextCompat.startForegroundService(ctx, Intent(ctx, LiveService::class.java))
                } catch (_: IllegalStateException) {
                    // ForegroundServiceStartNotAllowedException: try again when in front
                    pending = false
                }
            }
        }

        private fun text(i: Island): String =
            if (i.lines.size == 1) i.lines[0].doing
            else i.lines.joinToString("\n") { it.title + ": " + it.doing }
    }

    private var shown: Pair<String, String>? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        instance = this
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        pending = false
        // a service started in the foreground must say so, even when the
        // agents stopped before it got here
        try {
            ServiceCompat.startForeground(this, ID, notification(island),
                if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC else 0)
            shown = island.headline to text(island)
        } catch (_: RuntimeException) {
            stopSelf()
            return START_NOT_STICKY
        }
        if (island.running == 0) finish()
        return START_NOT_STICKY
    }

    private fun show() {
        val now = island.headline to text(island)
        if (now == shown) return
        shown = now
        try {
            NotificationManagerCompat.from(this).notify(ID, notification(island))
        } catch (_: SecurityException) {
        }
    }

    private fun finish() {
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
        if (instance === this) instance = null
    }

    // Android 15 limits dataSync services to six hours a day
    override fun onTimeout(startId: Int, fgsType: Int) = finish()

    override fun onDestroy() {
        if (instance === this) instance = null
        super.onDestroy()
    }

    private fun notification(i: Island): Notification {
        val body = text(i)
        val b = NotificationCompat.Builder(this, Notes.LIVE)
            .setSmallIcon(R.drawable.ic_stat)
            .setContentTitle(i.headline)
            .setContentText(body.substringBefore('\n'))
            .setStyle(NotificationCompat.BigTextStyle().bigText(body))
            .setProgress(0, 0, true)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            // one thread opens it; several, the app asks which
            .setContentIntent(i.lines.singleOrNull()?.let { Notes.open(this, it.thread, ID) } ?: Notes.open(this, "", ID))
            // Android 16: ask to be promoted to a status-bar chip
            // (Notification.EXTRA_REQUEST_PROMOTED_ONGOING; compileSdk 35 lacks it)
            .addExtras(Bundle().apply { putBoolean("android.requestPromotedOngoing", true) })
        // expanded, each thread (up to three) is a button that opens it
        if (i.lines.size > 1) i.lines.take(3).forEachIndexed { k, l ->
            b.addAction(0, l.title.ifEmpty { l.doing }, Notes.open(this, l.thread, ID + 1 + k))
        }
        return b.build()
    }
}
