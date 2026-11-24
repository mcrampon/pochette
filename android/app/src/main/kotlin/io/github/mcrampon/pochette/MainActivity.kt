package io.github.mcrampon.pochette

import android.content.ActivityNotFoundException
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.material3.Surface
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.fillMaxSize
import androidx.core.net.toUri
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import io.github.mcrampon.pochette.ui.AlbumsScreen
import io.github.mcrampon.pochette.ui.PochetteTheme
import io.github.mcrampon.pochette.ui.SignInScreen
import io.github.mcrampon.pochette.ui.StartingScreen
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private val model: AppModel by viewModels()

    /** No browser could open the sign-in page. */
    private var browserMissing by mutableStateOf(false)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        comeBackOnceSignedIn()
        setContent {
            PochetteTheme {
                Surface(Modifier.fillMaxSize()) {
                    val stage by model.stage.collectAsStateWithLifecycle()
                    when (val current = stage) {
                        Stage.Starting -> StartingScreen(error = null, onRetry = {})
                        is Stage.Unreachable -> StartingScreen(current.error, onRetry = model::resume)
                        is Stage.SignedOut ->
                            SignInScreen(
                                url = current.url,
                                error = current.error,
                                browserMissing = browserMissing,
                                onSignIn = { model.signIn(::openSignInPage) },
                                onCancel = model::cancelSignIn,
                            )
                        is Stage.SignedIn -> {
                            val library by model.library.collectAsStateWithLifecycle()
                            AlbumsScreen(library, onRefresh = model::refresh, onSignOut = model::signOut)
                        }
                    }
                }
            }
        }
    }

    private fun openSignInPage(url: String) {
        browserMissing = false
        try {
            CustomTabsIntent.Builder().build().launchUrl(this, url.toUri())
        } catch (missing: ActivityNotFoundException) {
            browserMissing = true
        }
    }

    /** Once signed in, close the sign-in page, which is above the app, and show the albums. */
    private fun comeBackOnceSignedIn() {
        lifecycleScope.launch {
            var signingIn = false
            model.stage.collect { stage ->
                if (stage is Stage.SignedOut && stage.url != null) {
                    signingIn = true
                } else if (stage is Stage.SignedIn && signingIn) {
                    signingIn = false
                    startActivity(
                        Intent(this@MainActivity, MainActivity::class.java)
                            .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP),
                    )
                }
            }
        }
    }
}
