package dev.backplane.mobile

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat

// The phone's alerts. "turns": a turn ended (what to say comes from Bend,
// src/mobile/notify.bend). "live": the ongoing status while agents work.
object Notes {
    const val TURNS = "turns"
    const val LIVE = "live"
    const val EXTRA_THREAD = "dev.backplane.mobile.thread"
    // the live status tapped while several threads work: the app asks which
    const val EXTRA_PICK = "dev.backplane.mobile.pick"

    fun channels(ctx: Context, lang: String = "en") {
        val nm = ctx.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(TURNS, NativeMessages.native_notify_channel_personal(lang), NotificationManager.IMPORTANCE_DEFAULT))
        nm.createNotificationChannel(NotificationChannel(LIVE, NativeMessages.native_notify_channel_live(lang), NotificationManager.IMPORTANCE_LOW).apply {
            setShowBadge(false)
        })
    }

    fun allowed(ctx: Context) =
        Build.VERSION.SDK_INT < 33 ||
            ContextCompat.checkSelfPermission(ctx, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED

    // opens the app, on a thread when one is named
    fun open(ctx: Context, thread: String, code: Int): PendingIntent {
        val i = Intent(ctx, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP)
        if (thread.isNotEmpty()) i.putExtra(EXTRA_THREAD, thread)
        return PendingIntent.getActivity(ctx, code, i, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }


    // one notification per key (src/core/notice.bend: "turn-<thread>", or
    // "room-<room>" for a room exchange, the same on every paired hub): a
    // newer alert replaces the last, and only the first one sounds
    private fun id(key: String) = key.hashCode().let { if (it == LiveService.ID) it + 1 else it }

    private fun key(c: Cmd) = c.key.ifEmpty { "turn-" + c.thread }

    fun turn(ctx: Context, c: Cmd, picture: android.graphics.Bitmap? = null) {
        if (!allowed(ctx)) return
        val n = NotificationCompat.Builder(ctx, TURNS)
            .setSmallIcon(R.drawable.ic_stat)
            .setContentTitle(c.title)
            .setContentText(c.body)
            .setStyle(
                if (picture != null) NotificationCompat.BigPictureStyle().bigPicture(picture).setSummaryText(c.body)
                else NotificationCompat.BigTextStyle().bigText(c.body),
            )
            .setCategory(when (c.kind) {
                "fail" -> NotificationCompat.CATEGORY_ERROR
                "ask", "reminder" -> NotificationCompat.CATEGORY_REMINDER
                "note" -> NotificationCompat.CATEGORY_MESSAGE
                else -> NotificationCompat.CATEGORY_STATUS
            })
            .setContentIntent(open(ctx, c.thread, id(c.thread)))
            .setAutoCancel(true)
            .setOnlyAlertOnce(true)
            .build()
        try {
            NotificationManagerCompat.from(ctx).notify(id(key(c)), n)
        } catch (_: SecurityException) {
        }
    }

    // the thread is open on screen: its alert has been seen
    fun clear(ctx: Context, thread: String) {
        NotificationManagerCompat.from(ctx).cancel(id("turn-" + thread))
    }
}
