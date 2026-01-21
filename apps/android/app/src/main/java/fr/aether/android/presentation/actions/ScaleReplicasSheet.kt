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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun ScaleReplicasAction(
    replicas: Int,
    minReplicas: Int,
    maxReplicas: Int,
    phase: ActionPhase,
    onScale: () -> Unit,
    modifier: Modifier = Modifier
) {
    ActionRow(
        modifier = modifier,
        icon = {
            Icon(
                imageVector = Icons.Outlined.Tune,
                contentDescription = null
            )
        },
        title = "Scale replicas",
        description = "Adjust replica count within safe limits.",
        trailing = {
            FilledTonalButton(
                onClick = onScale,
                enabled = phase != ActionPhase.InProgress
            ) {
                AnimatedContent(
                    targetState = phase,
                    transitionSpec = {
                        fadeIn(tween(160)) togetherWith fadeOut(tween(120))
                    },
                    label = "scale_label"
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
                            Text(text = "Scaling…")
                        }
                        ActionPhase.Success -> Text(text = "Scaled")
                        ActionPhase.Failure -> Text(text = "Retry")
                        ActionPhase.Idle -> Text(text = "Scale")
                    }
                }
            }
        }
    )
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun ScaleReplicasSheet(
    currentReplicas: Int,
    pendingReplicas: Int,
    minReplicas: Int,
    maxReplicas: Int,
    isScaling: Boolean,
    onValueChange: (Int) -> Unit,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit
) {
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp, vertical = 12.dp)
                .animateContentSize(),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text(
                text = "Scale replicas",
                style = MaterialTheme.typography.titleLarge
            )
            Text(
                text = "Current: $currentReplicas • Target: $pendingReplicas",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Slider(
                value = pendingReplicas.toFloat(),
                onValueChange = { onValueChange(it.toInt()) },
                valueRange = minReplicas.toFloat()..maxReplicas.toFloat(),
                steps = (maxReplicas - minReplicas - 1).coerceAtLeast(0)
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = "Min $minReplicas",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Text(
                    text = "Max $maxReplicas",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            FilledTonalButton(
                onClick = onConfirm,
                enabled = !isScaling,
                modifier = Modifier.fillMaxWidth()
            ) {
                if (isScaling) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(16.dp),
                            strokeWidth = 2.dp
                        )
                        Text(text = "Scaling…")
                    }
                } else {
                    Text(text = "Confirm scale")
                }
            }
            Spacer(modifier = Modifier.size(8.dp))
        }
    }
}
