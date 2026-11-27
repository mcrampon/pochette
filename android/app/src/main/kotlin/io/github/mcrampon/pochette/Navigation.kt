package io.github.mcrampon.pochette

import androidx.annotation.StringRes
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import io.github.mcrampon.pochette.core.AlbumCard
import io.github.mcrampon.pochette.core.ReleaseKind

/** A screen of the signed-in app, in a tab's stack. */
sealed interface Screen {
    /** Tells apart the screens of a stack, so each keeps its scroll and what it loaded. */
    val key: String

    data object Albums : Screen {
        override val key = "albums"
    }

    data object Discover : Screen {
        override val key = "discover"
    }

    data object Search : Screen {
        override val key = "search"
    }

    data object Stats : Screen {
        override val key = "stats"
    }

    data object Settings : Screen {
        override val key = "settings"
    }

    data object NowPlaying : Screen {
        override val key = "now"
    }

    /** `preview`, when known, shows while the page loads. */
    data class Album(val id: String, val preview: AlbumCard? = null) : Screen {
        override val key = "album/$id"
    }

    data class Artist(val id: ULong, val name: String) : Screen {
        override val key = "artist/$id"
    }

    /** All the albums of a Discover section, in the genres picked. */
    data class Section(val section: io.github.mcrampon.pochette.core.Section, val genres: List<ULong>) : Screen {
        override val key = "section/$section/${genres.joinToString(",")}"
    }

    /** All the releases of an artist of one kind. */
    data class Releases(val artistId: ULong, val artistName: String, val kind: ReleaseKind) : Screen {
        override val key = "releases/$artistId/$kind"
    }
}

/** The tabs, at the bottom: each keeps its own stack of screens. */
enum class Tab(@StringRes val label: Int, val root: Screen, val icon: Int) {
    Albums(R.string.tab_albums, Screen.Albums, R.drawable.ic_album),
    Discover(R.string.tab_discover, Screen.Discover, R.drawable.ic_explore),
    Search(R.string.tab_search, Screen.Search, R.drawable.ic_search),
    Stats(R.string.tab_stats, Screen.Stats, R.drawable.ic_bar_chart),
}

/** The tab shown, and each tab's stack, the last screen on top. */
class Navigation {
    var tab by mutableStateOf(Tab.Albums)
        private set
    private val stacks = Tab.entries.associateWith { mutableStateListOf(it.root) }

    /** The shown tab's stack. */
    val screens: List<Screen> get() = stacks.getValue(tab)

    val top: Screen get() = screens.last()

    /** Where the screen on top keeps its state: unique across tabs. */
    val topKey: String get() = keyOf(tab, screens.lastIndex)

    /** Whether back stays in the app. */
    val canGoBack: Boolean get() = screens.size > 1 || tab != Tab.Albums

    fun open(screen: Screen) {
        if (top.key != screen.key) stacks.getValue(tab).add(screen)
    }

    /** Show `tab`; when it's shown already, go back to its first screen. Returns the keys of the screens closed. */
    fun select(tab: Tab): List<String> {
        if (tab != this.tab) {
            this.tab = tab
            return emptyList()
        }
        val stack = stacks.getValue(tab)
        val closed = (1 until stack.size).map { keyOf(tab, it) }
        stack.removeRange(1, stack.size)
        return closed
    }

    /** Close the screen on top, or go back to Albums from another tab's first screen. Returns the key of the screen closed. */
    fun back(): String? {
        val stack = stacks.getValue(tab)
        if (stack.size > 1) {
            val key = keyOf(tab, stack.lastIndex)
            stack.removeAt(stack.lastIndex)
            return key
        }
        if (tab != Tab.Albums) tab = Tab.Albums
        return null
    }

    /** Back to each tab's first screen, on Albums. */
    fun reset() {
        stacks.values.forEach { it.removeRange(1, it.size) }
        tab = Tab.Albums
    }

    private fun keyOf(tab: Tab, index: Int) = "${tab.name}/$index/${stacks.getValue(tab)[index].key}"
}
