package io.github.mcrampon.pochette.playback

import android.app.PendingIntent
import android.content.Context
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
import androidx.media3.session.DefaultMediaNotificationProvider
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import io.github.mcrampon.pochette.Language
import io.github.mcrampon.pochette.MainActivity
import io.github.mcrampon.pochette.PochetteApp
import io.github.mcrampon.pochette.R
import io.github.mcrampon.pochette.Settings
import io.github.mcrampon.pochette.Sync

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
    private lateinit var remembering: Remembering
    private lateinit var onTheNetwork: OnTheNetwork

    /** Held here: preferences only keep a weak reference to their listeners. */
    private lateinit var qualityChanged: SharedPreferences.OnSharedPreferenceChangeListener

    // The notification in the language picked, before Android 13, which does it itself.
    override fun attachBaseContext(newBase: Context) = super.attachBaseContext(Language.wrap(newBase))

    override fun onCreate() {
        super.onCreate()
        // The app's icon in the status bar while it plays, not Media3's.
        setMediaNotificationProvider(
            DefaultMediaNotificationProvider.Builder(this).build().apply { setSmallIcon(R.drawable.ic_notification) },
        )
        val core = (application as PochetteApp).core
        Language.tell(this)
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
        onTheNetwork = OnTheNetwork((application as PochetteApp).sync)
        player.addListener(onTheNetwork)
        position = player::getCurrentPosition
        heard =
            Heard(core, player, streams) { stream ->
                session?.sessionExtras = Bundle().apply { putString(QUALITY_EXTRA, stream.quality) }
            }
        remembering = Remembering(core, player)
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

    companion object {
        /**
         * Where the player is, read from it, on the main thread: the screens run in the same
         * process, and a controller's position is only an estimate, off by up to 0.2 s.
         */
        var position: (() -> Long)? = null
            private set
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onDestroy() {
        position = null
        session?.run {
            remembering.release()
            heard.release()
            player.release()
            release()
        }
        session = null
        // Released, the player may not say it stopped.
        onTheNetwork.onIsPlayingChanged(false)
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
    /**
     * Syncs what's heard as it's heard while it plays, the app closed or the screen off: Android lets a playing app
     * on the network. Not while it's paused, when the service may linger.
     */
    private class OnTheNetwork(private val sync: Sync) : Player.Listener {
        private var holding = false

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            if (isPlaying == holding) return
            holding = isPlaying
            if (isPlaying) sync.hold() else sync.release()
        }
    }

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
