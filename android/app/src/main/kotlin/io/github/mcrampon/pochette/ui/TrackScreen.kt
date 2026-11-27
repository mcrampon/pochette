package io.github.mcrampon.pochette.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import io.github.mcrampon.pochette.Load
import io.github.mcrampon.pochette.R
import io.github.mcrampon.pochette.core.LinkedTrack
import io.github.mcrampon.pochette.core.PochetteException

/** What can be asked of a track's page. */
class TrackActions(
    val load: suspend () -> LinkedTrack,
    val openAlbum: (id: String) -> Unit,
    val back: () -> Unit,
)

/** Where a link to a track leads: Pochette plays albums, whole, so it offers the track's album. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TrackScreen(actions: TrackActions) {
    var attempt by remember { mutableIntStateOf(0) }
    val load by produceState<Load<LinkedTrack>>(Load.Pending, attempt) {
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
        Column(
            Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 32.dp, vertical = 24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterVertically),
        ) {
            Text(stringResource(R.string.track_heading), style = MaterialTheme.typography.headlineSmall, textAlign = TextAlign.Center)
            Text(
                stringResource(R.string.track_albums_only),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
            when (val current = load) {
                Load.Pending -> Loading()
                is Load.Failed -> Failed(stringResource(R.string.track_cant_load, current.error), onRetry = { attempt++ })
                is Load.Done -> {
                    val track = current.value
                    val albumId = track.albumId
                    if (albumId != null) {
                        Text(
                            stringResource(R.string.track_on_album, track.title, track.albumTitle.orEmpty()),
                            style = MaterialTheme.typography.bodyLarge,
                            textAlign = TextAlign.Center,
                        )
                        Button(onClick = { actions.openAlbum(albumId) }) { Text(stringResource(R.string.track_open_album)) }
                    }
                }
            }
        }
    }
}
