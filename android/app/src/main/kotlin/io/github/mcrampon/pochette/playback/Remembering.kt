package io.github.mcrampon.pochette.playback

import android.os.Handler
import android.os.Looper
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.Timeline
import io.github.mcrampon.pochette.core.Pochette
import io.github.mcrampon.pochette.core.ResumePoint

/** How often where the listening is gets saved while playing: what a crash loses at most. */
private const val SAVE_EVERY_MS = 10_000L

/**
 * Saves where the listening is, as on desktop, for the app to take it up when it opens again:
 * as a track starts, on pause and seek, from time to time while playing, and as the service
 * goes. Played to its end, or the queue emptied, the album leaves nothing to take up.
 */
class Remembering(private val core: Pochette, private val player: Player) : Player.Listener {
    private val clock = Handler(Looper.getMainLooper())
    private val tick =
        object : Runnable {
            override fun run() {
                save()
                clock.postDelayed(this, SAVE_EVERY_MS)
            }
        }

    init {
        player.addListener(this)
    }

    override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) = save()

    override fun onIsPlayingChanged(isPlaying: Boolean) {
        clock.removeCallbacks(tick)
        if (isPlaying) clock.postDelayed(tick, SAVE_EVERY_MS)
        save()
    }

    override fun onPositionDiscontinuity(
        oldPosition: Player.PositionInfo,
        newPosition: Player.PositionInfo,
        reason: Int,
    ) {
        if (reason == Player.DISCONTINUITY_REASON_SEEK) save()
    }

    override fun onPlaybackStateChanged(playbackState: Int) {
        if (playbackState == Player.STATE_ENDED) core.forgetResumePoint()
    }

    /** Emptied, as when signing out: nothing to take up. */
    override fun onTimelineChanged(timeline: Timeline, reason: Int) {
        if (timeline.isEmpty) core.forgetResumePoint()
    }

    /** The player is going away: where it was is saved one last time. */
    fun release() {
        clock.removeCallbacks(tick)
        player.removeListener(this)
        save()
    }

    private fun save() {
        // Nothing queued, as before anything plays or once stopped, or played to the end.
        val item = player.currentMediaItem ?: return
        if (player.playbackState == Player.STATE_ENDED) return
        val trackId = trackIdOf(item.mediaId) ?: return
        core.saveResumePoint(
            ResumePoint(albumIdOf(item.mediaId), trackId, player.currentPosition.coerceAtLeast(0).toULong()),
        )
    }
}
