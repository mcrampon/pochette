package io.github.mcrampon.pochette.ui

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.mcrampon.pochette.Load
import io.github.mcrampon.pochette.R
import io.github.mcrampon.pochette.core.AlbumCard
import io.github.mcrampon.pochette.core.ArtistCard
import io.github.mcrampon.pochette.core.ArtistPage
import io.github.mcrampon.pochette.core.PochetteException
import io.github.mcrampon.pochette.core.ReleaseKind
import androidx.annotation.StringRes
import androidx.compose.ui.res.stringResource

/** What can be asked of an artist's page. */
class ArtistActions(
    val load: suspend () -> ArtistPage,
    val openAlbum: (AlbumCard) -> Unit,
    val openArtist: (ArtistCard) -> Unit,
    val seeAll: (ReleaseKind) -> Unit,
    val back: () -> Unit,
)

/** An artist: their releases by kind, similar artists, and their biography. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ArtistScreen(name: String, actions: ArtistActions) {
    var attempt by remember { mutableIntStateOf(0) }
    val load by produceState<Load<ArtistPage>>(Load.Pending, attempt) {
        value =
            try {
                Load.Done(actions.load())
            } catch (err: PochetteException) {
                Load.Failed(err.message.orEmpty())
            }
    }
    Scaffold(
        topBar = {
            TopAppBar(
                title = {},
                navigationIcon = {
                    IconButton(onClick = actions.back) {
                        Icon(painterResource(R.drawable.ic_arrow_back), contentDescription = stringResource(R.string.back))
                    }
                },
            )
        },
        contentWindowInsets = WindowInsets(0),
    ) { padding ->
        val page = (load as? Load.Done)?.value
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(bottom = 24.dp)) {
            item {
                Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Portrait(page?.portrait, name, 160.dp)
                    Text(
                        stringResource(R.string.artist_kicker),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(top = 16.dp),
                    )
                    Text(page?.name ?: name, style = MaterialTheme.typography.headlineSmall, textAlign = TextAlign.Center)
                }
            }
            when (val current = load) {
                Load.Pending -> item { Loading() }
                is Load.Failed -> item { Failed(stringResource(R.string.cant_load_artist, current.error), onRetry = { attempt++ }) }
                is Load.Done -> {
                    val artist = current.value
                    if (artist.releases.none { it.albums.albums.isNotEmpty() }) {
                        item { Failed(stringResource(R.string.no_albums_from_artist), onRetry = { attempt++ }) }
                    }
                    for (releases in artist.releases.filter { it.albums.albums.isNotEmpty() }) {
                        item(key = releases.kind.name) {
                            Column {
                                Heading(stringResource(kindTitle(releases.kind)), onSeeAll = { actions.seeAll(releases.kind) }.takeIf { releases.albums.hasMore })
                                AlbumShelf(releases.albums.albums, onOpen = actions.openAlbum)
                            }
                        }
                    }
                    if (artist.similar.isNotEmpty()) {
                        item(key = "similar") {
                            Column {
                                Heading(stringResource(R.string.similar_artists))
                                ArtistShelf(artist.similar, onOpen = actions.openArtist)
                            }
                        }
                    }
                    artist.biography?.let { biography -> item(key = "about") { Biography(biography) } }
                }
            }
        }
    }
}

/** The biography, its first lines until asked for more. */
@Composable
private fun Biography(text: String) {
    var whole by rememberSaveable { mutableStateOf(false) }
    Column(Modifier.padding(horizontal = 16.dp).padding(top = 20.dp).animateContentSize(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(stringResource(R.string.about_artist), style = MaterialTheme.typography.titleMedium)
        Text(
            text,
            style = MaterialTheme.typography.bodyMedium,
            maxLines = if (whole) Int.MAX_VALUE else 6,
            overflow = TextOverflow.Ellipsis,
        )
        if (!whole) TextButton(onClick = { whole = true }) { Text(stringResource(R.string.read_more)) }
    }
}

@StringRes
fun kindTitle(kind: ReleaseKind) =
    when (kind) {
        ReleaseKind.ALBUM -> R.string.releases_albums
        ReleaseKind.EP_SINGLE -> R.string.releases_eps_singles
        ReleaseKind.LIVE -> R.string.releases_live
        ReleaseKind.COMPILATION -> R.string.releases_compilations
        ReleaseKind.OTHER -> R.string.releases_other
    }
