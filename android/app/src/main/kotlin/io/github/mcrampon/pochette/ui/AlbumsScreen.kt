package io.github.mcrampon.pochette.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import io.github.mcrampon.pochette.Library
import io.github.mcrampon.pochette.core.AlbumCard

/** The user's albums, as a grid of covers: tap one to open it. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AlbumsScreen(library: Library, onRefresh: () -> Unit, onSignOut: () -> Unit, onOpen: (AlbumCard) -> Unit) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Albums") },
                actions = { TextButton(onClick = onSignOut) { Text("Sign Out") } },
            )
        },
        // The frame around the screens keeps clear of the navigation bar.
        contentWindowInsets = WindowInsets(0),
    ) { padding ->
        PullToRefreshBox(
            isRefreshing = library.refreshing,
            onRefresh = onRefresh,
            modifier = Modifier.fillMaxSize().padding(padding),
        ) {
            if (library.albums.isEmpty() && !library.refreshing) {
                Box(Modifier.fillMaxSize().padding(32.dp), contentAlignment = Alignment.Center) {
                    Text(library.error ?: "No albums yet: add some to your favorites on Qobuz.")
                }
            } else {
                LazyVerticalGrid(
                    columns = GridCells.Adaptive(150.dp),
                    contentPadding = PaddingValues(16.dp),
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                    verticalArrangement = Arrangement.spacedBy(20.dp),
                ) {
                    items(library.albums, key = { it.id }) { album -> AlbumTile(album, onClick = { onOpen(album) }) }
                }
            }
        }
    }
}


/** An album's cover, square, with a placeholder while it loads. */
@Composable
fun Cover(url: Any?, modifier: Modifier = Modifier, corner: Int = 8) {
    AsyncImage(
        model = url,
        contentDescription = null,
        contentScale = ContentScale.Crop,
        modifier =
            modifier
                .aspectRatio(1f)
                .clip(RoundedCornerShape(corner.dp))
                .background(MaterialTheme.colorScheme.surfaceVariant),
    )
}
