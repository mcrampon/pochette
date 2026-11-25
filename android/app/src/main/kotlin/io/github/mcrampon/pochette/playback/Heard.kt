package io.github.mcrampon.pochette.playback

import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import io.github.mcrampon.pochette.core.Listening
import io.github.mcrampon.pochette.core.Pochette
import io.github.mcrampon.pochette.core.PochetteException
import io.github.mcrampon.pochette.core.Stream

private const val TAG = "Pochette"

/**
 * What's heard of each track, told to the core as on desktop: Qobuz hears of each stretch
 * played, and the play history gets the track once the player leaves it.
 */
@UnstableApi
class Heard(
    private val core: Pochette,
    private val player: Player,
    private val streams: Streams,
    /** A track started playing from `stream`. */
    private val started: (Stream) -> Unit,
) : Player.Listener {
    /** The track the player is on, at `index` of the queue. */
    private class Current(val index: Int, val mediaId: String) {
        /** Once it started playing: none if the core refused it. */
        var listening: Listening? = null
        var started = false

        /** A stretch is going: the sound plays, or is about to. */
        var stretching = false
    }

    private var current: Current? = null

    /** Where the player last was on the current track, for when it's already left it. */
    private var position = 0L
    private val clock = Handler(Looper.getMainLooper())
    private val tick =
        object : Runnable {
            override fun run() {
                position = player.currentPosition
                clock.postDelayed(this, 1000)
            }
        }

    init {
        player.addListener(this)
        clock.post(tick)
    }

    override fun onPositionDiscontinuity(
        oldPosition: Player.PositionInfo,
        newPosition: Player.PositionInfo,
        reason: Int,
    ) {
        val track = current ?: return
        if (track.stretching && oldPosition.mediaItemIndex == track.index &&
            oldPosition.mediaItem?.mediaId == track.mediaId
        ) {
            if (reason == Player.DISCONTINUITY_REASON_AUTO_TRANSITION) {
                track.listening?.finished()
            } else {
                track.listening?.stretchEnded(oldPosition.positionMs.toULong())
            }
            track.stretching = false
        }
        update()
    }

    override fun onEvents(player: Player, events: Player.Events) = update()

    /** The player is going away: what was heard goes into the history. */
    fun release() {
        clock.removeCallbacks(tick)
        player.removeListener(this)
        current?.let { end(it, player.currentPosition) }
        current = null
    }

    private fun update() {
        val item = player.currentMediaItem
        val index = player.currentMediaItemIndex
        current?.let {
            if (item == null || item.mediaId != it.mediaId || index != it.index) {
                end(it, position)
                current = null
            }
        }
        if (item == null) return
        val track = current ?: Current(index, item.mediaId).also { current = it }

        if (player.playbackState == Player.STATE_ENDED) {
            if (track.stretching) track.listening?.finished()
            track.stretching = false
            end(track, player.currentPosition)
            current = null
            return
        }
        // Waiting for the network counts, as on desktop: the position doesn't move meanwhile.
        val playing =
            player.playWhenReady && player.playbackSuppressionReason == Player.PLAYBACK_SUPPRESSION_REASON_NONE &&
                (player.playbackState == Player.STATE_READY || player.playbackState == Player.STATE_BUFFERING)
        position = player.currentPosition
        if (playing && !track.stretching && start(track)) {
            track.listening?.stretchStarted(position.toULong())
            track.stretching = true
        } else if (!playing && track.stretching) {
            track.listening?.stretchEnded(position.toULong())
            track.stretching = false
        }
    }

    /** Tell the core the track started playing, once its stream is known. */
    private fun start(track: Current): Boolean {
        if (track.started) return true
        val trackId = trackIdOf(track.mediaId) ?: return false
        val stream = streams.of(trackId) ?: return false
        track.started = true
        started(stream)
        track.listening =
            try {
                core.startListening(albumIdOf(track.mediaId), stream)
            } catch (err: PochetteException) {
                Log.w(TAG, "Not counting track $trackId: ${err.message}")
                null
            }
        return true
    }

    private fun end(track: Current, at: Long) {
        val listening = track.listening ?: return
        if (track.stretching) listening.stretchEnded(at.toULong())
        track.stretching = false
        val seconds = listening.heardSeconds()
        try {
            val kept = listening.end()
            Log.i(TAG, "Heard ${track.mediaId} for $seconds s, kept: $kept")
        } catch (err: PochetteException) {
            Log.w(TAG, "Can't keep what was heard: ${err.message}")
        }
        listening.destroy()
    }
}
