package app.nisshoku.android.sync

import android.content.Context
import android.util.Log
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import app.nisshoku.android.engine.Nisshoku
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonPrimitive
import java.util.concurrent.TimeUnit

/**
 * Syncs in the background every so often, with the app closed too
 * (WorkManager; the interval is in the settings). The sync runs through the
 * engine the process has open (`sync_unattended`), the app's own if it is
 * running: one sync at a time, and the open screens hear what it changed.
 */
class SyncWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    companion object {
        private const val NAME = "nisshoku-background-sync"

        private val network = Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()

        /** One sync as soon as the network allows (the app left with changes not sent yet). */
        fun runOnce(context: Context) {
            try {
                val request = OneTimeWorkRequestBuilder<SyncWorker>().setConstraints(network).build()
                WorkManager.getInstance(context).enqueueUniqueWork("$NAME-once", ExistingWorkPolicy.REPLACE, request)
            } catch (e: Throwable) {
                Log.w("SyncWorker", "no background sync: $e")
            }
        }

        /** Syncs every `minutes` (15 at least, Android's limit); 0 stops. */
        fun schedule(context: Context, minutes: Int) {
            try {
                val work = WorkManager.getInstance(context)
                if (minutes <= 0) {
                    work.cancelUniqueWork(NAME)
                    return
                }
                val request = PeriodicWorkRequestBuilder<SyncWorker>(maxOf(15, minutes).toLong(), TimeUnit.MINUTES)
                    .setConstraints(network)
                    .build()
                work.enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.UPDATE, request)
            } catch (e: Throwable) {
                Log.w("SyncWorker", "no background sync: $e")
            }
        }
    }

    override suspend fun doWork(): Result = try {
        when (Nisshoku.call("sync_unattended").jsonPrimitive.intOrNull) {
            1 -> Result.retry()
            else -> Result.success()
        }
    } catch (e: Exception) {
        if (e is kotlinx.coroutines.CancellationException) throw e
        Log.w("SyncWorker", "background sync failed: $e")
        Result.retry()
    }
}
