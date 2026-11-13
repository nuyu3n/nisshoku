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
import androidx.work.Worker
import androidx.work.WorkerParameters
import app.nisshoku.android.engine.NativeInit
import app.nisshoku.android.engine.Nisshoku
import app.nisshoku.android.engine.backgroundSync
import java.util.concurrent.TimeUnit

/**
 * Syncs in the background every so often, with the app closed too
 * (WorkManager; the interval is in the settings). The sync is the engine's,
 * run on its own over the profile (`background_sync`).
 */
class SyncWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
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

    override fun doWork(): Result = try {
        NativeInit.init(applicationContext)
        when (backgroundSync(Nisshoku.profileDir(applicationContext).path)) {
            1 -> Result.retry()
            else -> Result.success()
        }
    } catch (e: Throwable) {
        Log.w("SyncWorker", "background sync failed: $e")
        Result.failure()
    }
}
