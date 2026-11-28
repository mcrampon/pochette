package io.github.mcrampon.pochette.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import io.github.mcrampon.pochette.Paged
import io.github.mcrampon.pochette.R
import io.github.mcrampon.pochette.core.AlbumCard
import androidx.compose.ui.res.stringResource

/** A long list of albums, as "See All" opens: a Discover section, or an artist's releases of one kind. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ListScreen(
    title: String,
    subtitle: String?,
    paged: Paged,
    onOpen: (AlbumCard) -> Unit,
    onBack: () -> Unit,
    caption: Caption = { null },
    /** In the top bar, after the title: a filter. */
    actions: @Composable RowScope.() -> Unit = {},
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        subtitle?.let {
                            Text(
                                it,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(painterResource(R.drawable.ic_arrow_back), contentDescription = stringResource(R.string.back))
                    }
                },
                actions = actions,
            )
        },
        contentWindowInsets = WindowInsets(0),
    ) { padding ->
        PagedGrid(paged, onOpen, Modifier.fillMaxSize().padding(padding), empty = stringResource(R.string.no_albums_here), caption = caption)
    }
}
