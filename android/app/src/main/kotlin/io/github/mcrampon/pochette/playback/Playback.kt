package io.github.mcrampon.pochette.playback

import android.content.ComponentName
import android.content.Context
import android.net.Uri
import androidx.core.content.ContextCompat
import androidx.core.net.toUri
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

/** What's playing, as the screens show it. */
data class NowPlaying(
    val albumId: String,
    val trackId: ULong?,
    val title: String,
    val artist: String,
    val cover: Uri?,
    val playing: Boolean,
    val hasNext: Boolean,
    /** Why it stopped, if it broke. */
    val error: String?,
)

/** The screens' remote for the playback service. */
@UnstableApi
class Playback(context: Context) {
    private val _now = MutableStateFlow<NowPlaying?>(null)
    val now: StateFlow<NowPlaying?> = _now.asStateFlow()

    private val connecting =
        MediaController
            .Builder(context, SessionToken(context, ComponentName(context, PlaybackService::class.java)))
            .buildAsync()
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
        _now.value =
            item?.let {
                NowPlaying(
                    albumId = albumIdOf(it.mediaId),
                    trackId = trackIdOf(it.mediaId),
                    title = it.mediaMetadata.title?.toString().orEmpty(),
                    artist = it.mediaMetadata.artist?.toString().orEmpty(),
                    cover = it.mediaMetadata.artworkUri,
                    playing = !Util.shouldShowPlayButton(player),
                    hasNext = player.hasNextMediaItem(),
                    error = player.playerError?.let(::describe),
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
