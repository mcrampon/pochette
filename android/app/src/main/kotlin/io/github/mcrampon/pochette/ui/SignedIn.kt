package io.github.mcrampon.pochette.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.mcrampon.pochette.AppModel
import io.github.mcrampon.pochette.Screen
import io.github.mcrampon.pochette.Tab
import io.github.mcrampon.pochette.core.AlbumCard
import io.github.mcrampon.pochette.core.ArtistCard
import io.github.mcrampon.pochette.playback.NowPlaying

/** The signed-in app: the tabs, the screen on top of the shown one, what's playing, and messages. */
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
    val navigation = model.navigation
    // Each screen keeps its scroll while others are above it.
    val saved = rememberSaveableStateHolder()
    val back = { model.back()?.let(saved::removeState) }
    BackHandler(enabled = navigation.canGoBack) { back() }
    val fullScreen = navigation.top == Screen.NowPlaying

    Scaffold(
        bottomBar = {
            if (!fullScreen) {
                Column {
                    now?.let { playing ->
                        MiniPlayer(
                            playing,
                            onClick = { model.open(Screen.NowPlaying) },
                            onPlayPause = model.playback::playPause,
                            onNext = model.playback::next,
                        )
                    }
                    NavigationBar {
                        for (tab in Tab.entries) {
                            NavigationBarItem(
                                selected = tab == navigation.tab,
                                onClick = { navigation.select(tab).forEach(saved::removeState) },
                                icon = { Icon(painterResource(tab.icon), contentDescription = null) },
                                label = { Text(tab.label) },
                            )
                        }
                    }
                }
            } else {
                Spacer(Modifier.navigationBarsPadding())
            }
        },
        snackbarHost = { SnackbarHost(snackbars) },
        // The screens' own bars keep clear of the status bar; the bottom bar of the navigation bar.
        contentWindowInsets = WindowInsets(0),
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding)) {
            saved.SaveableStateProvider(navigation.topKey) {
                Screen(model, navigation.top, now, back = { back() })
            }
        }
    }
}

@androidx.annotation.OptIn(androidx.media3.common.util.UnstableApi::class)
@Composable
private fun Screen(model: AppModel, screen: Screen, now: NowPlaying?, back: () -> Unit) {
    val openAlbum = { album: AlbumCard -> model.open(Screen.Album(album.id, album)) }
    val openArtist = { artist: ArtistCard -> model.open(Screen.Artist(artist.id, artist.name)) }
    when (screen) {
        Screen.Albums -> {
            val library by model.library.collectAsStateWithLifecycle()
            AlbumsScreen(
                library,
                model.view,
                AlbumsActions(
                    refresh = model::refresh,
                    show = model::show,
                    genres = model::libraryGenres,
                    decades = model::libraryDecades,
                    playRandom = model::playRandom,
                    signOut = model::signOut,
                    open = openAlbum,
                ),
            )
        }
        Screen.Discover ->
            DiscoverScreen(
                model.discover,
                DiscoverActions(
                    load = model::loadDiscover,
                    genres = model::genres,
                    picked = { model.settings.discoverGenres },
                    pick = model::pickGenres,
                    seeAll = { model.open(Screen.Section(it, model.settings.discoverGenres)) },
                    open = openAlbum,
                ),
            )
        Screen.Search -> SearchScreen(model.found, onSearch = model::search, onOpenAlbum = openAlbum, onOpenArtist = openArtist)
        Screen.Stats ->
            StatsScreen(
                model.statsPeriod,
                // A play is recorded as its album changes track, or stops.
                playing = now?.trackId,
                StatsActions(
                    load = model::stats,
                    pick = { model.statsPeriod = it },
                    openAlbum = openAlbum,
                    openArtist = { model.open(Screen.Artist(it.id, it.name)) },
                ),
            )
        Screen.NowPlaying -> {
            val playing = now
            if (playing == null) {
                // Stopped: nothing to show.
                LaunchedEffect(Unit) { back() }
            } else {
                NowPlayingScreen(
                    playing,
                    NowPlayingActions(
                        position = { model.playback.positionMs },
                        playPause = model.playback::playPause,
                        previous = model.playback::previous,
                        next = model.playback::next,
                        seekTo = model.playback::seekTo,
                        skipTo = model.playback::skipTo,
                        lyrics = model::lyrics,
                        quality = { model.settings.quality },
                        setQuality = { model.settings.quality = it },
                        openAlbum = { model.open(Screen.Album(playing.albumId, model.card(playing.albumId))) },
                        back = back,
                    ),
                )
            }
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
                    openArtist = { id, name -> model.open(Screen.Artist(id, name)) },
                    back = back,
                ),
            )
        is Screen.Artist ->
            ArtistScreen(
                screen.name,
                ArtistActions(
                    load = { model.artist(screen.id) },
                    openAlbum = openAlbum,
                    openArtist = openArtist,
                    seeAll = { model.open(Screen.Releases(screen.id, screen.name, it)) },
                    back = back,
                ),
            )
        is Screen.Section -> ListScreen(title(screen.section), null, model.list(screen), openAlbum, back, caption(screen.section))
        is Screen.Releases -> ListScreen(kindTitle(screen.kind), screen.artistName, model.list(screen), openAlbum, back)
    }
}
