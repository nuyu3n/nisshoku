package app.nisshoku.desktop

import android.app.Application
import androidx.work.Configuration

/**
 * The application: sets WorkManager (the background sync, SyncWorker.kt) up
 * the first time it is asked for, rather than as the process starts
 * (its initializer is taken out in AndroidManifest.xml). Opening the app
 * then never depends on it.
 */
class App : Application(), Configuration.Provider {
  override val workManagerConfiguration: Configuration
    get() = Configuration.Builder().build()
}
