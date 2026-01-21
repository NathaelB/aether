package fr.aether.android.presentation.actions

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.RestartAlt
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun RestartAction(
    phase: ActionPhase,
    onRestart: () -> Unit,
    modifier: Modifier = Modifier
) {
    val showConfirm = rememberSaveable { mutableStateOf(false) }
    ActionRow(
        modifier = modifier,
        icon = {
            Icon(
                imageVector = Icons.Outlined.RestartAlt,
                contentDescription = null
            )
        },
        title = "Restart deployment",
        description = "Safely roll the pods to apply a clean restart.",
        trailing = {
            FilledTonalButton(
                onClick = { showConfirm.value = true },
                enabled = phase != ActionPhase.InProgress
            ) {
                AnimatedContent(
                    targetState = phase,
                    transitionSpec = {
                        fadeIn(tween(160)) togetherWith fadeOut(tween(120))
                    },
                    label = "restart_label"
                ) { target ->
                    when (target) {
                        ActionPhase.InProgress -> Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            CircularProgressIndicator(
                                modifier = Modifier.size(16.dp),
                                strokeWidth = 2.dp
                            )
                            Text(text = "Restarting…")
                        }
                        ActionPhase.Success -> Text(text = "Restarted")
                        ActionPhase.Failure -> Text(text = "Retry")
                        ActionPhase.Idle -> Text(text = "Restart")
                    }
                }
            }
        }
    )
    if (showConfirm.value) {
        AlertDialog(
            onDismissRequest = { showConfirm.value = false },
            confirmButton = {
                FilledTonalButton(
                    onClick = {
                        showConfirm.value = false
                        onRestart()
                    }
                ) {
                    Text(text = "Confirm restart")
                }
            },
            dismissButton = {
                androidx.compose.material3.TextButton(
                    onClick = { showConfirm.value = false }
                ) {
                    Text(text = "Cancel")
                }
            },
            title = {
                Text(text = "Restart deployment?")
            },
            text = {
                Text(text = "This will restart pods safely and may cause brief disruption.")
            }
        )
    }
}

@Composable
fun ActionRow(
    icon: @Composable () -> Unit,
    title: String,
    description: String,
    trailing: @Composable () -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .animateContentSize(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Column(
            verticalArrangement = Arrangement.spacedBy(4.dp),
            modifier = Modifier.weight(1f)
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                icon()
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleMedium
                )
            }
            Text(
                text = description,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        Spacer(modifier = Modifier.size(8.dp))
        trailing()
    }
}
