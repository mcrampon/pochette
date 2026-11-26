package io.github.mcrampon.pochette.ui

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
import io.github.mcrampon.pochette.core.Pairing
import io.github.mcrampon.pochette.core.PairingStage
import io.github.mcrampon.pochette.core.Peer
import io.github.mcrampon.pochette.core.SyncStatus
import io.github.mcrampon.pochette.core.since
import io.github.mcrampon.pochette.core.syncPort
import kotlinx.coroutines.delay

/** How often "Synced 3 minutes ago" is said again. */
private const val CLOCK_MS = 30_000L

/** A sync error shows once the last sync is this old, in seconds: a device that's just off for a moment isn't news. */
private const val STALE_SECONDS = 15 * 60

/** What can be asked of the devices. */
class DevicesActions(
    val setPairable: (Boolean) -> Unit,
    val pair: (String) -> Unit,
    val answer: (Boolean) -> Unit,
    val dismiss: () -> Unit,
    val forget: (String) -> Unit,
    val background: () -> Boolean,
    val setBackground: (Boolean) -> Unit,
    val back: () -> Unit,
)

/** The devices the plays sync with, and pairing more: on the same network, the code checked on both. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DevicesScreen(status: SyncStatus, actions: DevicesActions) {
    // Found by the others, and ready to pair, while it shows.
    DisposableEffect(Unit) {
        actions.setPairable(true)
        onDispose { actions.setPairable(false) }
    }
    var notice by rememberSaveable { mutableStateOf<String?>(null) }
    var background by rememberSaveable { mutableStateOf(actions.background()) }
    // How the last pairing went, once it's over.
    val pairing = status.pairing
    LaunchedEffect(pairing) {
        if (pairing == null) return@LaunchedEffect
        val name = pairing.name
        val outcome =
            when (val stage = pairing.stage) {
                PairingStage.Paired -> "Paired with $name."
                is PairingStage.Declined -> if (stage.here) "Not paired with $name." else "$name declined to pair."
                is PairingStage.Failed -> "Can't pair with $name: ${stage.reason}"
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
                title = { Text("Devices") },
                navigationIcon = {
                    IconButton(onClick = actions.back) {
                        Icon(painterResource(R.drawable.ic_arrow_back), contentDescription = "Back")
                    }
                },
            )
        },
        contentWindowInsets = WindowInsets(0),
    ) { padding ->
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(bottom = 24.dp)) {
            item(key = "about") {
                Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Dim(
                        "Paired devices share what you play with each other, for the stats and play counts, whenever " +
                            "they're on the same network. This phone is “${status.name}”.",
                    )
                    status.problem?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                    notice?.let { Text(it) }
                }
            }
            if (status.peers.isNotEmpty()) {
                item(key = "paired") { Heading("Paired Devices") }
                items(status.peers, key = { "peer/${it.id}" }) { peer -> PeerRow(peer, clock, actions.forget) }
                item(key = "background") {
                    ListItem(
                        headlineContent = { Text("Keep syncing while Pochette is closed") },
                        supportingContent = { Text("Every 15 minutes or so, on Wi-Fi.") },
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
                    Heading("Pair a Device")
                    Dim("Open Devices in Pochette on the other device as well, and it shows up here.", Modifier.padding(horizontal = 16.dp))
                }
            }
            items(status.nearby, key = { "nearby/${it.id}" }) { device ->
                ListItem(
                    headlineContent = { Text(device.name) },
                    trailingContent = { FilledTonalButton(onClick = { actions.pair(device.id) }) { Text("Pair") } },
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
                        Dim("Looking for devices on this network…")
                    }
                }
            }
            item(key = "address") { ByAddress(actions.pair) }
        }
    }
    if (pairing != null && pairing.stage.ongoing) PairingDialog(pairing, actions.answer)
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
                peer.syncing -> "Syncing…"
                error != null && (last == null || System.currentTimeMillis() / 1000 - last > STALE_SECONDS) ->
                    if (last == null) error else "Synced ${since(last)}. $error"
                last != null -> "Synced ${since(last)}"
                else -> "Not synced yet"
            }
        }
    ListItem(
        headlineContent = { Text(peer.name) },
        supportingContent = { Text(detail) },
        trailingContent = { TextButton(onClick = { forgetting = true }) { Text("Forget") } },
    )
    if (forgetting) {
        AlertDialog(
            onDismissRequest = { forgetting = false },
            title = { Text("Forget ${peer.name}?") },
            text = { Text("This phone stops syncing with it. The plays already shared stay. Pair again to sync again.") },
            confirmButton = {
                TextButton(onClick = {
                    forgetting = false
                    forget(peer.id)
                }) { Text("Forget") }
            },
            dismissButton = { TextButton(onClick = { forgetting = false }) { Text("Cancel") } },
        )
    }
}

/** Pairing with a device at an address, for networks where devices can't find each other. */
@Composable
private fun ByAddress(pair: (String) -> Unit) {
    var open by rememberSaveable { mutableStateOf(false) }
    var address by rememberSaveable { mutableStateOf("") }
    Column(Modifier.padding(horizontal = 16.dp).padding(top = 8.dp)) {
        TextButton(onClick = { open = !open }, contentPadding = PaddingValues(0.dp)) { Text("Pair by Address") }
        if (open) {
            val go = { if (address.isNotBlank()) pair(address.trim()) }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = address,
                    onValueChange = { address = it },
                    placeholder = { Text("Such as 192.168.1.20") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
                    keyboardActions = KeyboardActions(onGo = { go() }),
                    modifier = Modifier.weight(1f),
                )
                Button(onClick = go, enabled = address.isNotBlank()) { Text("Pair") }
            }
            Dim(
                "If the devices can't find each other, the network or a firewall may be in the way. Pochette needs " +
                    "TCP port ${syncPort()} to sync, and UDP port 5353 to find devices.",
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
        title = { Text("Pair With $name", textAlign = TextAlign.Center) },
        text = {
            Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Text(
                    when (pairing.stage) {
                        PairingStage.Connecting -> "Connecting…"
                        PairingStage.Confirm -> "Check that $name shows the same code, then pair on both."
                        else -> "Waiting for $name to confirm…"
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
        confirmButton = { if (confirming) TextButton(onClick = { answer(true) }) { Text("Pair") } },
        dismissButton = { if (confirming) TextButton(onClick = { answer(false) }) { Text("Cancel") } },
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
