package io.github.mcrampon.pochette.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
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

/** The genres to show, none for all. */
@Composable
private fun GenresDialog(actions: DiscoverActions, picked: List<ULong>, onDismiss: () -> Unit) {
    val genres by produceState<Load<List<Pair<ULong, String>>>>(Load.Pending) {
        value =
            try {
                Load.Done(actions.genres().map { it.id to it.name })
            } catch (err: PochetteException) {
                Load.Failed("Can't load the genres: ${err.message}")
            }
    }
    ChecklistDialog("Genres", genres, picked, all = "All Genres", onPick = actions.pick, onDismiss = onDismiss)
}
