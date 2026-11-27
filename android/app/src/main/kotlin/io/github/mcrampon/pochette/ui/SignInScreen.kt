package io.github.mcrampon.pochette.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.res.stringResource
import io.github.mcrampon.pochette.R

/**
 * Signing in, on Qobuz's own page: `url` while it's open, `error` if the last try failed.
 * `browserMissing` when no browser could open it: then the page's address is shown.
 */
@Composable
fun SignInScreen(
    url: String?,
    error: String?,
    browserMissing: Boolean,
    onSignIn: () -> Unit,
    onCancel: () -> Unit,
) {
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterVertically),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(stringResource(R.string.app_name), style = MaterialTheme.typography.displaySmall)
        Text(
            stringResource(R.string.sign_in_tagline),
            style = MaterialTheme.typography.bodyLarge,
            textAlign = TextAlign.Center,
        )
        if (url == null) {
            Button(onClick = onSignIn) { Text(stringResource(R.string.sign_in)) }
        } else {
            CircularProgressIndicator()
            Text(stringResource(R.string.sign_in_waiting), textAlign = TextAlign.Center)
            if (browserMissing) {
                Text(stringResource(R.string.sign_in_no_browser), textAlign = TextAlign.Center)
                SelectionContainer { Text(url, style = MaterialTheme.typography.bodySmall) }
            }
            TextButton(onClick = onCancel) { Text(stringResource(R.string.cancel)) }
        }
        if (error != null) {
            Text(
                error,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodyMedium,
                textAlign = TextAlign.Center,
            )
        }
    }
}

/** Starting, or retrying when Qobuz couldn't be reached. */
@Composable
fun StartingScreen(error: String?, onRetry: () -> Unit) {
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterVertically),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        if (error == null) {
            CircularProgressIndicator()
        } else {
            Text(stringResource(R.string.no_connection), style = MaterialTheme.typography.titleLarge)
            Text(error, style = MaterialTheme.typography.bodyMedium, textAlign = TextAlign.Center)
            Button(onClick = onRetry) { Text(stringResource(R.string.try_again)) }
        }
    }
}
