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
import androidx.browser.customtabs.CustomTabsClient
import androidx.browser.customtabs.CustomTabsIntent
import androidx.browser.customtabs.CustomTabsServiceConnection
import androidx.browser.customtabs.CustomTabsSession
import android.content.Intent
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LoadingIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.graphics.Brush
import fr.aether.android.ui.theme.AndroidTheme
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.Box

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LoginScreen(
    uiState: LoginUiState,
    onLoginClicked: () -> Unit,
    onAuthLaunched: () -> Unit
) {
    val context = LocalContext.current
    var customTabsSession by remember { mutableStateOf<CustomTabsSession?>(null) }
    val packageName = remember { CustomTabsClient.getPackageName(context, null) }

    DisposableEffect(packageName) {
        if (packageName == null) {
            onDispose { }
        } else {
            val connection = object : CustomTabsServiceConnection() {
                override fun onCustomTabsServiceConnected(
                    name: android.content.ComponentName,
                    client: CustomTabsClient
                ) {
                    client.warmup(0L)
                    customTabsSession = client.newSession(null)
                }

                override fun onServiceDisconnected(name: android.content.ComponentName) {
                    customTabsSession = null
                }
            }
            CustomTabsClient.bindCustomTabsService(context, packageName, connection)
            onDispose {
                context.unbindService(connection)
                customTabsSession = null
            }
        }
    }

    if (uiState is LoginUiState.Launching) {
        LaunchedEffect(uiState.request.authorizationUrl) {
            val uri = Uri.parse(uiState.request.authorizationUrl)
            customTabsSession?.mayLaunchUrl(uri, null, null)
            val intent = CustomTabsIntent.Builder()
                .setShowTitle(false)
                .setUrlBarHidingEnabled(true)
                .setShareState(CustomTabsIntent.SHARE_STATE_OFF)
                .setStartAnimations(context, android.R.anim.fade_in, android.R.anim.fade_out)
                .setExitAnimations(context, android.R.anim.fade_in, android.R.anim.fade_out)
                .build()
            intent.intent.addFlags(Intent.FLAG_ACTIVITY_NO_HISTORY)
            intent.launchUrl(context, uri)
            onAuthLaunched()
        }
    }

    Scaffold { padding ->
        val colorScheme = MaterialTheme.colorScheme
        val isBlocking = uiState is LoginUiState.Loading ||
            uiState is LoginUiState.AwaitingCallback ||
            uiState is LoginUiState.Launching
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .background(
                    Brush.verticalGradient(
                        colors = listOf(
                            colorScheme.primaryContainer,
                            colorScheme.surface
                        )
                    )
                )
                .padding(24.dp)
        ) {
            if (isBlocking) {
                AuthSplash()
            } else {
                LoginCard(
                    uiState = uiState,
                    onLoginClicked = onLoginClicked
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun AuthSplash() {
    val colorScheme = MaterialTheme.colorScheme
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(
            text = "Aether Deployments",
            style = MaterialTheme.typography.displaySmall,
            color = colorScheme.onSurface
        )
        Spacer(modifier = Modifier.height(12.dp))
        LoadingIndicator()
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Completing secure authentication…",
            style = MaterialTheme.typography.bodyMedium,
            color = colorScheme.onSurfaceVariant
        )
    }
}

@Composable
private fun LoginCard(
    uiState: LoginUiState,
    onLoginClicked: () -> Unit
) {
    val colorScheme = MaterialTheme.colorScheme
    Column(
        modifier = Modifier
            .fillMaxSize(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(
            text = "Aether Deployments",
            style = MaterialTheme.typography.displaySmall,
            color = colorScheme.onSurface
        )
        Spacer(modifier = Modifier.height(12.dp))
        Text(
            text = "Secure access to your environments.",
            style = MaterialTheme.typography.bodyLarge,
            color = colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(24.dp))
        Card(
            modifier = Modifier
                .fillMaxWidth()
                .widthIn(max = 420.dp),
            colors = CardDefaults.cardColors(
                containerColor = colorScheme.surfaceContainerHigh
            ),
            elevation = CardDefaults.cardElevation(defaultElevation = 4.dp),
            shape = MaterialTheme.shapes.extraLarge
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
                if (uiState is LoginUiState.Error) {
                    Text(
                        text = uiState.message,
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodyMedium
                    )
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
