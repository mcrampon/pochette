package io.github.mcrampon.pochette

import android.app.Application
import android.os.Build
import android.provider.Settings
import io.github.mcrampon.pochette.core.Config
import io.github.mcrampon.pochette.core.Pochette
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

/** The app: it holds the core, for every screen and service. */
class PochetteApp : Application() {
    lateinit var core: Pochette
        private set
    lateinit var sync: Sync
        private set

    /** For the work of the whole app, as long as it runs. */
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun onCreate() {
        super.onCreate()
        core =
            Pochette(
                Config(
                    // Backed up with the app's data.
                    dataDir = filesDir.resolve("plays").path,
                    // Never restored to another phone, which would take this one's identity.
                    stateDir = noBackupFilesDir.resolve("sync").path,
                    deviceName = deviceName(),
                    appVersion = packageManager.getPackageInfo(packageName, 0).versionName.orEmpty(),
                ),
            )
        sync = Sync(this, core, scope)
        SyncWorker.schedule(this, Settings(this).syncInBackground)
    }

    /** What the user named the phone, else its model: "Pixel 9". */
    private fun deviceName(): String =
        Settings.Global.getString(contentResolver, Settings.Global.DEVICE_NAME)?.takeIf { it.isNotBlank() }
            ?: Build.MODEL
}
