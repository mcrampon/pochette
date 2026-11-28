package io.github.mcrampon.pochette.ui

import android.text.format.Formatter
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import io.github.mcrampon.pochette.R
import io.github.mcrampon.pochette.core.Quality
import io.github.mcrampon.pochette.core.Pairing
import io.github.mcrampon.pochette.core.PairingStage
import io.github.mcrampon.pochette.core.Peer
import io.github.mcrampon.pochette.core.SyncStatus
import io.github.mcrampon.pochette.core.since
import io.github.mcrampon.pochette.core.syncPort
import kotlinx.coroutines.delay
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.selection.selectable
import androidx.compose.material3.RadioButton
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import io.github.mcrampon.pochette.Language

/** How often "Synced 3 minutes ago" is said again. */
private const val CLOCK_MS = 30_000L

/** A sync error shows once the last sync is this old, in seconds: a device that's just off for a moment isn't news. */
private const val STALE_SECONDS = 15 * 60

/** What can be asked of the settings: the language, the devices. */
class SettingsActions(
    /** The language picked, by BCP 47 tag, none for the phone's. */
    val language: () -> String?,
    /** Show the language of a BCP 47 tag, the phone's when none. */
    val setLanguage: (String?) -> Unit,
    val setPairable: (Boolean) -> Unit,
    val pair: (String) -> Unit,
    val answer: (Boolean) -> Unit,
    val dismiss: () -> Unit,
    val forget: (String) -> Unit,
    val background: () -> Boolean,
    val setBackground: (Boolean) -> Unit,
    val downloadQuality: () -> Quality,
    val setDownloadQuality: (Quality) -> Unit,
    /** Whether albums download on mobile data too, not only on Wi-Fi. */
    val mobileData: () -> Boolean,
    val setMobileData: (Boolean) -> Unit,
    /** How much room the downloads take, in bytes. */
    val downloadsSize: () -> Long,
    val removeAllDownloads: () -> Unit,
    val back: () -> Unit,
)

/**
 * The settings: the language, and the devices the plays sync with, and pairing more: on the same network, the
 * code checked on both.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(status: SyncStatus, actions: SettingsActions) {
    // Found by the others, and ready to pair, while it shows.
    DisposableEffect(Unit) {
        actions.setPairable(true)
        onDispose { actions.setPairable(false) }
    }
    val resources = LocalResources.current
    var notice by rememberSaveable { mutableStateOf<String?>(null) }
    var background by rememberSaveable { mutableStateOf(actions.background()) }
    var choosingLanguage by remember { mutableStateOf(false) }
    // How the last pairing went, once it's over.
    val pairing = status.pairing
    LaunchedEffect(pairing) {
        if (pairing == null) return@LaunchedEffect
        val name = pairing.name
        val outcome =
            when (val stage = pairing.stage) {
                PairingStage.Paired -> resources.getString(R.string.paired_with, name)
                is PairingStage.Declined -> resources.getString(if (stage.here) R.string.not_paired_with else R.string.declined_to_pair, name)
                is PairingStage.Failed -> resources.getString(R.string.cant_pair_with, name, stage.reason)
                else -> return@LaunchedEffect
            }
        // Syncing is what pairing is for: in the background too, from the first device on.
        if (pairing.stage == PairingStage.Paired && status.peers.size <= 1 && !background) {
            background = true
            actions.setBackground(true)
        }
        notice = outcome
        actions.dismiss()
    }
    val clock by produceState(0) {
        while (true) {
            delay(CLOCK_MS)
            value++
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.settings)) },
                navigationIcon = {
                    IconButton(onClick = actions.back) {
                        Icon(painterResource(R.drawable.ic_arrow_back), contentDescription = stringResource(R.string.back))
                    }
                },
            )
        },
        contentWindowInsets = WindowInsets(0),
    ) { padding ->
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(bottom = 24.dp)) {
            item(key = "language") {
                Column {
                    Section(stringResource(R.string.language))
                    ListItem(
                        headlineContent = { Text(actions.language()?.let(Language::name) ?: stringResource(R.string.language_system)) },
                        trailingContent = { Icon(painterResource(R.drawable.ic_expand_more), contentDescription = null) },
                        modifier = Modifier.clickable { choosingLanguage = true },
                    )
                }
            }
            item(key = "downloads") { Downloads(actions) }
            item(key = "devices") { Section(stringResource(R.string.devices)) }
            item(key = "about") {
                Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Dim(stringResource(R.string.devices_about, status.name))
                    status.problem?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                    notice?.let { Text(it) }
                }
            }
            if (status.peers.isNotEmpty()) {
                item(key = "paired") { Heading(stringResource(R.string.paired_devices)) }
                items(status.peers, key = { "peer/${it.id}" }) { peer -> PeerRow(peer, clock, actions.forget) }
                item(key = "background") {
                    ListItem(
                        headlineContent = { Text(stringResource(R.string.sync_in_background)) },
                        supportingContent = { Text(stringResource(R.string.sync_in_background_detail)) },
                        trailingContent = {
                            Switch(checked = background, onCheckedChange = {
                                background = it
                                actions.setBackground(it)
                            })
                        },
                    )
                }
            }
            item(key = "pair") {
                Column {
                    Heading(stringResource(R.string.pair_a_device))
                    Dim(stringResource(R.string.pair_a_device_detail), Modifier.padding(horizontal = 16.dp))
                }
            }
            items(status.nearby, key = { "nearby/${it.id}" }) { device ->
                ListItem(
                    headlineContent = { Text(device.name) },
                    trailingContent = { FilledTonalButton(onClick = { actions.pair(device.id) }) { Text(stringResource(R.string.pair)) } },
                )
            }
            if (status.nearby.isEmpty()) {
                item(key = "looking") {
                    Row(
                        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 16.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(16.dp),
                    ) {
                        CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                        Dim(stringResource(R.string.looking_for_devices))
                    }
                }
            }
            item(key = "address") { ByAddress(actions.pair) }
        }
    }
    if (pairing != null && pairing.stage.ongoing) PairingDialog(pairing, actions.answer)
    if (choosingLanguage) {
        LanguageDialog(
            actions.language(),
            onChoose = {
                choosingLanguage = false
                actions.setLanguage(it)
            },
            onDismiss = { choosingLanguage = false },
        )
    }
}

/** A section of the settings, over its own headings. */
/** The quality albums download in, whether on mobile data too, and the room they take. */
@Composable
private fun Downloads(actions: SettingsActions) {
    val context = LocalContext.current
    var quality by remember { mutableStateOf(actions.downloadQuality()) }
    var choosingQuality by remember { mutableStateOf(false) }
    var mobileData by remember { mutableStateOf(actions.mobileData()) }
    var size by remember { mutableLongStateOf(actions.downloadsSize()) }
    var removing by remember { mutableStateOf(false) }
    Column {
        Section(stringResource(R.string.downloads))
        ListItem(
            headlineContent = { Text(stringResource(R.string.download_quality)) },
            supportingContent = { Text(stringResource(qualityName(quality))) },
            trailingContent = { Icon(painterResource(R.drawable.ic_expand_more), contentDescription = null) },
            modifier = Modifier.clickable { choosingQuality = true },
        )
        ListItem(
            headlineContent = { Text(stringResource(R.string.download_on_mobile_data)) },
            supportingContent = { Text(stringResource(R.string.download_on_mobile_data_detail)) },
            trailingContent = {
                Switch(checked = mobileData, onCheckedChange = {
                    mobileData = it
                    actions.setMobileData(it)
                })
            },
        )
        ListItem(
            headlineContent = { Text(stringResource(R.string.downloads_space)) },
            supportingContent = { Text(Formatter.formatShortFileSize(context, size)) },
            trailingContent = {
                TextButton(onClick = { removing = true }, enabled = size > 0) { Text(stringResource(R.string.remove_all_downloads)) }
            },
        )
    }
    if (choosingQuality) {
        QualityDialog(R.string.download_quality, R.string.download_quality_detail, quality, onChoose = {
            quality = it
            actions.setDownloadQuality(it)
        }, onDismiss = { choosingQuality = false })
    }
    if (removing) {
        AlertDialog(
            onDismissRequest = { removing = false },
            title = { Text(stringResource(R.string.remove_all_downloads_question)) },
            text = { Text(stringResource(R.string.remove_all_downloads_detail)) },
            confirmButton = {
                TextButton(onClick = {
                    removing = false
                    actions.removeAllDownloads()
                    size = actions.downloadsSize()
                }) { Text(stringResource(R.string.remove)) }
            },
            dismissButton = { TextButton(onClick = { removing = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

@Composable
private fun Section(title: String) {
    Text(
        title,
        style = MaterialTheme.typography.titleLarge,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 24.dp, bottom = 8.dp),
    )
}

private val PairingStage.ongoing
    get() = this == PairingStage.Connecting || this == PairingStage.Confirm || this == PairingStage.Waiting

@Composable
private fun PeerRow(peer: Peer, clock: Int, forget: (String) -> Unit) {
    var forgetting by rememberSaveable { mutableStateOf(false) }
    // Said again as time passes.
    val detail =
        clock.let {
            val last = peer.lastSync
            val error = peer.error
            when {
                peer.syncing -> stringResource(R.string.syncing)
                error != null && (last == null || System.currentTimeMillis() / 1000 - last > STALE_SECONDS) ->
                    if (last == null) error else stringResource(R.string.synced_ago_error, since(last), error)
                last != null -> stringResource(R.string.synced_ago, since(last))
                else -> stringResource(R.string.not_synced_yet)
            }
        }
    ListItem(
        headlineContent = { Text(peer.name) },
        supportingContent = { Text(detail) },
        trailingContent = { TextButton(onClick = { forgetting = true }) { Text(stringResource(R.string.forget)) } },
    )
    if (forgetting) {
        AlertDialog(
            onDismissRequest = { forgetting = false },
            title = { Text(stringResource(R.string.forget_device, peer.name)) },
            text = { Text(stringResource(R.string.forget_device_detail)) },
            confirmButton = {
                TextButton(onClick = {
                    forgetting = false
                    forget(peer.id)
                }) { Text(stringResource(R.string.forget)) }
            },
            dismissButton = { TextButton(onClick = { forgetting = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

/** Pairing with a device at an address, for networks where devices can't find each other. */
@Composable
private fun ByAddress(pair: (String) -> Unit) {
    var open by rememberSaveable { mutableStateOf(false) }
    var address by rememberSaveable { mutableStateOf("") }
    Column(Modifier.padding(horizontal = 16.dp).padding(top = 8.dp)) {
        TextButton(onClick = { open = !open }, contentPadding = PaddingValues(0.dp)) { Text(stringResource(R.string.pair_by_address)) }
        if (open) {
            val go = { if (address.isNotBlank()) pair(address.trim()) }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = address,
                    onValueChange = { address = it },
                    placeholder = { Text(stringResource(R.string.address_example)) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
                    keyboardActions = KeyboardActions(onGo = { go() }),
                    modifier = Modifier.weight(1f),
                )
                Button(onClick = go, enabled = address.isNotBlank()) { Text(stringResource(R.string.pair)) }
            }
            Dim(
                stringResource(R.string.pair_by_address_detail, syncPort().toInt()),
                Modifier.padding(top = 8.dp),
                small = true,
            )
        }
    }
}

/** A pairing going on: connecting, the code to check on both devices, then waiting for the other's answer. */
@Composable
private fun PairingDialog(pairing: Pairing, answer: (Boolean) -> Unit) {
    val name = pairing.name
    val confirming = pairing.stage == PairingStage.Confirm
    AlertDialog(
        // Only the buttons answer: the code is what pairing is about.
        onDismissRequest = {},
        title = { Text(stringResource(R.string.pair_with, name), textAlign = TextAlign.Center) },
        text = {
            Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Text(
                    when (pairing.stage) {
                        PairingStage.Connecting -> stringResource(R.string.connecting)
                        PairingStage.Confirm -> stringResource(R.string.check_code, name)
                        else -> stringResource(R.string.waiting_to_confirm, name)
                    },
                    textAlign = TextAlign.Center,
                )
                if (confirming) {
                    Text(pairing.code, style = MaterialTheme.typography.displaySmall)
                } else {
                    CircularProgressIndicator()
                }
            }
        },
        confirmButton = { if (confirming) TextButton(onClick = { answer(true) }) { Text(stringResource(R.string.pair)) } },
        dismissButton = { if (confirming) TextButton(onClick = { answer(false) }) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun Dim(text: String, modifier: Modifier = Modifier, small: Boolean = false) {
    Text(
        text,
        style = if (small) MaterialTheme.typography.bodySmall else MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier,
    )
}

/** The languages to show the app in, by their own names, the phone's first: `chosen` is the tag of the one picked. */
@Composable
private fun LanguageDialog(chosen: String?, onChoose: (String?) -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.language)) },
        text = {
            val choices = listOf(null to stringResource(R.string.language_system)) + Language.choices
            LazyColumn(Modifier.heightIn(max = 420.dp)) {
                items(choices, key = { it.first.orEmpty() }) { (tag, name) ->
                    val selected = tag.equals(chosen, ignoreCase = true)
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .selectable(selected = selected, role = Role.RadioButton) { if (selected) onDismiss() else onChoose(tag) }
                            .padding(vertical = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = selected, onClick = null)
                        Text(name, modifier = Modifier.padding(start = 12.dp))
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
