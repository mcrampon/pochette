package io.github.mcrampon.pochette

import android.app.Activity
import android.app.LocaleManager
import android.content.Context
import android.content.ContextWrapper
import android.content.res.Configuration
import android.os.Build
import android.os.LocaleList
import io.github.mcrampon.pochette.core.setLanguages
import java.util.Locale

/**
 * The language the app shows, the phone's unless the user picked another. From Android 13, Android keeps the
 * choice and applies it to the whole app, as its own settings may change it too; before, the app keeps it, and
 * its activity and service show it through [wrap].
 */
object Language {
    /** The languages to pick from, by BCP 47 tag, and by their own names: they're never translated. */
    val choices =
        listOf(
            "de" to "Deutsch",
            "en-GB" to "English (UK)",
            "en-US" to "English (US)",
            "es" to "Español",
            "fr" to "Français",
            "fr-CA" to "Français (Canada)",
            "it" to "Italiano",
            "pl" to "Polski",
            "pt-BR" to "Português (Brasil)",
            "zh-Hans" to "中文（简体）",
        )

    /** The tag of the language picked, none to follow the phone. */
    fun chosen(context: Context): String? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.getSystemService(LocaleManager::class.java).applicationLocales.takeUnless { it.isEmpty }?.get(0)?.toLanguageTag()
        } else {
            Settings(context).language
        }

    /** The language of `tag` by its own name, as the settings show it. */
    fun name(tag: String): String =
        choices.find { it.first.equals(tag, ignoreCase = true) }?.second
            ?: Locale.forLanguageTag(tag).let { it.getDisplayName(it) }

    /** Show the language of `tag`, or the phone's when none: the activity of `context` shows it again. */
    fun choose(context: Context, tag: String?) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            // Android shows the activity again itself.
            context.getSystemService(LocaleManager::class.java).applicationLocales =
                if (tag == null) LocaleList.getEmptyLocaleList() else LocaleList.forLanguageTags(tag)
        } else {
            Settings(context).language = tag
            context.activity()?.recreate()
        }
    }

    /** `context` in the language picked: Android does it itself from Android 13. */
    fun wrap(context: Context): Context {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) return context
        val tag = Settings(context).language ?: return context
        val configuration = Configuration(context.resources.configuration).apply { setLocales(LocaleList.forLanguageTags(tag)) }
        return context.createConfigurationContext(configuration)
    }

    /** Have the core say what it says, as durations and dates, in the languages `context` shows. */
    fun tell(context: Context) {
        val locales = wrap(context).resources.configuration.locales
        setLanguages((0 until locales.size()).map { locales[it].toLanguageTag() })
    }

    private tailrec fun Context.activity(): Activity? =
        when (this) {
            is Activity -> this
            is ContextWrapper -> baseContext.activity()
            else -> null
        }
}
