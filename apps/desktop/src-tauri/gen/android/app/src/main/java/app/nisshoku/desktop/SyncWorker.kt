package app.nisshoku.desktop

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
import java.util.concurrent.TimeUnit

/**
 * Syncs in the background every so often, with the app closed too
 * (WorkManager; settings → sync). The sync is the app's own, run without a
 * window by `nativeSync` (src/android.rs) on the profile the app gave.
 *
 * WorkManager is set up the first time it is asked for (`App.kt`), not when
 * the process starts, and the app's calls into this class are guarded
 * (MainActivity.kt): a failure here costs the background sync, not the app.
 */
class SyncWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
  companion object {
    private const val NAME = "nisshoku-background-sync"
    private const val PREFS = "background-sync"

    init {
      System.loadLibrary("nisshoku_app_lib")
    }

    /**
     * One sync as soon as the network allows (the app left the screen with
     * changes not sent yet).
     */
    fun runOnce(context: Context) {
      if (context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString("dir", null) == null) return
      val request = OneTimeWorkRequestBuilder<SyncWorker>()
        .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
        .build()
      WorkManager.getInstance(context).enqueueUniqueWork("$NAME-once", ExistingWorkPolicy.REPLACE, request)
    }

    /** Syncs every `minutes` (15 at least, Android's limit); 0 stops. */
    fun schedule(context: Context, minutes: Int, dir: String) {
      context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString("dir", dir).apply()
      val work = WorkManager.getInstance(context)
      if (minutes <= 0) {
        work.cancelUniqueWork(NAME)
        return
      }
      val request = PeriodicWorkRequestBuilder<SyncWorker>(maxOf(15, minutes).toLong(), TimeUnit.MINUTES)
        .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
        .build()
      work.enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.UPDATE, request)
    }
  }

  /** 0 synced, 1 failed, 2 nothing to do (see android.rs). */
  private external fun nativeSync(context: Context, dir: String): Int

  override fun doWork(): Result {
    val dir = applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString("dir", null)
      ?: return Result.success()
    return try {
      when (nativeSync(applicationContext, dir)) {
        1 -> Result.retry()
        else -> Result.success()
      }
    } catch (e: Throwable) {
      Log.w("SyncWorker", "background sync failed: $e")
      Result.failure()
    }
  }
}
