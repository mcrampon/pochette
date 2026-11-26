package io.github.mcrampon.pochette

import android.content.Context
import android.net.wifi.WifiManager
import android.util.Log
import io.github.mcrampon.pochette.core.Observer
import io.github.mcrampon.pochette.core.Pochette
import io.github.mcrampon.pochette.core.PochetteException
import io.github.mcrampon.pochette.core.SyncStatus
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/**
 * Syncing the plays with the user's other devices on the same network: the core's engine, on the network while
 * something holds it, as the app in front, the playback service or the background job.
 */
class Sync(context: Context, private val core: Pochette, private val scope: CoroutineScope) {
    // Wi-Fi drops multicast packets without it, to save power: devices find each other with mDNS.
    private val multicast =
        context.getSystemService(WifiManager::class.java).createMulticastLock("Pochette sync").apply { setReferenceCounted(true) }

    private val _status = MutableStateFlow(core.syncStatus())
    val status: StateFlow<SyncStatus> = _status.asStateFlow()

    /** Goes up each time the plays change, here or as another device's arrive: what shows them asks again. */
    private val _plays = MutableStateFlow(0)
    val plays: StateFlow<Int> = _plays.asStateFlow()

    init {
        core.observe(
            object : Observer {
                override fun syncChanged(status: SyncStatus) {
                    _status.value = status
                }

                override fun playsChanged() {
                    _plays.update { it + 1 }
                }
            },
        )
    }

    /** Be on the network, until as many [release]s. The engine stays on once started: it costs little while idle. */
    fun hold() {
        multicast.acquire()
        scope.launch {
            try {
                core.syncStart()
            } catch (err: PochetteException) {
                Log.w(TAG, "can't sync: ${err.message}")
            }
        }
    }

    fun release() {
        if (multicast.isHeld) multicast.release()
    }

    private companion object {
        const val TAG = "Pochette"
    }
}
