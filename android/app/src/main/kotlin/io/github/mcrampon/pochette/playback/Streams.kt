package io.github.mcrampon.pochette.playback

import android.net.Uri
import android.os.SystemClock
import androidx.core.net.toUri
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.ResolvingDataSource
import io.github.mcrampon.pochette.core.Pochette
import io.github.mcrampon.pochette.core.PochetteException
import io.github.mcrampon.pochette.core.Quality
import io.github.mcrampon.pochette.core.Stream
import java.io.IOException
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.runBlocking

/** What the queue holds for a track: `pochette://track/<id>`, where it streams from comes later. */
fun trackUri(trackId: ULong): Uri = "$SCHEME://track/$trackId".toUri()

/** The queue's id for a track, which says which album it's played from. */
fun mediaId(albumId: String, trackId: ULong) = "$albumId/$trackId"

fun albumIdOf(mediaId: String): String = mediaId.substringBeforeLast('/')

fun trackIdOf(mediaId: String): ULong? = mediaId.substringAfterLast('/').toULongOrNull()

private const val SCHEME = "pochette"

/** Qobuz's addresses last about an hour: one older than this is asked again. */
private const val FRESH_FOR_MS = 30 * 60 * 1000L

/**
 * Where each track streams from, asked of Qobuz when the player opens it, not when it's
 * queued: an address fetched before a long pause would have expired. A track downloaded
 * plays from its file.
 */
@UnstableApi
class Streams(private val core: Pochette, private val quality: () -> Quality) : ResolvingDataSource.Resolver {
    private class Fetched(val stream: Stream, val at: Long)

    private val fetched = ConcurrentHashMap<ULong, Fetched>()

    /** The stream a track was last opened from, if it was. */
    fun of(trackId: ULong): Stream? = fetched[trackId]?.stream

    /** Ask again where every track streams from, as when the quality changed. */
    fun forgetAll() = fetched.clear()

    /** Ask again where the track streams from, next time it's opened. */
    fun forget(trackId: ULong) {
        fetched.remove(trackId)
    }

    /** On the player's loading thread, which may wait. */
    override fun resolveDataSpec(dataSpec: DataSpec): DataSpec {
        if (dataSpec.uri.scheme != SCHEME) return dataSpec
        val trackId =
            dataSpec.uri.lastPathSegment?.toULongOrNull() ?: throw IOException("Not a track: ${dataSpec.uri}")
        return dataSpec.withUri(fresh(trackId).url.toUri())
    }

    private fun fresh(trackId: ULong): Stream {
        // Downloaded, it plays from its file, offline too.
        core.downloadedStream(trackId)?.let {
            fetched[trackId] = Fetched(it, SystemClock.elapsedRealtime())
            return it
        }
        fetched[trackId]?.takeIf { SystemClock.elapsedRealtime() - it.at < FRESH_FOR_MS }?.let { return it.stream }
        val stream =
            try {
                runBlocking { core.stream(trackId, quality()) }
            } catch (err: PochetteException) {
                throw IOException(err.message, err)
            }
        fetched[trackId] = Fetched(stream, SystemClock.elapsedRealtime())
        return stream
    }
}
