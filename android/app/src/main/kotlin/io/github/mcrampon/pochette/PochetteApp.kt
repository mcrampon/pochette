package io.github.mcrampon.pochette

import android.app.Application
import android.os.Build
import android.provider.Settings
import io.github.mcrampon.pochette.core.Config
import io.github.mcrampon.pochette.core.Pochette

/** The app: it holds the core, for every screen and service. */
class PochetteApp : Application() {
    lateinit var core: Pochette
        private set

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
                ),
            )
    }

    /** What the user named the phone, else its model: "Pixel 9". */
    private fun deviceName(): String =
        Settings.Global.getString(contentResolver, Settings.Global.DEVICE_NAME)?.takeIf { it.isNotBlank() }
            ?: Build.MODEL
}
