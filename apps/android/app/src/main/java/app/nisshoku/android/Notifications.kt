package app.nisshoku.android

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.util.Log
import androidx.core.content.ContextCompat

/** The app's notifications: a sync's progress (a Live Update on Android 16) and due tasks. */
object Notifications {
    const val SYNC_CHANNEL = "sync"
    private const val REMINDER_CHANNEL = "reminders"
    const val SYNC_ID = 7301
    private var nextReminder = 8000

    fun allowed(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED

    fun createChannels(context: Context) {
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        manager.createNotificationChannel(
            NotificationChannel(SYNC_CHANNEL, trNow("同期", "Sync"), NotificationManager.IMPORTANCE_LOW).apply {
                setShowBadge(false)
            },
        )
        manager.createNotificationChannel(
            NotificationChannel(REMINDER_CHANNEL, trNow("タスクの期限", "Due tasks"), NotificationManager.IMPORTANCE_HIGH),
        )
    }

    private fun openApp(context: Context): PendingIntent = PendingIntent.getActivity(
        context, 0,
        Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )

    /** A to-do fell due (from the engine's reminders). */
    fun reminder(context: Context, title: String, body: String) {
        if (!allowed(context)) return
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        val n = Notification.Builder(context, REMINDER_CHANNEL)
            .setSmallIcon(R.drawable.shortcut_task_fg)
            .setContentTitle(title)
            .setContentText(body)
            .setContentIntent(openApp(context))
            .setAutoCancel(true)
            .setCategory(Notification.CATEGORY_REMINDER)
            .build()
        manager.notify(nextReminder++, n)
    }

    /** The notification of a running sync: `done` of `total` (0: not known yet). */
    fun syncProgress(context: Context, text: String, done: Int, total: Int): Notification {
        val builder = Notification.Builder(context, SYNC_CHANNEL)
            .setSmallIcon(R.drawable.notification_sync)
            .setContentTitle(trNow("同期しています", "Syncing"))
            .setContentText(text)
            .setContentIntent(openApp(context))
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setShowWhen(false)
            .setCategory(Notification.CATEGORY_PROGRESS)
        if (Build.VERSION.SDK_INT >= 36) {
            // Android 16: a Live Update (the status bar's chip, the lock
            // screen) when the user allows it, with the progress style.
            try {
                val style = Notification.ProgressStyle()
                    .setStyledByProgress(true)
                    .setProgressIndeterminate(total <= 0)
                if (total > 0) {
                    style.setProgressSegments(listOf(Notification.ProgressStyle.Segment(total)))
                    style.setProgress(done.coerceIn(0, total))
                }
                builder.setStyle(style)
                // `setRequestPromotedOngoing` came with 16 QPR1; the extra works on 16.0 too.
                builder.addExtras(Bundle().apply { putBoolean(Notification.EXTRA_REQUEST_PROMOTED_ONGOING, true) })
                builder.setShortCriticalText(if (total > 0) "$done/$total" else trNow("同期", "Sync"))
            } catch (e: Throwable) {
                Log.w("Notifications", "no live update: $e")
                builder.setProgress(total, done.coerceAtMost(total), total <= 0)
            }
        } else {
            builder.setProgress(total, done.coerceAtMost(total), total <= 0)
        }
        return builder.build()
    }
}
