package io.github.mcrampon.pochette

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.pm.ServiceInfo
import android.os.SystemClock
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.ForegroundInfo
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkInfo
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import io.github.mcrampon.pochette.core.DownloadProgress
import io.github.mcrampon.pochette.core.PochetteException

/**
 * Downloads an album to listen to offline, on Wi-Fi unless the settings allow mobile data,
 * with a notification saying how it goes. One that stopped picks up where it was.
 */
class DownloadWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val id = inputData.getString(ALBUM) ?: return Result.failure()
        val title = inputData.getString(TITLE).orEmpty()
        val app = applicationContext as PochetteApp
        setForeground(foregroundInfo(title, null))
        // Started by Android, with the app closed: sign in with the kept session.
        if (app.core.account() == null) {
            val kept = SessionStore(app).load() ?: return failure(id, null)
            try {
                app.core.restore(kept)
            } catch (signedOut: PochetteException.SignedOut) {
                return failure(id, signedOut.message)
            } catch (err: PochetteException) {
                return Result.retry()
            }
        }
        val progress =
            object : DownloadProgress {
                private var lastShown = 0L

                override fun progress(fraction: Float) {
                    val now = SystemClock.elapsedRealtime()
                    // Often enough to move, rarely enough not to flood the system.
                    if (now - lastShown < SHOW_EVERY_MS && fraction < 1f) return
                    lastShown = now
                    setProgressAsync(workDataOf(ALBUM to id, PROGRESS to fraction))
                    notifications().notify(NOTIFICATION, notification(title, fraction))
                }
            }
        return try {
            app.core.downloadAlbum(id, Settings(app).downloadQuality, progress)
            Log.i(TAG, "downloaded album $id")
            Result.success(workDataOf(ALBUM to id))
        } catch (err: PochetteException.Network) {
            Log.i(TAG, "can't reach Qobuz to download album $id, trying again later: ${err.message}")
            Result.retry()
        } catch (err: PochetteException) {
            Log.w(TAG, "can't download album $id: ${err.message}")
            failure(id, err.message)
        }
    }

    private fun failure(id: String, error: String?): Result =
        Result.failure(workDataOf(ALBUM to id, TITLE to inputData.getString(TITLE), ERROR to error))

    override suspend fun getForegroundInfo(): ForegroundInfo = foregroundInfo(inputData.getString(TITLE).orEmpty(), null)

    private fun foregroundInfo(title: String, fraction: Float?): ForegroundInfo =
        ForegroundInfo(NOTIFICATION, notification(title, fraction), ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)

    private fun notifications(): NotificationManager = applicationContext.getSystemService(NotificationManager::class.java)

    private fun notification(title: String, fraction: Float?): android.app.Notification {
        val texts = Language.wrap(applicationContext)
        notifications().createNotificationChannel(
            NotificationChannel(CHANNEL, texts.getString(R.string.downloads), NotificationManager.IMPORTANCE_LOW),
        )
        return NotificationCompat
            .Builder(applicationContext, CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(texts.getString(R.string.downloading, title))
            .setProgress(100, ((fraction ?: 0f) * 100).toInt(), fraction == null)
            .setOngoing(true)
            .setSilent(true)
            .addAction(0, texts.getString(R.string.cancel), WorkManager.getInstance(applicationContext).createCancelPendingIntent(getId()))
            .build()
    }

    companion object {
        private const val TAG = "Pochette"

        /** Every download's work has it, to follow them all. */
        const val WORK_TAG = "download"
        const val ALBUM = "album"
        const val TITLE = "title"
        const val PROGRESS = "progress"
        const val ERROR = "error"
        private const val CHANNEL = "downloads"
        private const val NOTIFICATION = 2
        private const val SHOW_EVERY_MS = 500L

        private fun name(albumId: String) = "download/$albumId"

        /** The album a download's work is for. */
        fun albumOf(work: WorkInfo): String? = work.tags.firstOrNull { it.startsWith("album:") }?.removePrefix("album:")

        /** Download album `id`, called `title`, on mobile data too if `mobileData`. Again, it picks up where it was. */
        fun start(context: Context, id: String, title: String, mobileData: Boolean) {
            val request =
                OneTimeWorkRequestBuilder<DownloadWorker>()
                    .setInputData(workDataOf(ALBUM to id, TITLE to title))
                    .setConstraints(
                        Constraints.Builder().setRequiredNetworkType(if (mobileData) NetworkType.CONNECTED else NetworkType.UNMETERED).build(),
                    ).addTag(WORK_TAG)
                    .addTag("album:$id")
                    .build()
            WorkManager.getInstance(context).enqueueUniqueWork(name(id), ExistingWorkPolicy.REPLACE, request)
        }

        fun cancel(context: Context, id: String) {
            WorkManager.getInstance(context).cancelUniqueWork(name(id))
        }

        fun cancelAll(context: Context) {
            WorkManager.getInstance(context).cancelAllWorkByTag(WORK_TAG)
        }
    }
}
