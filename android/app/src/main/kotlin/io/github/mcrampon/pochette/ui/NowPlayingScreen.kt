package io.github.mcrampon.pochette.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.mcrampon.pochette.R
import io.github.mcrampon.pochette.core.LyricLine
import io.github.mcrampon.pochette.core.Lyrics
import io.github.mcrampon.pochette.core.PochetteException
import io.github.mcrampon.pochette.core.Quality
import io.github.mcrampon.pochette.core.currentLine
import io.github.mcrampon.pochette.core.trackDuration
import io.github.mcrampon.pochette.playback.NowPlaying
import kotlinx.coroutines.delay

/** What can be asked of Now Playing. */
class NowPlayingActions(
    val position: () -> Long,
    val playPause: () -> Unit,
    val previous: () -> Unit,
    val next: () -> Unit,
    val seekTo: (Long) -> Unit,
    val skipTo: (Int) -> Unit,
    val lyrics: suspend (NowPlaying) -> Lyrics,
    val quality: () -> Quality,
    val setQuality: (Quality) -> Unit,
    val openAlbum: () -> Unit,
    val back: () -> Unit,
)

/** The track playing, big: its cover or lyrics, where it is, the controls, and the queue. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NowPlayingScreen(now: NowPlaying, actions: NowPlayingActions) {
    var position by remember { mutableLongStateOf(actions.position()) }
    LaunchedEffect(now.trackId, now.playing) {
        while (true) {
            position = actions.position()
            delay(250)
        }
    }
    var showLyrics by rememberSaveable { mutableStateOf(false) }
    var choosingQuality by remember { mutableStateOf(false) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {},
                navigationIcon = {
                    IconButton(onClick = actions.back) {
                        Icon(painterResource(R.drawable.ic_expand_more), contentDescription = "Close")
                    }
                },
            )
        },
        contentWindowInsets = WindowInsets(0),
    ) { padding ->
        LazyColumn(
            Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(start = 24.dp, end = 24.dp, bottom = 32.dp),
        ) {
            item {
                Box(Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(12.dp))) {
                    if (showLyrics) {
                        LyricsView(now, actions, position)
                    } else {
                        Cover(now.cover, Modifier.fillMaxSize(), corner = 12)
                    }
                    FilledTonalIconButton(
                        onClick = { showLyrics = !showLyrics },
                        modifier = Modifier.align(Alignment.BottomEnd).padding(8.dp),
                    ) {
                        Icon(
                            painterResource(if (showLyrics) R.drawable.ic_image else R.drawable.ic_lyrics),
                            contentDescription = if (showLyrics) "Show the Cover" else "Show the Lyrics",
                        )
                    }
                }
                Spacer(Modifier.height(20.dp))
                Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(now.title, style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
                    Text(
                        now.album,
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.primary,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.clip(RoundedCornerShape(4.dp)).clickable(onClick = actions.openAlbum),
                    )
                    Text(
                        now.artist,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                    )
                    now.quality?.let {
                        Surface(
                            onClick = { choosingQuality = true },
                            color = MaterialTheme.colorScheme.secondaryContainer,
                            shape = RoundedCornerShape(6.dp),
                            modifier = Modifier.padding(top = 10.dp),
                        ) {
                            Text(it, style = MaterialTheme.typography.labelMedium, modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp))
                        }
                    }
                }
                Progress(now, position, onSeek = {
                    position = it
                    actions.seekTo(it)
                })
                Controls(now, actions)
                now.error?.let {
                    Text(
                        it,
                        color = MaterialTheme.colorScheme.error,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    )
                }
                Text("Tracks", style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 24.dp, bottom = 4.dp))
            }
            itemsIndexed(now.queue, key = { index, queued -> "$index/${queued.mediaId}" }) { index, queued ->
                val current = index == now.index
                val color = if (current) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface
                Row(
                    Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(8.dp))
                        .clickable { actions.skipTo(index) }
                        .padding(vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Box(Modifier.width(28.dp)) {
                        if (current) {
                            Icon(painterResource(R.drawable.ic_graphic_eq), contentDescription = "Playing", tint = color, modifier = Modifier.size(18.dp))
                        } else {
                            Text("${index + 1}", color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium)
                        }
                    }
                    Text(queued.title, color = color, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                    Text(
                        trackDuration((queued.durationMs / 1000).toUInt()),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
    if (choosingQuality) {
        QualityDialog(actions.quality(), onChoose = actions.setQuality, onDismiss = { choosingQuality = false })
    }
}

@Composable
private fun Progress(now: NowPlaying, position: Long, onSeek: (Long) -> Unit) {
    // Where the thumb is while it's dragged: the player only goes there once it's let go.
    var dragged by remember { mutableStateOf<Float?>(null) }
    val duration = now.durationMs.coerceAtLeast(1)
    val shown = dragged ?: position.coerceIn(0, duration).toFloat()
    Column(Modifier.padding(top = 16.dp)) {
        Slider(
            value = shown,
            onValueChange = { dragged = it },
            onValueChangeFinished = {
                dragged?.let { onSeek(it.toLong()) }
                dragged = null
            },
            valueRange = 0f..duration.toFloat(),
        )
        Row(Modifier.fillMaxWidth()) {
            Text(trackDuration((shown.toLong() / 1000).toUInt()), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.weight(1f))
            Text(trackDuration((now.durationMs / 1000).toUInt()), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun Controls(now: NowPlaying, actions: NowPlayingActions) {
    Row(
        Modifier.fillMaxWidth().padding(top = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(24.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = actions.previous, modifier = Modifier.size(56.dp)) {
            Icon(painterResource(R.drawable.ic_skip_previous), contentDescription = "Previous Track", modifier = Modifier.size(32.dp))
        }
        FilledIconButton(
            onClick = actions.playPause,
            modifier = Modifier.size(72.dp),
            colors = IconButtonDefaults.filledIconButtonColors(),
        ) {
            Icon(
                painterResource(if (now.playing) R.drawable.ic_pause else R.drawable.ic_play),
                contentDescription = if (now.playing) "Pause" else "Play",
                modifier = Modifier.size(36.dp),
            )
        }
        IconButton(onClick = actions.next, enabled = now.hasNext, modifier = Modifier.size(56.dp)) {
            Icon(painterResource(R.drawable.ic_skip_next), contentDescription = "Next Track", modifier = Modifier.size(32.dp))
        }
    }
}

private sealed interface LyricsLoad {
    data object Pending : LyricsLoad

    data class Done(val lyrics: Lyrics) : LyricsLoad

    data class Failed(val error: String) : LyricsLoad
}

/** The lyrics over the cover, blurred and darkened: the line sung stands out, and follows the song. */
@Composable
private fun LyricsView(now: NowPlaying, actions: NowPlayingActions, position: Long) {
    val load by produceState<LyricsLoad>(LyricsLoad.Pending, now.trackId) {
        value =
            try {
                LyricsLoad.Done(actions.lyrics(now))
            } catch (err: PochetteException) {
                LyricsLoad.Failed(err.message.orEmpty())
            }
    }
    Box(Modifier.fillMaxSize()) {
        // Blurred from Android 12; only darkened before.
        Cover(now.cover, Modifier.fillMaxSize().blur(24.dp), corner = 12)
        Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.6f)))
        when (val current = load) {
            LyricsLoad.Pending -> CircularProgressIndicator(Modifier.align(Alignment.Center), color = Color.White)
            is LyricsLoad.Failed -> Message("Can't load the lyrics: ${current.error}")
            is LyricsLoad.Done ->
                when (val lyrics = current.lyrics) {
                    is Lyrics.Synced -> Synced(lyrics.lines, position, actions.seekTo)
                    is Lyrics.Plain ->
                        Text(
                            lyrics.text,
                            color = Color.White,
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
                        )
                    Lyrics.Instrumental -> Message("Instrumental")
                    Lyrics.Missing -> Message("No lyrics for this track")
                }
        }
    }
}

@Composable
private fun Synced(lines: List<LyricLine>, position: Long, onSeek: (Long) -> Unit) {
    val current = currentLine(lines, position.coerceAtLeast(0).toULong())?.toInt()
    val list = rememberLazyListState()
    val padding = 120.dp
    val paddingPx = with(LocalDensity.current) { padding.roundToPx() }
    // The line sung, a third of the way down: scrolled to, it would be under the padding.
    LaunchedEffect(current) {
        list.animateScrollToItem((current ?: 0).coerceAtLeast(0), scrollOffset = paddingPx - list.layoutInfo.viewportSize.height / 3)
    }
    LazyColumn(
        state = list,
        modifier =
            Modifier
                .fillMaxSize()
                // Lines fade out at the edges rather than being cut.
                .graphicsLayer(compositingStrategy = CompositingStrategy.Offscreen)
                .drawWithContent {
                    drawContent()
                    drawRect(
                        Brush.verticalGradient(0f to Color.Transparent, 0.15f to Color.Black, 0.85f to Color.Black, 1f to Color.Transparent),
                        blendMode = BlendMode.DstIn,
                    )
                },
        contentPadding = PaddingValues(vertical = padding, horizontal = 20.dp),
    ) {
        itemsIndexed(lines) { index, line ->
            val sung = index == current
            Text(
                line.text.ifEmpty { "♪" },
                color = Color.White.copy(alpha = if (sung) 1f else 0.5f),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = if (sung) FontWeight.Bold else FontWeight.Normal,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(4.dp))
                        .clickable { onSeek(line.atMs.toLong()) }
                        .padding(vertical = 6.dp),
            )
        }
    }
}

@Composable
private fun Message(text: String) {
    Box(Modifier.fillMaxSize().padding(24.dp), contentAlignment = Alignment.Center) {
        Text(text, color = Color.White, textAlign = TextAlign.Center, style = MaterialTheme.typography.titleMedium)
    }
}

/** The best quality to stream in. */
@Composable
private fun QualityDialog(saved: Quality, onChoose: (Quality) -> Unit, onDismiss: () -> Unit) {
    var chosen by remember { mutableStateOf(saved) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Best Quality") },
        text = {
            Column {
                Text(
                    "Each track streams in the best it has, up to this, as your subscription allows.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
                for (quality in Quality.entries.reversed()) {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .selectable(selected = quality == chosen, role = Role.RadioButton) {
                                chosen = quality
                                onChoose(quality)
                            }.padding(vertical = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = quality == chosen, onClick = null)
                        Text(qualityName(quality), modifier = Modifier.padding(start = 12.dp))
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Done") } },
    )
}

private fun qualityName(quality: Quality) =
    when (quality) {
        Quality.MP3 -> "MP3 · 320 kbps"
        Quality.CD -> "CD · 16-bit / 44.1 kHz"
        Quality.HI_RES96 -> "Hi-Res · up to 24-bit / 96 kHz"
        Quality.HI_RES192 -> "Hi-Res · up to 24-bit / 192 kHz"
    }
