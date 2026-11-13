package app.nisshoku.android.sync

import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import android.util.Log
import app.nisshoku.android.Notifications
import app.nisshoku.android.engine.Nisshoku
import app.nisshoku.android.engine.SyncProgress
import app.nisshoku.android.engine.SyncStatus
import app.nisshoku.android.trNow
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Keeps a sync going when the app leaves the screen, and shows how far it
 * is. Android cuts the network of apps in the background; a foreground
 * service (type `dataSync`) keeps it while the sync runs. The engine reports
 * the sync as it goes (`sync-progress`, `sync-status`); the service ends
 * with it.
 */
class SyncService : Service() {
    companion object {
        /** A sync began (the app is in front, so the service may start). */
        fun start(context: Context) {
            try {
                context.startService(Intent(context, SyncService::class.java))
            } catch (e: Throwable) {
                // Not allowed now: the sync runs without the notification.
                Log.w("SyncService", "could not start: $e")
            }
        }
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private var watching: Job? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        try {
            val first = Notifications.syncProgress(this, trNow("同期しています", "Syncing"), 0, 0)
            startForeground(Notifications.SYNC_ID, first, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        } catch (e: Throwable) {
            Log.w("SyncService", "could not go to the foreground: $e")
            stopSelf()
            return START_NOT_STICKY
        }
        if (watching?.isActive != true) watching = scope.launch { watch() }
        return START_NOT_STICKY
    }

    private suspend fun watch() {
        val manager = getSystemService(NotificationManager::class.java)
        // Gives up when nothing is heard for long (the sync died).
        val stale = scope.launch {
            delay(10 * 60_000L)
            finish()
        }
        Nisshoku.events.collect { event ->
            when (event.name) {
                "sync-progress" -> {
                    val p = runCatching { Nisshoku.json.decodeFromString<SyncProgress>(event.payload) }.getOrNull() ?: return@collect
                    if (p.state == "running") {
                        manager?.notify(Notifications.SYNC_ID, Notifications.syncProgress(this, p.text, p.done, p.total))
                    }
                }
                "sync-status" -> {
                    val s = runCatching { Nisshoku.json.decodeFromString<SyncStatus>(event.payload) }.getOrNull() ?: return@collect
                    if (s.state != "syncing") {
                        stale.cancel()
                        finish()
                    }
                }
            }
        }
    }

    private fun finish() {
        try {
            stopForeground(STOP_FOREGROUND_REMOVE)
        } catch (e: Throwable) {
            Log.w("SyncService", "could not leave the foreground: $e")
        }
        stopSelf()
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }
}
