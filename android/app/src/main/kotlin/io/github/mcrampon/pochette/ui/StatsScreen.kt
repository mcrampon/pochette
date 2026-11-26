package io.github.mcrampon.pochette.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.mcrampon.pochette.Load
import io.github.mcrampon.pochette.R
import io.github.mcrampon.pochette.StatsPage
import io.github.mcrampon.pochette.core.AlbumCard
import io.github.mcrampon.pochette.core.ArtistStat
import io.github.mcrampon.pochette.core.Period
import io.github.mcrampon.pochette.core.Stats
import io.github.mcrampon.pochette.core.TimelinePoint
import io.github.mcrampon.pochette.core.listeningTime
import io.github.mcrampon.pochette.core.monthName
import io.github.mcrampon.pochette.core.playCount
import io.github.mcrampon.pochette.core.shortMonthName
import java.time.LocalDate

/** How many artists and genres the stats rank, as on desktop. */
private const val TOP = 10

/** Under this many bars, they're this wide rather than as wide as the screen lets them. */
private const val FEW_BARS = 6

/** The most days the timeline shows at once: the others are a scroll away. */
private const val DAYS_SHOWN = 12

/** What can be asked of the stats. */
class StatsActions(
    /** The stats of the period picked, none before anything was played. */
    val load: suspend () -> StatsPage?,
    val pick: (Period) -> Unit,
    val openAlbum: (AlbumCard) -> Unit,
    val openArtist: (ArtistStat) -> Unit,
)

/** How much was listened to, and the albums, artists and genres played most, over a month, a year or all time. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun StatsScreen(
    period: Period?,
    /** Changes as plays are recorded, to show them. */
    playing: Any?,
    actions: StatsActions,
) {
    // The stats shown stay until the next ones are ready, so changing the period doesn't flash.
    val load by produceState<Load<StatsPage?>>(Load.Pending, period, playing) { value = Load.Done(actions.load()) }

    Scaffold(
        topBar = { TopAppBar(title = { Text("Stats") }) },
        contentWindowInsets = WindowInsets(0),
    ) { padding ->
        when (val current = load) {
            is Load.Done -> {
                val page = current.value
                if (page == null) {
                    NothingPlayed(Modifier.fillMaxSize().padding(padding))
                } else {
                    StatsContent(page, actions, Modifier.fillMaxSize().padding(padding))
                }
            }
            // Reading the logs takes a moment at most.
            else -> Box(Modifier.fillMaxSize().padding(padding))
        }
    }
}

/** The numbers and rankings across the screen, then every album played, the most first, in a grid. */
@Composable
private fun StatsContent(page: StatsPage, actions: StatsActions, modifier: Modifier) {
    val stats = page.stats
    val captions = remember(stats) { stats.albums.associate { it.album.id to "${playCount(it.plays)} · ${listeningTime(it.time)}" } }
    LazyVerticalGrid(
        columns = GridCells.Adaptive(150.dp),
        modifier = modifier,
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        across("periods") { Periods(page, actions.pick) }
        across("numbers") { Numbers(stats) }
        across("timeline") { Timeline(stats, page.period, actions.pick) }
        if (stats.artists.isNotEmpty()) {
            across("artists") { Artists(stats, actions.openArtist) }
        }
        if (stats.genres.isNotEmpty()) {
            across("genres") { Genres(stats) }
        }
        if (stats.albums.isNotEmpty()) {
            across("albums") { Heading("Top Albums") }
            items(stats.albums, key = { "album/${it.album.id}" }) { stat ->
                AlbumTile(
                    stat.album,
                    onClick = { actions.openAlbum(stat.album) },
                    Modifier.padding(bottom = 20.dp),
                    caption = captions[stat.album.id],
                )
            }
        }
    }
}

/** A section as wide as the screen, over the grid's albums. */
private fun LazyGridScope.across(key: String, content: @Composable () -> Unit) =
    item(key = key, span = { GridItemSpan(maxLineSpan) }) { Column(Modifier.fullBleed(16.dp)) { content() } }

/** The year, all time after the years, and the month of the year, the whole year first. */
@Composable
private fun Periods(page: StatsPage, pick: (Period) -> Unit) {
    val period = page.period
    val year =
        when (period) {
            Period.All -> null
            is Period.Year -> period.year
            is Period.Month -> period.year
        }
    Row(Modifier.padding(horizontal = 16.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Picker(
            shown = year?.toString() ?: "All Time",
            choices = page.years.map { it.toString() to Period.Year(it) } + ("All Time" to Period.All),
            pick = pick,
        )
        if (year != null) {
            Picker(
                shown = (period as? Period.Month)?.let { monthName(it.month) } ?: "Whole Year",
                choices = listOf("Whole Year" to Period.Year(year)) + page.months.map { monthName(it) to Period.Month(year, it) },
                pick = pick,
            )
        }
    }
}

@Composable
private fun Picker(shown: String, choices: List<Pair<String, Period>>, pick: (Period) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        OutlinedButton(onClick = { open = true }, contentPadding = PaddingValues(start = 16.dp, end = 8.dp)) {
            Text(shown)
            Icon(painterResource(R.drawable.ic_expand_more), contentDescription = null, modifier = Modifier.padding(start = 4.dp))
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            for ((name, period) in choices) {
                DropdownMenuItem(text = { Text(name) }, onClick = {
                    open = false
                    pick(period)
                })
            }
        }
    }
}

/** The period in a few big numbers. */
@Composable
private fun Numbers(stats: Stats) {
    Row(Modifier.padding(horizontal = 16.dp, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        for ((value, name) in listOf(
            listeningTime(stats.time) to "Listened",
            stats.albumPlays.toString() to "Album Plays",
            stats.artists.size.toString() to "Artists",
        )) {
            Surface(
                color = MaterialTheme.colorScheme.surfaceContainer,
                shape = RoundedCornerShape(12.dp),
                modifier = Modifier.weight(1f),
            ) {
                Column(Modifier.padding(12.dp)) {
                    Text(value, style = MaterialTheme.typography.headlineSmall, maxLines = 1)
                    Text(
                        name,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

/**
 * The time listened by year for all time, by month for a year, by day for a month, in bars; tapping a year or a
 * month shows it. The days scroll, a dozen at a time.
 */
@Composable
private fun Timeline(stats: Stats, period: Period, pick: (Period) -> Unit) {
    Heading(
        when (period) {
            Period.All -> "By Year"
            is Period.Year -> "By Month"
            is Period.Month -> "By Day"
        },
    )
    val most = (stats.timeline.maxOfOrNull { it.time } ?: 0u).coerceAtLeast(1u).toFloat()
    if (period is Period.Month) {
        Days(stats.timeline, period, most)
        return
    }
    val few = stats.timeline.size < FEW_BARS
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp).padding(top = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        for (point in stats.timeline) {
            val (label, opens) =
                when (period) {
                    is Period.Year -> shortMonthName(point.at) to Period.Month(period.year, point.at)
                    else -> point.at.toString() to Period.Year(point.at)
                }
            Bar(
                point,
                label,
                most,
                onClick = { pick(opens) }.takeIf { point.time > 0u },
                modifier = if (few) Modifier.width(56.dp) else Modifier.weight(1f),
            )
        }
    }
}

/**
 * The days of a month, a dozen in view: from the 1st, or for the month under way, up to today, the days still to
 * come a scroll away.
 */
@Composable
private fun Days(timeline: List<TimelinePoint>, period: Period.Month, most: Float) {
    BoxWithConstraints(Modifier.fillMaxWidth().padding(top = 8.dp)) {
        val spacing = 4.dp
        val width = (maxWidth - 32.dp - spacing * (DAYS_SHOWN - 1)) / DAYS_SHOWN
        // Each month opens at its own place.
        key(period) {
            val today = LocalDate.now()
            val first =
                if (period.year == today.year && period.month == today.monthValue) {
                    (today.dayOfMonth - DAYS_SHOWN).coerceAtLeast(0)
                } else {
                    0
                }
            val days = rememberLazyListState(initialFirstVisibleItemIndex = first)
            LazyRow(
                state = days,
                contentPadding = PaddingValues(horizontal = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(spacing),
            ) {
                items(timeline, key = { it.at }) { point -> Bar(point, point.at.toString(), most, onClick = null, Modifier.width(width)) }
            }
        }
    }
}

/** A bar of the timeline, as high as its time against the most, with its label under it. */
@Composable
private fun Bar(point: TimelinePoint, label: String, most: Float, onClick: (() -> Unit)?, modifier: Modifier) {
    Column(
        modifier
            .clip(RoundedCornerShape(4.dp))
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(Modifier.fillMaxWidth().height(120.dp), contentAlignment = Alignment.BottomCenter) {
            if (point.time > 0u) {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .fillMaxHeight((point.time.toFloat() / most).coerceAtLeast(0.025f))
                        .clip(RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp))
                        .background(MaterialTheme.colorScheme.primary),
                )
            }
        }
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            softWrap = false,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
}

@Composable
private fun Artists(stats: Stats, open: (ArtistStat) -> Unit) {
    Column {
        Heading("Top Artists")
        stats.artists.take(TOP).forEachIndexed { rank, artist ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .clickable { open(artist) }
                    .padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(
                    "${rank + 1}",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.End,
                    modifier = Modifier.width(24.dp),
                )
                Text(artist.name, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                Text(listeningTime(artist.time), color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

@Composable
private fun Genres(stats: Stats) {
    val total = stats.genres.sumOf { it.time }.coerceAtLeast(1u).toFloat()
    Column {
        Heading("Top Genres")
        for (genre in stats.genres.take(TOP)) {
            val share = genre.time.toFloat() / total
            Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                Row {
                    Text(genre.name, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                    Text("${Math.round(share * 100)}%", color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                LinearProgressIndicator(
                    progress = { share },
                    drawStopIndicator = {},
                    gapSize = 0.dp,
                    modifier = Modifier.fillMaxWidth().padding(top = 6.dp),
                )
            }
        }
    }
}

/** What shows before anything was played. */
@Composable
private fun NothingPlayed(modifier: Modifier) {
    Column(
        modifier.padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterVertically),
    ) {
        Icon(
            painterResource(R.drawable.ic_bar_chart),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(48.dp),
        )
        Text("Nothing Played Yet", style = MaterialTheme.typography.titleLarge)
        Text(
            "Play some albums: how much you listen, your top albums, artists and genres show up here.",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
    }
}
