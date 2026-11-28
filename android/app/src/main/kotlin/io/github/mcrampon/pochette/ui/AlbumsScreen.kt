package io.github.mcrampon.pochette.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import io.github.mcrampon.pochette.Library
import io.github.mcrampon.pochette.Load
import io.github.mcrampon.pochette.R
import io.github.mcrampon.pochette.core.AlbumCard
import io.github.mcrampon.pochette.core.LibraryView
import io.github.mcrampon.pochette.core.Sort
import io.github.mcrampon.pochette.core.orderLabel
import io.github.mcrampon.pochette.core.sortLabel
import android.content.res.Resources
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource

/** What can be asked of the library. */
class AlbumsActions(
    val refresh: () -> Unit,
    val show: (LibraryView) -> Unit,
    val genres: () -> List<String>,
    val decades: () -> List<UShort>,
    val playRandom: () -> Unit,
    val settings: () -> Unit,
    val signOut: () -> Unit,
    val open: (AlbumCard) -> Unit,
)

/**
 * The user's albums, as a grid of covers, searched, filtered and sorted: tap one to open it.
 * `offline`, out of Qobuz's reach: the albums downloaded, which `anyDownloaded` says there are.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AlbumsScreen(library: Library, view: LibraryView, offline: Boolean, anyDownloaded: Boolean, actions: AlbumsActions) {
    val filtered = view.query.isNotBlank() || view.genres.isNotEmpty() || view.decades.isNotEmpty() || (view.downloadedOnly && !offline)
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.tab_albums)) },
                actions = {
                    if (library.albums.isNotEmpty()) {
                        IconButton(onClick = actions.playRandom) {
                            Icon(
                                painterResource(R.drawable.ic_shuffle),
                                contentDescription = stringResource(if (filtered) R.string.play_random_album_filtered else R.string.play_random_album),
                            )
                        }
                    }
                    Menu(actions)
                },
            )
        },
        // The frame around the screens keeps clear of the navigation bar.
        contentWindowInsets = WindowInsets(0),
    ) { padding ->
        PullToRefreshBox(
            isRefreshing = library.refreshing,
            onRefresh = actions.refresh,
            modifier = Modifier.fillMaxSize().padding(padding),
        ) {
            if (!library.any && !library.refreshing) {
                Box(Modifier.fillMaxSize().padding(32.dp), contentAlignment = Alignment.Center) {
                    Text(
                        library.error ?: stringResource(R.string.no_albums_yet),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                    )
                }
                return@PullToRefreshBox
            }
            LazyVerticalGrid(
                columns = GridCells.Adaptive(150.dp),
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(16.dp),
                verticalArrangement = Arrangement.spacedBy(20.dp),
            ) {
                // The search and filters scroll away with the albums, leaving the screen to them.
                item(key = "controls", span = { GridItemSpan(maxLineSpan) }) {
                    Column(Modifier.fullBleed(16.dp)) {
                        if (offline) OfflineNote(onRetry = actions.refresh)
                        Controls(view, showDownloaded = anyDownloaded && !offline, actions)
                    }
                }
                items(library.albums, key = { it.id }) { album -> AlbumTile(album, onClick = { actions.open(album) }) }
                if (library.albums.isEmpty() && filtered) {
                    item(key = "none", span = { GridItemSpan(maxLineSpan) }) {
                        Column(Modifier.fillMaxWidth().padding(32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                            Text(stringResource(R.string.no_matching_albums), style = MaterialTheme.typography.titleMedium)
                            TextButton(
                                onClick = { actions.show(view.copy(query = "", genres = emptyList(), decades = emptyList(), downloadedOnly = false)) },
                                modifier = Modifier.padding(top = 8.dp),
                            ) { Text(stringResource(R.string.clear_search_and_filters)) }
                        }
                    }
                }
            }
        }
    }
}

/** What's not about the albums: the settings, signing out. */
@Composable
private fun Menu(actions: AlbumsActions) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) { Icon(painterResource(R.drawable.ic_more_vert), contentDescription = stringResource(R.string.more)) }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            DropdownMenuItem(text = { Text(stringResource(R.string.settings)) }, onClick = {
                open = false
                actions.settings()
            })
            DropdownMenuItem(text = { Text(stringResource(R.string.sign_out)) }, onClick = {
                open = false
                actions.signOut()
            })
        }
    }
}

/** Out of Qobuz's reach: the downloads are what shows. */
@Composable
private fun OfflineNote(onRetry: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            stringResource(R.string.offline_downloads),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f),
        )
        TextButton(onClick = onRetry) { Text(stringResource(R.string.try_again)) }
    }
}

/** The search, the genre, decade and download filters, the latter if `showDownloaded`, and the sort. */
@Composable
private fun Controls(view: LibraryView, showDownloaded: Boolean, actions: AlbumsActions) {
    val focus = LocalFocusManager.current
    val resources = LocalResources.current
    var picking by remember { mutableStateOf<Filter?>(null) }
    Column(Modifier.padding(top = 4.dp)) {
        OutlinedTextField(
            value = view.query,
            onValueChange = { actions.show(view.copy(query = it)) },
            placeholder = { Text(stringResource(R.string.search_library), maxLines = 1, overflow = TextOverflow.Ellipsis) },
            leadingIcon = { Icon(painterResource(R.drawable.ic_search), contentDescription = null) },
            trailingIcon = {
                if (view.query.isNotEmpty()) {
                    IconButton(onClick = { actions.show(view.copy(query = "")) }) {
                        Icon(painterResource(R.drawable.ic_close), contentDescription = stringResource(R.string.clear))
                    }
                }
            },
            singleLine = true,
            shape = RoundedCornerShape(28.dp),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = { focus.clearFocus() }),
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
        )
        Row(
            Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Chip(
                label =
                    when (view.genres.size) {
                        0 -> stringResource(R.string.genres)
                        1 -> view.genres.first()
                        else -> pluralStringResource(R.plurals.genres_picked, view.genres.size, view.genres.size)
                    },
                on = view.genres.isNotEmpty(),
                onClick = { picking = Filter.Genres },
            )
            Chip(
                label =
                    when (view.decades.size) {
                        0 -> stringResource(R.string.decades)
                        1 -> decade(resources, view.decades.first())
                        else -> pluralStringResource(R.plurals.decades_picked, view.decades.size, view.decades.size)
                    },
                on = view.decades.isNotEmpty(),
                onClick = { picking = Filter.Decades },
            )
            if (showDownloaded || view.downloadedOnly) {
                // On or off: no menu.
                FilterChip(
                    selected = view.downloadedOnly,
                    onClick = { actions.show(view.copy(downloadedOnly = !view.downloadedOnly)) },
                    label = { Text(stringResource(R.string.downloaded), maxLines = 1) },
                    leadingIcon = {
                        Icon(painterResource(R.drawable.ic_download_done), contentDescription = null, Modifier.size(FilterChipDefaults.IconSize))
                    },
                )
            }
            SortChip(view, actions.show)
        }
    }
    when (picking) {
        Filter.Genres ->
            ChecklistDialog(
                title = stringResource(R.string.genres),
                options = remember { Load.Done(actions.genres().map { it to it }) },
                picked = view.genres,
                all = stringResource(R.string.all_genres),
                onPick = { actions.show(view.copy(genres = it.sorted())) },
                onDismiss = { picking = null },
            )
        Filter.Decades ->
            ChecklistDialog(
                title = stringResource(R.string.decades),
                options = remember { Load.Done(actions.decades().map { it to decade(resources, it) }) },
                picked = view.decades,
                all = stringResource(R.string.all_decades),
                onPick = { actions.show(view.copy(decades = it.sorted())) },
                onDismiss = { picking = null },
            )
        null -> {}
    }
}

private enum class Filter { Genres, Decades }

private fun decade(resources: Resources, decade: UShort) = resources.getString(R.string.decade, decade.toInt())

@Composable
private fun Chip(label: String, on: Boolean, onClick: () -> Unit) {
    FilterChip(
        selected = on,
        onClick = onClick,
        label = { Text(label, maxLines = 1) },
        trailingIcon = { Icon(painterResource(R.drawable.ic_expand_more), contentDescription = null, Modifier.size(FilterChipDefaults.IconSize)) },
    )
}

/** The sort, and its order: "Date Added · Newest First". */
@Composable
private fun SortChip(view: LibraryView, show: (LibraryView) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        Chip("${sortLabel(view.sort)} · ${orderLabel(view)}", on = false, onClick = { open = true })
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            for (sort in Sort.entries) {
                DropdownMenuItem(
                    text = { Text(sortLabel(sort)) },
                    leadingIcon = { RadioButton(selected = sort == view.sort, onClick = null) },
                    onClick = {
                        open = false
                        // Each sort starts the way it reads best: newest or most played first, A to Z.
                        show(view.copy(sort = sort, reversed = false))
                    },
                )
            }
            HorizontalDivider()
            for (reversed in listOf(false, true)) {
                DropdownMenuItem(
                    text = { Text(orderLabel(view.copy(reversed = reversed))) },
                    leadingIcon = { RadioButton(selected = reversed == view.reversed, onClick = null) },
                    onClick = {
                        open = false
                        show(view.copy(reversed = reversed))
                    },
                )
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
