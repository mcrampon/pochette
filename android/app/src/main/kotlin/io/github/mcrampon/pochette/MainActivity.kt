package io.github.mcrampon.pochette

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.widget.Toast
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
import io.github.mcrampon.pochette.core.findLink
import io.github.mcrampon.pochette.ui.SignedIn
import io.github.mcrampon.pochette.ui.PochetteTheme
import io.github.mcrampon.pochette.ui.SignInScreen
import io.github.mcrampon.pochette.ui.StartingScreen
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private val model: AppModel by viewModels()

    /** No browser could open the sign-in page. */
    private var browserMissing by mutableStateOf(false)

    // In the language picked, before Android 13, which does it itself.
    override fun attachBaseContext(newBase: Context) = super.attachBaseContext(Language.wrap(newBase))

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Again, as picking a language shows the activity again.
        Language.tell(this)
        enableEdgeToEdge()
        comeBackOnceRedirected()
        // Not again when shown again, as after picking a language: it's been opened already.
        if (savedInstanceState == null) openLink(intent)
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
                        // Offline, the downloads.
                        is Stage.SignedIn, Stage.Offline -> SignedIn(model, offline = current is Stage.Offline)
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        openLink(intent)
    }

    /** Open what the Qobuz link `intent` brings points to: opened, or shared to the app. */
    private fun openLink(intent: Intent) {
        val text =
            when (intent.action) {
                Intent.ACTION_VIEW -> intent.dataString
                Intent.ACTION_SEND -> intent.getStringExtra(Intent.EXTRA_TEXT)
                else -> null
            } ?: return
        val link = findLink(text)
        when {
            link != null -> model.openLink(link)
            // A link opened that isn't Qobuz's, as the one back from signing in, is for others.
            intent.action == Intent.ACTION_SEND -> Toast.makeText(this, R.string.not_a_qobuz_link, Toast.LENGTH_LONG).show()
        }
    }

    // On the network while the app is in front, to sync and find devices to pair.
    override fun onStart() {
        super.onStart()
        (application as PochetteApp).sync.hold()
    }

    override fun onStop() {
        (application as PochetteApp).sync.release()
        super.onStop()
    }

    private fun openSignInPage(url: String) {
        browserMissing = false
        try {
            CustomTabsIntent.Builder().build().launchUrl(this, url.toUri())
        } catch (missing: ActivityNotFoundException) {
            browserMissing = true
        }
    }

    /**
     * Once signed in on Qobuz's page, come back above it, in case the browser didn't follow
     * the link back to the app: Android may refuse, as the app is in the background.
     */
    private fun comeBackOnceRedirected() {
        lifecycleScope.launch {
            var signingIn = false
            model.stage.collect { stage ->
                if (stage is Stage.SignedOut && stage.url != null) {
                    signingIn = true
                } else if (signingIn) {
                    signingIn = false
                    if (stage is Stage.Starting) {
                        // singleTask: closes what's above the app in its task.
                        startActivity(Intent(this@MainActivity, MainActivity::class.java))
                    }
                }
            }
        }
    }
}
