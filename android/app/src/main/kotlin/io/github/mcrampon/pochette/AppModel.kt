package io.github.mcrampon.pochette

import android.app.Application
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import io.github.mcrampon.pochette.core.Account
import io.github.mcrampon.pochette.core.AlbumCard
import io.github.mcrampon.pochette.core.AlbumPage
import io.github.mcrampon.pochette.core.ArtistCard
import io.github.mcrampon.pochette.core.ArtistPage
import io.github.mcrampon.pochette.core.Discover
import io.github.mcrampon.pochette.core.Genre
import io.github.mcrampon.pochette.core.LibraryView
import io.github.mcrampon.pochette.core.Lyrics
import io.github.mcrampon.pochette.core.Period
import io.github.mcrampon.pochette.core.Plays
import io.github.mcrampon.pochette.core.PochetteException
import io.github.mcrampon.pochette.core.Sort
import io.github.mcrampon.pochette.core.Stats
import io.github.mcrampon.pochette.playback.NowPlaying
import io.github.mcrampon.pochette.playback.Playback
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.random.Random

/** Where the app is: signing in, or in the user's library. */
sealed interface Stage {
    data object Starting : Stage

    /** Not signed in: `url` while the sign-in page is open, `error` if the last try failed. */
    data class SignedOut(val url: String? = null, val error: String? = null) : Stage

    /** The kept session couldn't be checked, as without a network. */
    data class Unreachable(val error: String) : Stage

    data class SignedIn(val account: Account) : Stage
}

/** The user's albums, as the library shows them: `any` when there are some, shown or not. */
data class Library(
    val albums: List<AlbumCard> = emptyList(),
    val any: Boolean = false,
    val refreshing: Boolean = false,
    val error: String? = null,
)

/** Something loaded from Qobuz, as a screen shows it. */
sealed interface Load<out T> {
    data object Pending : Load<Nothing>

    data class Done<T>(val value: T) : Load<T>

    data class Failed(val error: String) : Load<Nothing>
}

/** Search results: the artists, and the albums a page at a time. */
data class Found(val query: String, val artists: Load<List<ArtistCard>>, val albums: Paged)

/** The stats of `period`, and the periods with plays to pick from: the years, latest first, and the months of the year shown. */
data class StatsPage(val period: Period, val years: List<Int>, val months: List<Int>, val stats: Stats)

/** How many artists a search shows, as on desktop. */
private const val ARTISTS_FOUND = 12u

/** What the screens show, and what they ask of the core. */
@androidx.annotation.OptIn(androidx.media3.common.util.UnstableApi::class)
class AppModel(app: Application) : AndroidViewModel(app) {
    private val core = (app as PochetteApp).core
    private val sessions = SessionStore(app)
    val settings = Settings(app)

    private val _stage = MutableStateFlow<Stage>(Stage.Starting)
    val stage: StateFlow<Stage> = _stage.asStateFlow()

    private val _library = MutableStateFlow(Library())
    val library: StateFlow<Library> = _library.asStateFlow()

    val playback = Playback(app)

    /** Something that went wrong, to show once. */
    private val _notice = MutableStateFlow<String?>(null)
    val notice: StateFlow<String?> = _notice.asStateFlow()

    val navigation = Navigation()

    /** Album pages already loaded, as going back to one is common. */
    private val pages = HashMap<String, AlbumPage>()
    private val artists = HashMap<ULong, ArtistPage>()

    /** Lyrics already found, by track. */
    private val lyrics = HashMap<ULong, Lyrics>()

    /** Long lists, by the key of their screen: they stay as scrolled while it's open. */
    private val lists = HashMap<String, Paged>()

    /** Discover, for the genres picked. */
    var discover by mutableStateOf<Load<Discover>>(Load.Pending)
        private set
    private var genres: List<Genre>? = null

    var found by mutableStateOf<Found?>(null)
        private set
    private var searching: Job? = null

    /** Show the library's albums as `view` says, keeping its sort and filters for next time. */
    fun show(view: LibraryView) {
        this.view = view
        settings.libraryView = view
        filtering?.cancel()
        filtering = viewModelScope.launch { reshow() }
    }

    /** The genres and decades of the user's albums, to filter them by. */
    fun libraryGenres(): List<String> = core.libraryGenres()

    fun libraryDecades(): List<UShort> = core.libraryDecades()

    /** Play one of the albums shown, picked at random: another than the one playing, when there's another. */
    fun playRandom() {
        val albums = _library.value.albums
        if (albums.isEmpty()) return
        var index = Random.nextInt(albums.size)
        if (albums.size > 1 && albums[index].id == playback.now.value?.albumId) index = (index + 1) % albums.size
        val card = albums[index]
        viewModelScope.launch {
            try {
                playback.play(album(card.id))
            } catch (expired: PochetteException.SignedOut) {
                return@launch
            } catch (err: PochetteException) {
                _notice.value = "Can't play “${card.title}”: ${err.message}"
            }
        }
    }

    /** The stats period picked, none for the latest year. */
    var statsPeriod by mutableStateOf<Period?>(null)

    /** Which albums the library shows, and in what order. */
    var view by mutableStateOf(settings.libraryView)
        private set
    private var filtering: Job? = null
    private var signingIn: Job? = null

    val sync = (app as PochetteApp).sync

    init {
        resume()
        // Sorted by plays, the albums move as plays come, here or from another device.
        viewModelScope.launch {
            sync.plays.drop(1).collect {
                if (_stage.value is Stage.SignedIn && view.sort in listOf(Sort.LAST_PLAYED, Sort.PLAY_COUNT)) reshow()
            }
        }
    }

    /** Pair with a device nearby, by its id, or at an address. */
    fun pair(target: String) = core.pair(target)

    /** Whether the user saw the same code on both devices. */
    fun answerPairing(accept: Boolean) = core.answerPairing(accept)

    fun dismissPairing() = core.dismissPairing()

    /** Stop syncing with a device. */
    fun forget(id: String) = core.forget(id)

    /** Be ready to pair, as while the settings show. */
    fun setPairable(pairable: Boolean) = core.setPairable(pairable)

    var syncInBackground: Boolean
        get() = settings.syncInBackground
        set(value) {
            settings.syncInBackground = value
            SyncWorker.schedule(getApplication(), value)
        }

    /** Sign in with the session kept from before, if there's one. */
    fun resume() {
        val kept = sessions.load()
        if (kept == null) {
            _stage.value = Stage.SignedOut()
            return
        }
        _stage.value = Stage.Starting
        viewModelScope.launch {
            try {
                signedIn(core.restore(kept))
            } catch (expired: PochetteException.SignedOut) {
                sessions.clear()
                _stage.value = Stage.SignedOut(error = "Sign in again: ${expired.message}")
            } catch (err: PochetteException) {
                _stage.value = Stage.Unreachable(err.message.orEmpty())
            }
        }
    }

    /** Open Qobuz's sign-in page with `open`, and wait for the user to sign in there. */
    fun signIn(open: (String) -> Unit) {
        signingIn?.cancel()
        signingIn =
            viewModelScope.launch {
                try {
                    val signIn = core.beginSignIn(SIGNED_IN)
                    _stage.value = Stage.SignedOut(url = signIn.url())
                    open(signIn.url())
                    signIn.redirected()
                    // Android keeps apps in the background off the network: the screen
                    // brings the app back, if the browser didn't, before Qobuz is asked.
                    _stage.value = Stage.Starting
                    signedIn(signIn.finish())
                } catch (err: PochetteException) {
                    _stage.value = Stage.SignedOut(error = err.message)
                }
            }
    }

    fun cancelSignIn() {
        signingIn?.cancel()
        _stage.value = Stage.SignedOut()
    }

    fun open(screen: Screen) = navigation.open(screen)

    /** The album's page, from Qobuz the first time. */
    suspend fun album(id: String): AlbumPage = pages[id] ?: ask { core.album(id) }.also { pages[id] = it }

    suspend fun artist(id: ULong): ArtistPage = artists[id] ?: ask { core.artist(id) }.also { artists[id] = it }

    /** The lyrics of the track playing, from LRCLIB the first time. */
    suspend fun lyrics(now: NowPlaying): Lyrics {
        val trackId = now.trackId ?: return Lyrics.Missing
        return lyrics[trackId] ?: core.lyrics(now.albumId, trackId).also { lyrics[trackId] = it }
    }

    /** An album the library knows, to show before its page loads. */
    fun card(id: String): AlbumCard? = _library.value.albums.find { it.id == id }

    /** How often the album was played, on every device: it changes as tracks are heard. */
    fun plays(id: String): Plays = core.albumPlays(id)

    fun isFavorite(id: String): Boolean = core.isFavorite(id)

    /** Add the album to the favorites, or take it out: `failed` if Qobuz refused. */
    fun setFavorite(album: AlbumCard, favorite: Boolean, failed: () -> Unit) {
        viewModelScope.launch {
            try {
                core.setFavorite(album.id, favorite)
                reshow()
            } catch (err: PochetteException) {
                failed()
                val change = if (favorite) "add “${album.title}” to" else "take “${album.title}” out of"
                _notice.value = "Can't $change your favorites: ${err.message}"
            }
        }
    }

    /** The albums of a screen's long list, loading the first page the first time. */
    fun list(screen: Screen): Paged =
        lists.getOrPut(screen.key) {
            val fetch: suspend (UInt) -> io.github.mcrampon.pochette.core.AlbumList =
                when (screen) {
                    is Screen.Section -> { offset -> core.discoverSection(screen.section, screen.genres, offset, PAGE_SIZE) }
                    is Screen.Releases -> { offset -> core.artistReleases(screen.artistId, screen.kind, offset, PAGE_SIZE) }
                    else -> error("${screen.key} has no list")
                }
            Paged(viewModelScope) { offset -> ask { fetch(offset) } }.also { it.more() }
        }

    /** Load Discover again, for the genres picked: after picking others, or failing. */
    fun loadDiscover() {
        discover = Load.Pending
        val picked = settings.discoverGenres
        viewModelScope.launch {
            discover =
                try {
                    Load.Done(ask { core.discover(picked) })
                } catch (err: PochetteException) {
                    Load.Failed(err.message.orEmpty())
                }
        }
    }

    /** The genres Discover can show, from Qobuz the first time. */
    suspend fun genres(): List<Genre> = genres ?: ask { core.genres() }.also { genres = it }

    fun pickGenres(picked: List<ULong>) {
        settings.discoverGenres = picked
        loadDiscover()
    }

    /** Search Qobuz for `query`: artists, and albums a page at a time. */
    fun search(query: String) {
        val trimmed = query.trim()
        if (trimmed == found?.query) return
        searching?.cancel()
        if (trimmed.isEmpty()) {
            found = null
            return
        }
        val albums = Paged(viewModelScope) { offset -> ask { core.searchAlbums(trimmed, offset, PAGE_SIZE) } }
        found = Found(trimmed, Load.Pending, albums)
        albums.more()
        searching =
            viewModelScope.launch {
                val artists =
                    try {
                        Load.Done(ask { core.searchArtists(trimmed, ARTISTS_FOUND) })
                    } catch (err: PochetteException) {
                        Load.Failed(err.message.orEmpty())
                    }
                found = found?.takeIf { it.query == trimmed }?.copy(artists = artists)
            }
    }

    /** Close the screen on top, forgetting what it loaded. Returns the key of its state. */
    fun back(): String? {
        val closing = navigation.top
        val key = navigation.back() ?: return null
        if (navigation.screens.none { it.key == closing.key }) lists.remove(closing.key)
        return key
    }

    /**
     * The stats of the period picked, with the plays the logs hold now; none before anything was played. A period
     * that no longer has plays, as syncing may remove some, gives way to its year or the latest year.
     */
    suspend fun stats(): StatsPage? =
        withContext(Dispatchers.IO) {
            val years = core.statsYears()
            val latest = years.firstOrNull() ?: return@withContext null
            val period =
                when (val picked = statsPeriod) {
                    null -> Period.Year(latest)
                    Period.All -> picked
                    is Period.Year -> if (picked.year in years) picked else Period.Year(latest)
                    is Period.Month ->
                        when {
                            picked.year !in years -> Period.Year(latest)
                            picked.month !in core.statsMonths(picked.year) -> Period.Year(picked.year)
                            else -> picked
                        }
                }
            val months =
                when (period) {
                    Period.All -> emptyList()
                    is Period.Year -> core.statsMonths(period.year)
                    is Period.Month -> core.statsMonths(period.year)
                }
            StatsPage(period, years, months, core.stats(period))
        }

    fun noticeShown() {
        _notice.value = null
    }

    override fun onCleared() {
        playback.release()
    }

    fun signOut() {
        playback.stop()
        core.signOut()
        pages.clear()
        artists.clear()
        lists.clear()
        discover = Load.Pending
        found = null
        statsPeriod = null
        navigation.reset()
        sessions.clear()
        _library.value = Library()
        _stage.value = Stage.SignedOut()
    }

    /** Get the user's albums from Qobuz again. */
    fun refresh() {
        if (_library.value.refreshing) return
        _library.value = _library.value.copy(refreshing = true, error = null)
        viewModelScope.launch {
            val error =
                try {
                    ask { core.refreshLibrary() }
                    null
                } catch (expired: PochetteException.SignedOut) {
                    return@launch
                } catch (err: PochetteException) {
                    err.message
                }
            _library.value = _library.value.copy(refreshing = false, error = error)
            reshow()
        }
    }

    /** Ask Qobuz, signing out if it no longer takes the session. */
    private suspend fun <T> ask(call: suspend () -> T): T =
        try {
            call()
        } catch (expired: PochetteException.SignedOut) {
            if (_stage.value is Stage.SignedIn) signOut()
            throw expired
        }

    private suspend fun signedIn(account: Account) {
        core.session()?.let(sessions::save)
        _stage.value = Stage.SignedIn(account)
        takeUpListening()
        refresh()
    }

    /** Get the album that was playing when the app last went ready to play again, paused where it was. */
    private fun takeUpListening() {
        val point = core.resumePoint() ?: return
        viewModelScope.launch {
            val page =
                try {
                    album(point.albumId)
                } catch (err: PochetteException) {
                    Log.w("Pochette", "Can't load album ${point.albumId} to take up the listening: ${err.message}")
                    return@launch
                }
            playback.cue(page, point.trackId, point.positionMs.toLong())
        }
    }

    /** Show the albums the view picks. */
    private suspend fun reshow() {
        val view = view
        val (albums, any) =
            withContext(Dispatchers.Default) {
                val albums = core.library(view)
                // Whether none match, or there's none at all.
                albums to (albums.isNotEmpty() || core.library(LibraryView("", emptyList(), emptyList(), Sort.DATE_ADDED, false)).isNotEmpty())
            }
        _library.value = _library.value.copy(albums = albums, any = any)
    }

    companion object {
        /** Opens the app: the browser goes there once signed in. See the manifest. */
        const val SIGNED_IN = "io.github.mcrampon.pochette://signed-in"
    }
}
