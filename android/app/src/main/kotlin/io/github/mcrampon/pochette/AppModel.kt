package io.github.mcrampon.pochette

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import io.github.mcrampon.pochette.core.Account
import io.github.mcrampon.pochette.core.AlbumCard
import io.github.mcrampon.pochette.core.LibraryView
import io.github.mcrampon.pochette.core.PochetteException
import io.github.mcrampon.pochette.core.Sort
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

/** The user's albums, as the library shows them. */
data class Library(
    val albums: List<AlbumCard> = emptyList(),
    val refreshing: Boolean = false,
    val error: String? = null,
)

/** What the screens show, and what they ask of the core. */
class AppModel(app: Application) : AndroidViewModel(app) {
    private val core = (app as PochetteApp).core
    private val sessions = SessionStore(app)

    private val _stage = MutableStateFlow<Stage>(Stage.Starting)
    val stage: StateFlow<Stage> = _stage.asStateFlow()

    private val _library = MutableStateFlow(Library())
    val library: StateFlow<Library> = _library.asStateFlow()

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
                    val signIn = core.beginSignIn()
                    _stage.value = Stage.SignedOut(url = signIn.url())
                    open(signIn.url())
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

    fun signOut() {
        core.signOut()
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
}
