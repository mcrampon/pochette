package io.github.mcrampon.pochette.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.mcrampon.pochette.AppModel
import io.github.mcrampon.pochette.Screen

/** The signed-in app: the screen on top of the stack, what's playing under it, and messages. */
@androidx.annotation.OptIn(androidx.media3.common.util.UnstableApi::class)
@Composable
fun SignedIn(model: AppModel) {
    val now by model.playback.now.collectAsStateWithLifecycle()
    val notice by model.notice.collectAsStateWithLifecycle()
    val snackbars = remember { SnackbarHostState() }
    LaunchedEffect(notice) {
        notice?.let {
            snackbars.showSnackbar(it)
            model.noticeShown()
        }
    }
    // Each screen keeps its scroll while others are above it.
    val saved = rememberSaveableStateHolder()
    val back = {
        val key = "${model.screens.size}/${model.screens.last().key}"
        if (model.back() != null) saved.removeState(key)
    }
    BackHandler(enabled = model.screens.size > 1) { back() }

    Scaffold(
        bottomBar = {
            val playing = now
            if (playing != null) {
                MiniPlayer(
                    playing,
                    onClick = { model.open(Screen.Album(playing.albumId, model.card(playing.albumId))) },
                    onPlayPause = model.playback::playPause,
                    onNext = model.playback::next,
                )
            } else {
                Spacer(Modifier.navigationBarsPadding())
            }
        },
        snackbarHost = { SnackbarHost(snackbars) },
        // The screens' own bars keep clear of the status bar; the bottom bar of the navigation bar.
        contentWindowInsets = WindowInsets(0),
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding)) {
            val screen = model.screens.last()
            saved.SaveableStateProvider("${model.screens.size}/${screen.key}") {
                when (screen) {
                    Screen.Albums -> {
                        val library by model.library.collectAsStateWithLifecycle()
                        AlbumsScreen(
                            library,
                            onRefresh = model::refresh,
                            onSignOut = model::signOut,
                            onOpen = { model.open(Screen.Album(it.id, it)) },
                        )
                    }
                    is Screen.Album ->
                        AlbumScreen(
                            screen.id,
                            screen.preview,
                            now,
                            AlbumActions(
                                load = { model.album(screen.id) },
                                plays = { model.plays(screen.id) },
                                isFavorite = { model.isFavorite(screen.id) },
                                setFavorite = model::setFavorite,
                                play = model.playback::play,
                                playPause = model.playback::playPause,
                                back = { back() },
                            ),
                        )
                }
            }
        }
    }
}
