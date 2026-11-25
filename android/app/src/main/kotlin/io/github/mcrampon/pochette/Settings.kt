package io.github.mcrampon.pochette

import android.content.Context
import android.content.SharedPreferences
import androidx.core.content.edit
import io.github.mcrampon.pochette.core.Quality

/** What the user chose, kept on the phone. */
class Settings(context: Context) {
    private val prefs = context.getSharedPreferences("settings", Context.MODE_PRIVATE)

    /** The best quality to stream in: Qobuz sends the best the track and the plan allow, up to it. */
    var quality: Quality
        get() = prefs.getString(QUALITY, null)?.let { saved -> Quality.entries.find { it.name == saved } } ?: Quality.HI_RES192
        set(value) = prefs.edit { putString(QUALITY, value.name) }

    /** Call `action` when the quality changes: keep what this returns for as long as it should be called. */
    fun onQualityChanged(action: () -> Unit): SharedPreferences.OnSharedPreferenceChangeListener =
        SharedPreferences.OnSharedPreferenceChangeListener { _, key -> if (key == QUALITY) action() }
            .also(prefs::registerOnSharedPreferenceChangeListener)

    private companion object {
        const val QUALITY = "quality"
    }
}
