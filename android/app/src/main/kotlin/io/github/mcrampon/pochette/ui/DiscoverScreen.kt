package io.github.mcrampon.pochette.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.toMutableStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import io.github.mcrampon.pochette.Load
import io.github.mcrampon.pochette.R
import io.github.mcrampon.pochette.core.AlbumCard
import io.github.mcrampon.pochette.core.AlbumList
import io.github.mcrampon.pochette.core.Discover
import io.github.mcrampon.pochette.core.Genre
import io.github.mcrampon.pochette.core.PochetteException
import io.github.mcrampon.pochette.core.Section

/** What can be asked of Discover. */
class DiscoverActions(
    val load: () -> Unit,
    val genres: suspend () -> List<Genre>,
    val picked: () -> List<ULong>,
    val pick: (List<ULong>) -> Unit,
    val seeAll: (Section) -> Unit,
    val open: (AlbumCard) -> Unit,
)

/** What Qobuz puts forward: new releases, awarded albums and essentials, in the genres picked. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DiscoverScreen(discover: Load<Discover>, actions: DiscoverActions) {
    LaunchedEffect(Unit) { if (discover !is Load.Done) actions.load() }
    var picking by remember { mutableStateOf(false) }
    val picked = actions.picked()
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Discover") },
                actions = {
                    IconButton(onClick = { picking = true }) {
                        Icon(
                            painterResource(R.drawable.ic_filter_list),
                            contentDescription = "Genres",
                            tint = if (picked.isEmpty()) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.primary,
                        )
                    }
                },
            )
        },
        contentWindowInsets = WindowInsets(0),
    ) { padding ->
        PullToRefreshBox(isRefreshing = false, onRefresh = actions.load, modifier = Modifier.fillMaxSize().padding(padding)) {
            when (discover) {
                Load.Pending -> Loading()
                is Load.Failed -> Failed("Can't load Discover: ${discover.error}", onRetry = actions.load)
                is Load.Done ->
                    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
                        for ((section, list) in sections(discover.value)) {
                            item(key = section.name) {
                                Column {
                                    Heading(title(section), onSeeAll = { actions.seeAll(section) }.takeIf { list.hasMore })
                                    AlbumShelf(list.albums, onOpen = actions.open, rows = 2, caption = caption(section))
                                }
                            }
                        }
                    }
            }
        }
    }
    if (picking) GenresDialog(actions, picked, onDismiss = { picking = false })
}

private fun sections(discover: Discover): List<Pair<Section, AlbumList>> =
    listOf(
        Section.NEW_RELEASES to discover.newReleases,
        Section.PRESS_AWARDS to discover.pressAwards,
        Section.IDEAL_DISCOGRAPHY to discover.idealDiscography,
    ).filter { it.second.albums.isNotEmpty() }

/** What tiles say under the artist: in the Press Awards, which awards. */
fun caption(section: Section): Caption = if (section == Section.PRESS_AWARDS) awards else { _ -> null }

fun title(section: Section) =
    when (section) {
        Section.NEW_RELEASES -> "New Releases"
        Section.PRESS_AWARDS -> "Press Awards"
        Section.IDEAL_DISCOGRAPHY -> "Ideal Discography"
    }

private sealed interface GenresLoad {
    data object Pending : GenresLoad

    data class Done(val genres: List<Genre>) : GenresLoad

    data class Failed(val error: String) : GenresLoad
}

/** The genres to show, none for all. */
@Composable
private fun GenresDialog(actions: DiscoverActions, picked: List<ULong>, onDismiss: () -> Unit) {
    val load by produceState<GenresLoad>(GenresLoad.Pending) {
        value =
            try {
                GenresLoad.Done(actions.genres())
            } catch (err: PochetteException) {
                GenresLoad.Failed(err.message.orEmpty())
            }
    }
    val chosen = remember { picked.toMutableStateList() }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Genres") },
        text = {
            when (val current = load) {
                GenresLoad.Pending -> Loading()
                is GenresLoad.Failed -> Text("Can't load the genres: ${current.error}")
                is GenresLoad.Done ->
                    LazyColumn(Modifier.heightIn(max = 420.dp)) {
                        items(current.genres, key = { it.id.toString() }) { genre ->
                            val on = genre.id in chosen
                            Row(
                                Modifier
                                    .fillMaxWidth()
                                    .toggleable(value = on, role = Role.Checkbox) {
                                        if (it) chosen.add(genre.id) else chosen.remove(genre.id)
                                    }.padding(vertical = 4.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Checkbox(checked = on, onCheckedChange = null)
                                Text(genre.name, modifier = Modifier.padding(start = 12.dp))
                            }
                        }
                    }
            }
        },
        confirmButton = {
            TextButton(onClick = {
                actions.pick(chosen.toList())
                onDismiss()
            }) { Text("Show") }
        },
        dismissButton = {
            TextButton(onClick = {
                actions.pick(emptyList())
                onDismiss()
            }) { Text("All Genres") }
        },
    )
}
