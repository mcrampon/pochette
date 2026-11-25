package io.github.mcrampon.pochette.playback

import android.app.PendingIntent
import android.content.Intent
import android.content.SharedPreferences
import android.os.Bundle
import android.os.SystemClock
import android.util.Log
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.ResolvingDataSource
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import io.github.mcrampon.pochette.MainActivity
import io.github.mcrampon.pochette.PochetteApp
import io.github.mcrampon.pochette.Settings

/** Try a broken stream again at most this often, so a track that can't play doesn't loop. */
private const val RECOVER_EVERY_MS = 30_000L

/**
 * Plays the queue, also with the app closed: the notification, the lock screen, headphones
 * and cars control it through its session.
 */
@UnstableApi
class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null
    private lateinit var heard: Heard

    /** Held here: preferences only keep a weak reference to their listeners. */
    private lateinit var qualityChanged: SharedPreferences.OnSharedPreferenceChangeListener

    override fun onCreate() {
        super.onCreate()
        val core = (application as PochetteApp).core
        val settings = Settings(this)
        val streams = Streams(core) { settings.quality }
        // The next tracks stream in the new quality, not in the one they were fetched in.
        qualityChanged = settings.onQualityChanged(streams::forgetAll)
        val player =
            ExoPlayer
                // Hi-res FLAC decoded to floats, not cut down to 16 bits.
                .Builder(this, DefaultRenderersFactory(this).setEnableAudioFloatOutput(true))
                .setMediaSourceFactory(
                    DefaultMediaSourceFactory(ResolvingDataSource.Factory(DefaultHttpDataSource.Factory(), streams)),
                ).setAudioAttributes(
                    AudioAttributes
                        .Builder()
                        .setUsage(C.USAGE_MEDIA)
                        .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                        .build(),
                    // Pause for calls and other apps' sound.
                    true,
                )
                // Pause when the headphones are unplugged.
                .setHandleAudioBecomingNoisy(true)
                .setWakeMode(C.WAKE_MODE_NETWORK)
                .build()
        player.addListener(Recovery(player, streams))
        heard =
            Heard(core, player, streams) { stream ->
                session?.sessionExtras = Bundle().apply { putString(QUALITY_EXTRA, stream.quality) }
            }
        session =
            MediaSession
                .Builder(this, player)
                .setCallback(Queueing)
                .setSessionActivity(
                    PendingIntent.getActivity(
                        this,
                        0,
                        Intent(this, MainActivity::class.java),
                        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                    ),
                ).build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onDestroy() {
        session?.run {
            heard.release()
            player.release()
            release()
        }
        session = null
        super.onDestroy()
    }

    /** Tracks come from the app without where they stream from: their id says. */
    private object Queueing : MediaSession.Callback {
        override fun onAddMediaItems(
            mediaSession: MediaSession,
            controller: MediaSession.ControllerInfo,
            mediaItems: MutableList<MediaItem>,
        ): ListenableFuture<MutableList<MediaItem>> =
            Futures.immediateFuture(
                mediaItems
                    .map { item ->
                        val trackId = trackIdOf(item.mediaId) ?: return@map item
                        item.buildUpon().setUri(trackUri(trackId)).build()
                    }.toMutableList(),
            )
    }

    /**
     * A stream broke, as when its address expired during a long pause, or the network went
     * away: ask where it streams from again, and go on from the same place.
     */
    private class Recovery(private val player: Player, private val streams: Streams) : Player.Listener {
        private var recoveredAt = 0L

        override fun onPlayerError(error: PlaybackException) {
            val io = error.errorCode in 2000..2999
            val now = SystemClock.elapsedRealtime()
            if (!io || (recoveredAt != 0L && now - recoveredAt < RECOVER_EVERY_MS)) {
                Log.w("Pochette", "Playback stopped: ${error.errorCodeName}", error)
                return
            }
            Log.i("Pochette", "Stream broke (${error.errorCodeName}): trying again")
            recoveredAt = now
            player.currentMediaItem?.mediaId?.let(::trackIdOf)?.let(streams::forget)
            player.prepare()
        }
    }
}
