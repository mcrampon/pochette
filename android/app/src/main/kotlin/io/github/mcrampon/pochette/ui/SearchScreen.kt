package io.github.mcrampon.pochette.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.mcrampon.pochette.Found
import io.github.mcrampon.pochette.Load
import io.github.mcrampon.pochette.R
import io.github.mcrampon.pochette.core.AlbumCard
import io.github.mcrampon.pochette.core.ArtistCard
import kotlinx.coroutines.delay
import androidx.compose.ui.res.stringResource

/** How long typing pauses before searching. */
private const val TYPING_PAUSE_MS = 400L

/** Qobuz's albums and artists, searched as the user types. */
@Composable
fun SearchScreen(found: Found?, onSearch: (String) -> Unit, onOpenAlbum: (AlbumCard) -> Unit, onOpenArtist: (ArtistCard) -> Unit) {
    var query by rememberSaveable { mutableStateOf(found?.query.orEmpty()) }
    LaunchedEffect(query) {
        delay(TYPING_PAUSE_MS)
        onSearch(query)
    }
    val focus = remember { FocusRequester() }
    // Ready to type when opened on nothing searched yet.
    LaunchedEffect(Unit) { if (query.isEmpty()) focus.requestFocus() }
    val focusManager = LocalFocusManager.current

    Scaffold(
        topBar = {
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                placeholder = { Text(stringResource(R.string.search_hint), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                leadingIcon = { Icon(painterResource(R.drawable.ic_search), contentDescription = null) },
                trailingIcon = {
                    if (query.isNotEmpty()) {
                        IconButton(onClick = {
                            query = ""
                            focus.requestFocus()
                        }) { Icon(painterResource(R.drawable.ic_close), contentDescription = stringResource(R.string.clear)) }
                    }
                },
                singleLine = true,
                shape = RoundedCornerShape(28.dp),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions =
                    KeyboardActions(onSearch = {
                        onSearch(query)
                        focusManager.clearFocus()
                    }),
                modifier = Modifier.fillMaxWidth().statusBarsPadding().padding(16.dp).focusRequester(focus),
            )
        },
        contentWindowInsets = WindowInsets(0),
    ) { padding ->
        if (found == null) {
            Box(Modifier.fillMaxSize().padding(padding).padding(32.dp), contentAlignment = Alignment.Center) {
                Text(
                    stringResource(R.string.search_prompt),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                )
            }
            return@Scaffold
        }
        PagedGrid(found.albums, onOpenAlbum, Modifier.fillMaxSize().padding(padding), empty = stringResource(R.string.search_no_albums, found.query)) {
            val artists = (found.artists as? Load.Done)?.value.orEmpty()
            if (artists.isNotEmpty()) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    Column(Modifier.fullBleed(16.dp)) {
                        Heading(stringResource(R.string.artists))
                        ArtistShelf(artists, onOpen = onOpenArtist)
                    }
                }
            }
            if (found.albums.albums.isNotEmpty()) {
                item(span = { GridItemSpan(maxLineSpan) }) { Column(Modifier.fullBleed(16.dp)) { Heading(stringResource(R.string.albums)) } }
            }
        }
    }
}
