package io.github.mcrampon.pochette

import android.content.Context
import android.content.SharedPreferences
import androidx.core.content.edit
import io.github.mcrampon.pochette.core.LibraryView
import io.github.mcrampon.pochette.core.Quality
import io.github.mcrampon.pochette.core.Sort

/** What the user chose, kept on the phone. */
class Settings(context: Context) {
    private val prefs = context.getSharedPreferences("settings", Context.MODE_PRIVATE)

    /** The best quality to stream in: Qobuz sends the best the track and the plan allow, up to it. */
    var quality: Quality
        get() = prefs.getString(QUALITY, null)?.let { saved -> Quality.entries.find { it.name == saved } } ?: Quality.HI_RES192
        set(value) = prefs.edit { putString(QUALITY, value.name) }

    /** The genres Discover shows, by Qobuz id: all when none. */
    var discoverGenres: List<ULong>
        get() = prefs.getString(DISCOVER_GENRES, null).orEmpty().split(',').mapNotNull { it.toULongOrNull() }
        set(value) = prefs.edit { putString(DISCOVER_GENRES, value.joinToString(",")) }

    /** Whether to sync with the paired devices while the app is closed, from time to time. */
    var syncInBackground: Boolean
        get() = prefs.getBoolean(SYNC_IN_BACKGROUND, true)
        set(value) = prefs.edit { putBoolean(SYNC_IN_BACKGROUND, value) }

    /** The language picked, by BCP 47 tag, none for the phone's: kept here before Android 13 only, which keeps it itself. */
    var language: String?
        get() = prefs.getString(LANGUAGE, null)
        set(value) = prefs.edit { putString(LANGUAGE, value) }

    /** How the library sorts and which genres and decades it shows: the search isn't kept. */
    var libraryView: LibraryView
        get() =
            LibraryView(
                query = "",
                genres = prefs.getStringSet(LIBRARY_GENRES, null).orEmpty().sorted(),
                decades = prefs.getString(LIBRARY_DECADES, null).orEmpty().split(',').mapNotNull { it.toUShortOrNull() },
                sort = prefs.getString(LIBRARY_SORT, null)?.let { saved -> Sort.entries.find { it.name == saved } } ?: Sort.DATE_ADDED,
                reversed = prefs.getBoolean(LIBRARY_REVERSED, false),
            )
        set(value) =
            prefs.edit {
                putStringSet(LIBRARY_GENRES, value.genres.toSet())
                putString(LIBRARY_DECADES, value.decades.joinToString(","))
                putString(LIBRARY_SORT, value.sort.name)
                putBoolean(LIBRARY_REVERSED, value.reversed)
            }

    /** Call `action` when the quality changes: keep what this returns for as long as it should be called. */
    fun onQualityChanged(action: () -> Unit): SharedPreferences.OnSharedPreferenceChangeListener =
        SharedPreferences.OnSharedPreferenceChangeListener { _, key -> if (key == QUALITY) action() }
            .also(prefs::registerOnSharedPreferenceChangeListener)

    private companion object {
        const val QUALITY = "quality"
        const val DISCOVER_GENRES = "discover_genres"
        const val SYNC_IN_BACKGROUND = "sync_in_background"
        const val LANGUAGE = "language"
        const val LIBRARY_GENRES = "library_genres"
        const val LIBRARY_DECADES = "library_decades"
        const val LIBRARY_SORT = "library_sort"
        const val LIBRARY_REVERSED = "library_reversed"
    }
}
