package fr.aether.android.presentation.auth

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.expandVertically
import androidx.compose.animation.shrinkVertically
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LoadingIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import fr.aether.android.ui.theme.AndroidTheme
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.ImeAction

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LoginScreen(
    uiState: LoginUiState,
    onPasswordLogin: (String, String) -> Unit
) {
    var username by rememberSaveable { mutableStateOf("") }
    var password by rememberSaveable { mutableStateOf("") }
    var showPassword by remember { mutableStateOf(false) }

    Scaffold { padding ->
        val colorScheme = MaterialTheme.colorScheme
        val isBlocking = uiState is LoginUiState.Loading ||
            uiState is LoginUiState.AwaitingCallback ||
            uiState is LoginUiState.Launching
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .background(colorScheme.surface)
                .padding(24.dp)
        ) {
            LoginCard(
                uiState = uiState,
                username = username,
                onUsernameChanged = { username = it },
                password = password,
                onPasswordChanged = { password = it },
                showPassword = showPassword,
                onTogglePassword = { showPassword = !showPassword },
                onPasswordLogin = onPasswordLogin,
                enabled = !isBlocking
            )

            AnimatedVisibility(
                visible = isBlocking,
                enter = fadeIn(),
                exit = fadeOut()
            ) {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(colorScheme.surface.copy(alpha = 0.7f)),
                    contentAlignment = Alignment.Center
                ) {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        LoadingIndicator()
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = "Signing you in…",
                            style = MaterialTheme.typography.bodyMedium,
                            color = colorScheme.onSurfaceVariant
                        )
                    }
                }
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
    username: String,
    onUsernameChanged: (String) -> Unit,
    password: String,
    onPasswordChanged: (String) -> Unit,
    showPassword: Boolean,
    onTogglePassword: () -> Unit,
    onPasswordLogin: (String, String) -> Unit,
    enabled: Boolean
) {
    val colorScheme = MaterialTheme.colorScheme
    Column(
        modifier = Modifier
            .fillMaxSize(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.Start
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
                .widthIn(max = 420.dp)
                .animateContentSize(),
            colors = CardDefaults.cardColors(
                containerColor = colorScheme.surfaceContainerLow
            ),
            elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
            shape = MaterialTheme.shapes.extraLarge
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(20.dp),
                horizontalAlignment = Alignment.Start
            ) {
                Text(
                    text = "Sign in to continue.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = colorScheme.onSurfaceVariant
                )
                Spacer(modifier = Modifier.height(12.dp))
                OutlinedTextField(
                    value = username,
                    onValueChange = onUsernameChanged,
                    label = { Text(text = "Username") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                    enabled = enabled,
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Email,
                        imeAction = ImeAction.Next
                    )
                )
                Spacer(modifier = Modifier.height(12.dp))
                OutlinedTextField(
                    value = password,
                    onValueChange = onPasswordChanged,
                    label = { Text(text = "Password") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                    enabled = enabled,
                    visualTransformation = if (showPassword) {
                        VisualTransformation.None
                    } else {
                        PasswordVisualTransformation()
                    },
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Password,
                        imeAction = ImeAction.Done
                    ),
                    trailingIcon = {
                        Text(
                            text = if (showPassword) "Hide" else "Show",
                            style = MaterialTheme.typography.labelLarge,
                            color = colorScheme.primary,
                            modifier = Modifier
                                .padding(horizontal = 8.dp, vertical = 4.dp)
                                .clickable(onClick = onTogglePassword)
                        )
                    }
                )
                Spacer(modifier = Modifier.height(16.dp))
                Button(
                    onClick = { onPasswordLogin(username, password) },
                    enabled = username.isNotBlank() &&
                        password.isNotBlank() &&
                        enabled,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(text = "Sign in")
                }
                Spacer(modifier = Modifier.height(16.dp))
                AnimatedVisibility(
                    visible = uiState is LoginUiState.Error,
                    enter = fadeIn() + expandVertically(),
                    exit = fadeOut() + shrinkVertically()
                ) {
                    Text(
                        text = (uiState as? LoginUiState.Error)?.message.orEmpty(),
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
            onPasswordLogin = { _, _ -> }
        )
    }
}
