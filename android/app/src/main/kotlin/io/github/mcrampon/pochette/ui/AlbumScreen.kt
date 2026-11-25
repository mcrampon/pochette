package io.github.mcrampon.pochette.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
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
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.mcrampon.pochette.R
import io.github.mcrampon.pochette.core.AlbumCard
import io.github.mcrampon.pochette.core.AlbumPage
import io.github.mcrampon.pochette.core.PochetteException
import io.github.mcrampon.pochette.core.Plays
import io.github.mcrampon.pochette.core.TrackRow
import io.github.mcrampon.pochette.core.ago
import io.github.mcrampon.pochette.core.playCount
import io.github.mcrampon.pochette.core.trackDuration
import io.github.mcrampon.pochette.playback.NowPlaying

/** What can be asked of the album page. */
class AlbumActions(
    val load: suspend () -> AlbumPage,
    val plays: () -> Plays,
    val isFavorite: () -> Boolean,
    val setFavorite: (AlbumCard, Boolean, failed: () -> Unit) -> Unit,
    val play: (AlbumPage, from: ULong?) -> Unit,
    val playPause: () -> Unit,
    val back: () -> Unit,
)

private sealed interface Load {
    data object Pending : Load

    data class Done(val page: AlbumPage) : Load

    data class Failed(val error: String) : Load
}

/** An album: what it is, its tracks to play from, and Qobuz's review. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AlbumScreen(id: String, preview: AlbumCard?, now: NowPlaying?, actions: AlbumActions) {
    var attempt by remember { mutableIntStateOf(0) }
    val load by produceState<Load>(Load.Pending, id, attempt) {
        value =
            try {
                Load.Done(actions.load())
            } catch (err: PochetteException) {
                Load.Failed(err.message.orEmpty())
            }
    }
    val page = (load as? Load.Done)?.page
    val card = page?.card ?: preview
    var favorite by rememberSaveable(id) { mutableStateOf(actions.isFavorite()) }
    val ours = now?.albumId == id

    Scaffold(
        topBar = {
            TopAppBar(
                title = {},
                navigationIcon = {
                    IconButton(onClick = actions.back) {
                        Icon(painterResource(R.drawable.ic_arrow_back), contentDescription = "Back")
                    }
                },
                actions = {
                    if (card != null) {
                        IconButton(onClick = {
                            favorite = !favorite
                            actions.setFavorite(card, favorite) { favorite = !favorite }
                        }) {
                            Icon(
                                painterResource(if (favorite) R.drawable.ic_favorite else R.drawable.ic_favorite_border),
                                contentDescription = if (favorite) "Remove from Favorites" else "Add to Favorites",
                                tint = if (favorite) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                },
            )
        },
        contentWindowInsets = WindowInsets(0),
    ) { padding ->
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(bottom = 32.dp)) {
            item {
                Header(card, page, ours, now, actions)
            }
            when (val current = load) {
                Load.Pending ->
                    item {
                        Box(Modifier.fillMaxWidth().padding(32.dp), contentAlignment = Alignment.Center) {
                            CircularProgressIndicator()
                        }
                    }
                is Load.Failed ->
                    item {
                        Column(
                            Modifier.fillMaxWidth().padding(32.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            Text(
                                "Can't load the tracks: ${current.error}",
                                textAlign = TextAlign.Center,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            TextButton(onClick = { attempt++ }) { Text("Try Again") }
                        }
                    }
                is Load.Done -> {
                    tracks(current.page, playing = now?.trackId.takeIf { ours }, onPlay = { actions.play(current.page, it) })
                    about(current.page)
                }
            }
        }
    }
}

@Composable
private fun Header(card: AlbumCard?, page: AlbumPage?, ours: Boolean, now: NowPlaying?, actions: AlbumActions) {
    // Read again when a track ends, as it may have made a play.
    val plays = remember(now?.trackId, page) { actions.plays() }
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            // Not taller than most of the window, as in landscape.
            val most = with(LocalDensity.current) { (LocalWindowInfo.current.containerSize.height * 0.6f).toDp() }
            Cover(card?.coverLarge ?: card?.cover, Modifier.widthIn(max = minOf(360.dp, most)).fillMaxWidth(), corner = 12)
        }
        Spacer(Modifier.height(16.dp))
        val kicker = listOfNotNull(page?.kind ?: "Album", card?.year).joinToString(" · ").uppercase()
        Text(kicker, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
        Text(card?.title.orEmpty(), style = MaterialTheme.typography.headlineSmall)
        Text(card?.artist.orEmpty(), style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(4.dp))
        val details = listOfNotNull(card?.genre, page?.label).joinToString(" · ")
        if (details.isNotEmpty()) Dim(details)
        if (page != null) Dim("${page.trackCount} · ${page.length}")
        plays.last?.let { Dim("${playCount(plays.count)} · last played ${ago(it)}") }
        val quality = card?.hires?.let { "Hi-Res · $it" } ?: page?.quality
        if (quality != null) {
            Surface(
                color = MaterialTheme.colorScheme.secondaryContainer,
                shape = RoundedCornerShape(6.dp),
                modifier = Modifier.padding(top = 6.dp),
            ) {
                Text(
                    quality,
                    style = MaterialTheme.typography.labelMedium,
                    modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp),
                )
            }
        }
        val playing = ours && now?.playing == true
        Button(
            onClick = { if (ours) actions.playPause() else page?.let { actions.play(it, null) } },
            enabled = page?.tracks?.any { it.streamable } == true,
            modifier = Modifier.padding(top = 12.dp),
        ) {
            Icon(
                painterResource(if (playing) R.drawable.ic_pause else R.drawable.ic_play),
                contentDescription = null,
                modifier = Modifier.size(20.dp),
            )
            Spacer(Modifier.width(8.dp))
            Text(if (playing) "Pause" else "Play")
        }
        if (ours && now?.error != null) {
            Text(now.error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
        }
        Spacer(Modifier.height(16.dp))
    }
}

@Composable
private fun Dim(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

/** The tracks, under a heading per disc when there are several, and per work for classical pieces. */
private fun LazyListScope.tracks(page: AlbumPage, playing: ULong?, onPlay: (ULong) -> Unit) {
    var disc = 0u
    var work: String? = null
    for (track in page.tracks) {
        if (page.discs > 1u && track.disc != disc) {
            // Not `disc`, which the loop changes before the heading shows.
            val number = track.disc
            disc = number
            work = null
            item(key = "disc/$number") { Heading("Disc $number") }
        }
        val trackWork = track.work
        if (trackWork != null && trackWork != work) {
            item(key = "work/${track.id}") {
                Text(
                    trackWork,
                    style = MaterialTheme.typography.bodyMedium,
                    fontStyle = FontStyle.Italic,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 4.dp),
                )
            }
        }
        work = trackWork
        item(key = track.id.toString()) {
            Track(track, page.card.artist, playing = track.id == playing, onClick = { onPlay(track.id) })
        }
    }
}

@Composable
private fun Track(track: TrackRow, albumArtist: String, playing: Boolean, onClick: () -> Unit) {
    val color = if (playing) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface
    Row(
        Modifier
            .fillMaxWidth()
            .clickable(enabled = track.streamable, onClick = onClick)
            .alpha(if (track.streamable) 1f else 0.4f)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(Modifier.width(28.dp)) {
            if (playing) {
                Icon(painterResource(R.drawable.ic_graphic_eq), contentDescription = "Playing", tint = color, modifier = Modifier.size(18.dp))
            } else {
                Text(track.number.toString(), color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium)
            }
        }
        Column(Modifier.weight(1f)) {
            Text(track.title, color = color, style = MaterialTheme.typography.bodyLarge, maxLines = 2, overflow = TextOverflow.Ellipsis)
            // The performer, when it isn't the album's artist, as on compilations.
            track.performer?.takeIf { it != albumArtist }?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        if (track.explicit) {
            Surface(color = MaterialTheme.colorScheme.surfaceVariant, shape = RoundedCornerShape(3.dp)) {
                Text("E", style = MaterialTheme.typography.labelSmall, modifier = Modifier.padding(horizontal = 4.dp))
            }
        }
        Text(trackDuration(track.duration), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

private fun LazyListScope.about(page: AlbumPage) {
    page.description?.let { review ->
        item(key = "about") {
            Column(Modifier.padding(horizontal = 16.dp).padding(top = 24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("About This Album", style = MaterialTheme.typography.titleMedium)
                Text(review, style = MaterialTheme.typography.bodyMedium)
            }
        }
    }
    page.copyright?.let { copyright ->
        item(key = "copyright") {
            Text(
                copyright,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp).padding(top = 16.dp),
            )
        }
    }
}

@Composable
private fun Heading(text: String) {
    Text(text, style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp))
}
