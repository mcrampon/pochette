package io.github.mcrampon.pochette.ui

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext

/** Material You's colors, from the wallpaper, as the desktop follows the accent color. */
@Composable
fun PochetteTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val colors =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val context = LocalContext.current
            if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        } else if (dark) {
            darkColorScheme()
        } else {
            lightColorScheme()
        }
    MaterialTheme(colorScheme = colors, content = content)
}
