package io.github.mcrampon.pochette.playback

import android.content.ComponentName
import android.content.Context
import android.net.Uri
import android.os.Bundle
import android.os.SystemClock
import androidx.core.content.ContextCompat
import androidx.core.net.toUri
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.common.util.Util
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import io.github.mcrampon.pochette.core.AlbumPage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** A track in the queue. */
data class Queued(val mediaId: String, val title: String, val artist: String, val durationMs: Long)

/** What's playing, as the screens show it. */
data class NowPlaying(
    val albumId: String,
    val trackId: ULong?,
    val title: String,
    val artist: String,
    val album: String,
    val albumArtist: String,
    val cover: Uri?,
    val playing: Boolean,
    val hasNext: Boolean,
    /** Why it stopped, if it broke. */
    val error: String?,
    val queue: List<Queued>,
    /** Of the track playing, in the queue. */
    val index: Int,
    val durationMs: Long,
    /** The stream's: "24-bit / 96 kHz", once it plays. */
    val quality: String?,
)

/** How far the position goes on past what the player last told, in case it stalled. */
private const val MAX_AHEAD_MS = 300L

/** The key of the stream's quality, in the session's extras. */
const val QUALITY_EXTRA = "quality"

/** The screens' remote for the playback service. */
@UnstableApi
class Playback(context: Context) {
    private val _now = MutableStateFlow<NowPlaying?>(null)
    val now: StateFlow<NowPlaying?> = _now.asStateFlow()

    private val connecting =
        MediaController
            .Builder(context, SessionToken(context, ComponentName(context, PlaybackService::class.java)))
            .setListener(
                object : MediaController.Listener {
                    override fun onExtrasChanged(controller: MediaController, extras: Bundle) = publish(controller)
                },
            ).buildAsync()
    private var controller: MediaController? = null

    /** What was asked before the service answered. */
    private var waiting: ((MediaController) -> Unit)? = null

    private val listener =
        object : Player.Listener {
            override fun onEvents(player: Player, events: Player.Events) = publish(player)
        }

    init {
        connecting.addListener(
            {
                val connected = runCatching { connecting.get() }.getOrNull() ?: return@addListener
                controller = connected
                connected.addListener(listener)
                publish(connected)
                waiting?.invoke(connected)
                waiting = null
            },
            ContextCompat.getMainExecutor(context),
        )
    }

    /** The last position the player told, and when it was first seen. */
    private var reported = -1L
    private var reportedAt = 0L

    /**
     * Where the track playing is, in milliseconds: ask it again as it plays. The player tells
     * its position every quarter of a second: in between, it goes on with the clock, so the
     * lyrics light up on time.
     */
    val positionMs: Long get() {
        val told = PlaybackService.position?.invoke() ?: controller?.currentPosition ?: 0
        val now = SystemClock.elapsedRealtime()
        if (told != reported) {
            reported = told
            reportedAt = now
        }
        if (controller?.isPlaying != true) return told
        return told + (now - reportedAt).coerceAtMost(MAX_AHEAD_MS)
    }

    /** Play `album`'s tracks that can be played, from the track `from`, else the first. */
    fun play(album: AlbumPage, from: ULong? = null) {
        val cover = (album.card.coverLarge ?: album.card.cover)?.toUri()
        val items =
            album.tracks.filter { it.streamable }.map { track ->
                MediaItem
                    .Builder()
                    .setMediaId(mediaId(album.card.id, track.id))
                    .setMediaMetadata(
                        MediaMetadata
                            .Builder()
                            .setTitle(track.title)
                            .setArtist(track.performer ?: album.card.artist)
                            .setAlbumTitle(album.card.title)
                            .setAlbumArtist(album.card.artist)
                            .setArtworkUri(cover)
                            .setTrackNumber(track.number.toInt())
                            .setDiscNumber(track.disc.toInt())
                            .setDurationMs(track.duration.toLong() * 1000)
                            .setIsPlayable(true)
                            .setIsBrowsable(false)
                            .build(),
                    ).build()
            }
        if (items.isEmpty()) return
        val start = from?.let { mediaId(album.card.id, it) }?.let { id -> items.indexOfFirst { it.mediaId == id } } ?: 0
        withController {
            it.setMediaItems(items, start.coerceAtLeast(0), 0)
            it.prepare()
            it.play()
        }
    }

    fun playPause() = withController { Util.handlePlayPauseButtonAction(it) }

    fun next() = withController { it.seekToNextMediaItem() }

    /** The start of the track, or the one before when already there. */
    fun previous() = withController { it.seekToPrevious() }

    fun seekTo(positionMs: Long) = withController { it.seekTo(positionMs) }

    /** Play the queue's track at `index`. */
    fun skipTo(index: Int) =
        withController {
            it.seekTo(index, 0)
            if (it.playbackState == Player.STATE_IDLE) it.prepare()
            it.play()
        }

    fun stop() =
        withController {
            it.stop()
            it.clearMediaItems()
        }

    fun release() = MediaController.releaseFuture(connecting)

    private fun withController(action: (MediaController) -> Unit) {
        controller?.let(action) ?: run { waiting = action }
    }

    private fun publish(player: Player) {
        val item = player.currentMediaItem
        val queue =
            (0 until player.mediaItemCount).map { index ->
                val queued = player.getMediaItemAt(index)
                Queued(
                    mediaId = queued.mediaId,
                    title = queued.mediaMetadata.title?.toString().orEmpty(),
                    artist = queued.mediaMetadata.artist?.toString().orEmpty(),
                    durationMs = queued.mediaMetadata.durationMs ?: 0,
                )
            }
        _now.value =
            item?.let {
                val metadata = it.mediaMetadata
                NowPlaying(
                    albumId = albumIdOf(it.mediaId),
                    trackId = trackIdOf(it.mediaId),
                    title = metadata.title?.toString().orEmpty(),
                    artist = metadata.artist?.toString().orEmpty(),
                    album = metadata.albumTitle?.toString().orEmpty(),
                    albumArtist = metadata.albumArtist?.toString().orEmpty(),
                    cover = metadata.artworkUri,
                    playing = !Util.shouldShowPlayButton(player),
                    hasNext = player.hasNextMediaItem(),
                    error = player.playerError?.let(::describe),
                    queue = queue,
                    index = player.currentMediaItemIndex,
                    // The stream's, once known: Qobuz's may be a little off.
                    durationMs = player.duration.takeIf { d -> d != C.TIME_UNSET } ?: metadata.durationMs ?: 0,
                    quality = (player as? MediaController)?.sessionExtras?.getString(QUALITY_EXTRA),
                )
            }
    }

    /** Only the error's code crosses from the service: its cause stays there. */
    private fun describe(error: PlaybackException): String =
        when (error.errorCode) {
            PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_FAILED,
            PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_TIMEOUT,
            -> "No connection: tap play to try again"
            PlaybackException.ERROR_CODE_IO_BAD_HTTP_STATUS -> "Qobuz refused the stream"
            in 2000..2999 -> "Can't get the stream from Qobuz"
            else -> "Can't play this track"
        }
}
