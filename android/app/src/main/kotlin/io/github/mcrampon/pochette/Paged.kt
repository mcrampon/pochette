package io.github.mcrampon.pochette

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import io.github.mcrampon.pochette.core.AlbumCard
import io.github.mcrampon.pochette.core.AlbumList
import io.github.mcrampon.pochette.core.PochetteException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/** Albums asked of Qobuz this many at a time, as on desktop. */
const val PAGE_SIZE = 50u

/** A long list of albums, loaded a page at a time as it's scrolled. */
class Paged(private val scope: CoroutineScope, private val fetch: suspend (offset: UInt) -> AlbumList) {
    val albums = mutableStateListOf<AlbumCard>()
    var hasMore by mutableStateOf(true)
        private set
    var loading by mutableStateOf(false)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    /** Load the next page, unless one is loading or there's none. */
    fun more() {
        if (loading || !hasMore) return
        loading = true
        error = null
        scope.launch {
            try {
                val page = fetch(albums.size.toUInt())
                albums.addAll(page.albums)
                hasMore = page.hasMore && page.albums.isNotEmpty()
            } catch (err: PochetteException) {
                error = err.message
            } finally {
                loading = false
            }
        }
    }
}
