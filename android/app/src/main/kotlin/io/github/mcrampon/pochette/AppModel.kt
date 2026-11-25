package io.github.mcrampon.pochette

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import io.github.mcrampon.pochette.core.Account
import androidx.compose.runtime.mutableStateListOf
import io.github.mcrampon.pochette.core.AlbumCard
import io.github.mcrampon.pochette.core.AlbumPage
import io.github.mcrampon.pochette.core.Plays
import io.github.mcrampon.pochette.core.LibraryView
import io.github.mcrampon.pochette.core.PochetteException
import io.github.mcrampon.pochette.core.Sort
import io.github.mcrampon.pochette.playback.Playback
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** Where the app is: signing in, or in the user's library. */
sealed interface Stage {
    data object Starting : Stage

    /** Not signed in: `url` while the sign-in page is open, `error` if the last try failed. */
    data class SignedOut(val url: String? = null, val error: String? = null) : Stage

    /** The kept session couldn't be checked, as without a network. */
    data class Unreachable(val error: String) : Stage

    data class SignedIn(val account: Account) : Stage
}

/** A screen of the signed-in app, in the back stack. */
sealed interface Screen {
    /** Tells apart the same screen opened twice, so each keeps its scroll. */
    val key: String

    data object Albums : Screen {
        override val key = "albums"
    }

    /** `preview`, when known, shows while the page loads. */
    data class Album(val id: String, val preview: AlbumCard? = null) : Screen {
        override val key = "album/$id"
    }
}

/** The user's albums, as the library shows them. */
data class Library(
    val albums: List<AlbumCard> = emptyList(),
    val refreshing: Boolean = false,
    val error: String? = null,
)

/** What the screens show, and what they ask of the core. */
@androidx.annotation.OptIn(androidx.media3.common.util.UnstableApi::class)
class AppModel(app: Application) : AndroidViewModel(app) {
    private val core = (app as PochetteApp).core
    private val sessions = SessionStore(app)

    private val _stage = MutableStateFlow<Stage>(Stage.Starting)
    val stage: StateFlow<Stage> = _stage.asStateFlow()

    private val _library = MutableStateFlow(Library())
    val library: StateFlow<Library> = _library.asStateFlow()

    val playback = Playback(app)

    /** Something that went wrong, to show once. */
    private val _notice = MutableStateFlow<String?>(null)
    val notice: StateFlow<String?> = _notice.asStateFlow()

    /** What's open, the last on top. */
    val screens = mutableStateListOf<Screen>(Screen.Albums)

    /** Album pages already loaded, as going back to one is common. */
    private val pages = HashMap<String, AlbumPage>()

    private var view = LibraryView("", emptyList(), emptyList(), Sort.DATE_ADDED, false)
    private var signingIn: Job? = null

    init {
        resume()
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

    fun open(screen: Screen) {
        if (screens.last().key != screen.key) screens.add(screen)
    }

    /** Close the screen on top: its key, to forget its state. */
    fun back(): String? = if (screens.size > 1) screens.removeAt(screens.lastIndex).key else null

    /** The album's page, from Qobuz the first time. Signs out if Qobuz no longer takes the session. */
    suspend fun album(id: String): AlbumPage =
        pages[id] ?: try {
            core.album(id).also { pages[id] = it }
        } catch (expired: PochetteException.SignedOut) {
            signOut()
            throw expired
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
                _library.value = _library.value.copy(albums = albums())
            } catch (err: PochetteException) {
                failed()
                val change = if (favorite) "add “${album.title}” to" else "take “${album.title}” out of"
                _notice.value = "Can't $change your favorites: ${err.message}"
            }
        }
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
        screens.removeRange(1, screens.size)
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
                    core.refreshLibrary()
                    null
                } catch (expired: PochetteException.SignedOut) {
                    signOut()
                    return@launch
                } catch (err: PochetteException) {
                    err.message
                }
            _library.value = Library(albums = albums(), error = error)
        }
    }

    private suspend fun signedIn(account: Account) {
        core.session()?.let(sessions::save)
        _stage.value = Stage.SignedIn(account)
        refresh()
    }

    private suspend fun albums(): List<AlbumCard> = withContext(Dispatchers.Default) { core.library(view) }

    companion object {
        /** Opens the app: the browser goes there once signed in. See the manifest. */
        const val SIGNED_IN = "io.github.mcrampon.pochette://signed-in"
    }
}
