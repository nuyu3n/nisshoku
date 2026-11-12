package app.nisshoku.desktop

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import org.json.JSONObject
import java.io.File

/**
 * Keeps a sync going when the app leaves the screen, and shows how far it is.
 *
 * Android cuts the network of apps in the background; a foreground service
 * (type `dataSync`) keeps it while the sync runs. The sync itself is the
 * app's (Rust): it writes its progress to `sync-progress.json` in the
 * profile, which this service reads a few times a second for its
 * notification, and stops when the file says the sync is done. On Android 16
 * the notification asks to be shown as a Live Update.
 */
class SyncService : Service() {
  companion object {
    private const val CHANNEL = "sync"
    private const val NOTIFICATION = 7301
    /** Gives up on a progress file that stops changing (the sync died). */
    private const val STALE_MS = 120_000L

    /** Starts the service for a sync of the profile in `dir` (the app is in front). */
    fun start(context: Context, dir: String) {
      val intent = Intent(context, SyncService::class.java).putExtra("dir", dir)
      try {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) context.startForegroundService(intent)
        else context.startService(intent)
      } catch (e: Exception) {
        // Not allowed now (for example already in the background): the sync
        // runs without the notification.
        Log.w("SyncService", "could not start: $e")
      }
    }
  }

  private val handler = Handler(Looper.getMainLooper())
  private var file: File? = null
  private var startedAt = 0L

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    val dir = intent?.getStringExtra("dir")
    if (dir == null) {
      stopSelf()
      return START_NOT_STICKY
    }
    file = File(dir, "sync-progress.json")
    startedAt = System.currentTimeMillis()
    createChannel()
    val first = build(getString(R.string.sync_running), 0, 0)
    try {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
        startForeground(NOTIFICATION, first, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
      } else {
        startForeground(NOTIFICATION, first)
      }
    } catch (e: Exception) {
      Log.w("SyncService", "could not go to the foreground: $e")
      stopSelf()
      return START_NOT_STICKY
    }
    handler.removeCallbacksAndMessages(null)
    handler.postDelayed(::poll, 400)
    return START_NOT_STICKY
  }

  private fun poll() {
    val now = System.currentTimeMillis()
    val json = try {
      file?.takeIf { it.exists() }?.readText()?.let { JSONObject(it) }
    } catch (e: Exception) {
      null
    }
    val time = json?.optLong("time") ?: 0L
    // Done: this sync's end, or an older end with nothing new for a moment
    // (a short sync may be over before the service starts).
    val done = json?.optString("state") == "done" && (time >= startedAt - 1000 || now - startedAt > 3000)
    val stale = now - maxOf(time, startedAt) > STALE_MS
    if (done || stale) {
      Log.i("SyncService", if (done) "sync done" else "progress stopped changing")
      finish()
      return
    }
    if (json != null && time >= startedAt - 1000) {
      val manager = getSystemService(NotificationManager::class.java)
      manager?.notify(NOTIFICATION, build(json.optString("text", getString(R.string.sync_running)), json.optInt("done"), json.optInt("total")))
    }
    handler.postDelayed(::poll, 400)
  }

  private fun finish() {
    handler.removeCallbacksAndMessages(null)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) stopForeground(STOP_FOREGROUND_REMOVE)
    else @Suppress("DEPRECATION") stopForeground(true)
    stopSelf()
  }

  override fun onDestroy() {
    handler.removeCallbacksAndMessages(null)
    super.onDestroy()
  }

  private fun createChannel() {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
    val manager = getSystemService(NotificationManager::class.java) ?: return
    if (manager.getNotificationChannel(CHANNEL) != null) return
    val channel = NotificationChannel(CHANNEL, getString(R.string.sync_channel), NotificationManager.IMPORTANCE_LOW)
    channel.setShowBadge(false)
    manager.createNotificationChannel(channel)
  }

  private fun build(text: String, done: Int, total: Int): Notification {
    val open = PendingIntent.getActivity(
      this, 0,
      Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
      PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )
    @Suppress("DEPRECATION")
    val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) Notification.Builder(this, CHANNEL) else Notification.Builder(this)
    builder
      .setSmallIcon(R.drawable.notification_sync)
      .setContentTitle(getString(R.string.sync_running))
      .setContentText(text)
      .setContentIntent(open)
      .setOngoing(true)
      .setOnlyAlertOnce(true)
      .setShowWhen(false)
      .setCategory(Notification.CATEGORY_PROGRESS)
      .setProgress(total, done.coerceAtMost(total), total <= 0)
    if (Build.VERSION.SDK_INT >= 36) {
      // Android 16: shown as a Live Update (in the status bar chip and on
      // the lock screen) when the user allows it.
      builder.setRequestPromotedOngoing(true)
      builder.setShortCriticalText(if (total > 0) "$done/$total" else getString(R.string.sync_short))
    }
    return builder.build()
  }
}
