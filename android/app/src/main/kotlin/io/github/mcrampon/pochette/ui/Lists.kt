package io.github.mcrampon.pochette.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.layout
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import io.github.mcrampon.pochette.Paged
import io.github.mcrampon.pochette.core.AlbumCard
import io.github.mcrampon.pochette.core.ArtistCard

/** What a tile can say under the artist, such as the album's awards. */
typealias Caption = (AlbumCard) -> String?

/** The awards of `album`, as a caption: "4F de Télérama · Pitchfork: Best New Music". */
val awards: Caption = { album -> album.awards.takeIf { it.isNotEmpty() }?.joinToString(" · ") }

/** An album as grids and rows show it: its cover, title and artist, and `caption` under them. */
@Composable
fun AlbumTile(album: AlbumCard, onClick: () -> Unit, modifier: Modifier = Modifier, caption: String? = null) {
    Column(modifier.clip(RoundedCornerShape(8.dp)).clickable(onClick = onClick)) {
        Cover(album.coverLarge ?: album.cover, Modifier.fillMaxWidth())
        Text(
            album.title,
            style = MaterialTheme.typography.titleSmall,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 8.dp),
        )
        Text(
            album.artist,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        caption?.let {
            Text(
                it,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.primary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 2.dp),
            )
        }
    }
}

/** A heading over a row or a grid, with "See All" when there's more. */
@Composable
fun Heading(title: String, onSeeAll: (() -> Unit)? = null) {
    Row(
        Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 20.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
        onSeeAll?.let { TextButton(onClick = it) { Text("See All") } }
    }
}

/** Albums in `rows` rows that scroll sideways together, filled column by column. */
@Composable
fun AlbumShelf(albums: List<AlbumCard>, onOpen: (AlbumCard) -> Unit, rows: Int = 1, caption: Caption = { null }) {
    // A tile without a caption still keeps its line, so the rows stay aligned.
    val captioned = albums.any { caption(it) != null }
    LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        items(albums.chunked(rows), key = { it.first().id }) { column ->
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                for (album in column) {
                    AlbumTile(
                        album,
                        onClick = { onOpen(album) },
                        Modifier.width(140.dp),
                        caption = if (captioned) caption(album).orEmpty() else null,
                    )
                }
            }
        }
    }
}

/** Artists in a row that scrolls sideways, as round portraits. */
@Composable
fun ArtistShelf(artists: List<ArtistCard>, onOpen: (ArtistCard) -> Unit) {
    LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        items(artists, key = { it.id.toString() }) { artist ->
            Column(
                Modifier.width(96.dp).clip(RoundedCornerShape(8.dp)).clickable { onOpen(artist) }.padding(vertical = 4.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Portrait(artist.portrait, artist.name, 88.dp)
                Text(
                    artist.name,
                    style = MaterialTheme.typography.bodySmall,
                    textAlign = TextAlign.Center,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(top = 6.dp),
                )
            }
        }
    }
}

/** An artist's portrait, round, or their initial when Qobuz has none. */
@Composable
fun Portrait(url: String?, name: String, size: Dp) {
    Box(
        Modifier.size(size).clip(CircleShape).background(MaterialTheme.colorScheme.secondaryContainer),
        contentAlignment = Alignment.Center,
    ) {
        if (url == null) {
            Text(
                name.firstOrNull()?.uppercase().orEmpty(),
                style = if (size > 64.dp) MaterialTheme.typography.headlineMedium else MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSecondaryContainer,
            )
        } else {
            AsyncImage(model = url, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
        }
    }
}

/** A grid of albums that loads more as it's scrolled, under `header`. */
@Composable
fun PagedGrid(
    paged: Paged,
    onOpen: (AlbumCard) -> Unit,
    modifier: Modifier = Modifier,
    empty: String,
    caption: Caption = { null },
    header: LazyGridScope.() -> Unit = {},
) {
    val grid = rememberLazyGridState()
    // The next page, when the last ones come into view.
    val nearEnd by remember { derivedStateOf { (grid.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0) >= grid.layoutInfo.totalItemsCount - 8 } }
    LaunchedEffect(nearEnd, paged.albums.size) { if (nearEnd) paged.more() }
    LazyVerticalGrid(
        columns = GridCells.Adaptive(150.dp),
        state = grid,
        modifier = modifier,
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        header()
        items(paged.albums, key = { it.id }) { album -> AlbumTile(album, onClick = { onOpen(album) }, caption = caption(album)) }
        item(span = { GridItemSpan(maxLineSpan) }) {
            Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) {
                val error = paged.error
                when {
                    paged.loading -> CircularProgressIndicator()
                    error != null ->
                        Column(horizontalAlignment = Alignment.CenterHorizontally) {
                            Text(error, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
                            TextButton(onClick = paged::more) { Text("Try Again") }
                        }
                    paged.albums.isEmpty() -> Text(empty, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
}

/** Wider by `margin` on both sides than the grid's padding allows: for rows that scroll sideways. */
fun Modifier.fullBleed(margin: Dp): Modifier =
    layout { measurable, constraints ->
        val extra = margin.roundToPx() * 2
        val placeable = measurable.measure(constraints.copy(minWidth = constraints.minWidth + extra, maxWidth = constraints.maxWidth + extra))
        layout(constraints.maxWidth, placeable.height) { placeable.place(-extra / 2, 0) }
    }

/** A message in the middle of a screen, with a way to try again. */
@Composable
fun Failed(message: String, onRetry: () -> Unit, modifier: Modifier = Modifier) {
    Column(
        modifier.fillMaxWidth().padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(message, textAlign = TextAlign.Center, color = MaterialTheme.colorScheme.onSurfaceVariant)
        TextButton(onClick = onRetry) { Text("Try Again") }
    }
}

/** A spinner in the middle of what's loading. */
@Composable
fun Loading(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().padding(32.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
}
