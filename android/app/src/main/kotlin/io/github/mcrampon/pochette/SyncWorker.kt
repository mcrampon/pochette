package io.github.mcrampon.pochette

import android.content.Context
import android.util.Log
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import io.github.mcrampon.pochette.core.PochetteException
import java.util.concurrent.TimeUnit

/** Syncs the plays with the paired devices while the app is closed, as often as Android lets it, on Wi-Fi. */
class SyncWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val app = applicationContext as PochetteApp
        if (app.core.syncStatus().peers.isEmpty()) return Result.success()
        app.sync.hold()
        try {
            val synced = app.core.syncOnce(TIMEOUT_SECONDS)
            Log.i(TAG, if (synced) "synced in the background" else "some devices didn't sync in the background")
        } catch (err: PochetteException) {
            Log.w(TAG, "can't sync in the background: ${err.message}")
        } finally {
            app.sync.release()
        }
        // Next time, whatever happened: the devices may be off.
        return Result.success()
    }

    companion object {
        private const val TAG = "Pochette"
        private const val NAME = "sync"

        /** How long a sync may take: Android gives a job ten minutes. */
        private const val TIMEOUT_SECONDS = 60u

        /** Sync in the background every 15 minutes or so, Android's shortest, or no longer. */
        fun schedule(context: Context, on: Boolean) {
            val work = WorkManager.getInstance(context)
            if (!on) {
                work.cancelUniqueWork(NAME)
                return
            }
            val request =
                PeriodicWorkRequestBuilder<SyncWorker>(15, TimeUnit.MINUTES)
                    // The other devices are on the same network: Wi-Fi, or Ethernet.
                    .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.UNMETERED).build())
                    .build()
            work.enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.KEEP, request)
        }
    }
}
