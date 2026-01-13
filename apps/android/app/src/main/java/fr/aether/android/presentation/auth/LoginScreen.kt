package fr.aether.android.presentation.auth

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import fr.aether.android.presentation.components.SpinningProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.graphics.Brush
import fr.aether.android.ui.theme.AndroidTheme

@Composable
fun LoginScreen(
    uiState: LoginUiState,
    onLoginClicked: () -> Unit,
    onAuthLaunched: () -> Unit
) {
    val context = LocalContext.current

    if (uiState is LoginUiState.Launching) {
        LaunchedEffect(uiState.request.authorizationUrl) {
            val uri = Uri.parse(uiState.request.authorizationUrl)
            val intent = CustomTabsIntent.Builder().build()
            intent.launchUrl(context, uri)
            onAuthLaunched()
        }
    }

    Scaffold { padding ->
        val colorScheme = MaterialTheme.colorScheme
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .background(
                    Brush.verticalGradient(
                        colors = listOf(
                            colorScheme.surface,
                            colorScheme.surfaceVariant
                        )
                    )
                )
                .padding(24.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Text(
                text = "Aether Deployments",
                style = MaterialTheme.typography.headlineMedium
            )
            Spacer(modifier = Modifier.height(12.dp))
            Text(
                text = "Secure access to your environments.",
                style = MaterialTheme.typography.bodyLarge
            )
            Spacer(modifier = Modifier.height(24.dp))
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(
                    containerColor = colorScheme.surfaceContainer
                )
            ) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(20.dp),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    Text(
                        text = "Sign in with Keycloak to continue.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = colorScheme.onSurfaceVariant
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    Button(
                        onClick = onLoginClicked,
                        enabled = uiState !is LoginUiState.Loading &&
                            uiState !is LoginUiState.AwaitingCallback &&
                            uiState !is LoginUiState.Launching,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text(text = "Sign in")
                    }
                    Spacer(modifier = Modifier.height(16.dp))
                    when (uiState) {
                        LoginUiState.Loading -> SpinningProgressIndicator()
                        LoginUiState.AwaitingCallback -> Text(
                            text = "Waiting for authorization...",
                            style = MaterialTheme.typography.bodyMedium
                        )
                        is LoginUiState.Error -> Text(
                            text = uiState.message,
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.bodyMedium
                        )
                        else -> Unit
                    }
                }
            }
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun LoginScreenPreview() {
    AndroidTheme {
        LoginScreen(
            uiState = LoginUiState.Idle,
            onLoginClicked = {},
            onAuthLaunched = {}
        )
    }
}
