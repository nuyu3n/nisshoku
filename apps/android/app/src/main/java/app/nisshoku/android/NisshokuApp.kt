package app.nisshoku.android

import android.app.Application
import androidx.work.Configuration
import app.nisshoku.android.engine.Nisshoku

/**
 * The application: hands the engine its context, and sets WorkManager (the
 * background sync) up the first time it is asked for, not as the process
 * starts (its initializer is taken out in the manifest).
 */
class NisshokuApp : Application(), Configuration.Provider {
    override fun onCreate() {
        super.onCreate()
        Nisshoku.setUp(this)
        Notifications.createChannels(this)
    }

    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder().build()
}
