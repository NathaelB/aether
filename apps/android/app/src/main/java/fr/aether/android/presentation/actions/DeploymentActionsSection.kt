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
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun DeploymentActionsSection(
    state: DeploymentActionState,
    onRestartClick: () -> Unit,
    onScaleClick: () -> Unit,
    onToggleMaintenance: () -> Unit,
    modifier: Modifier = Modifier
) {
    Card(
        modifier = modifier
            .fillMaxWidth()
            .animateContentSize(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainer
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text(
                text = "Actions",
                style = MaterialTheme.typography.titleLarge
            )
            RestartAction(
                phase = state.restartPhase,
                onRestart = onRestartClick
            )
            ScaleReplicasAction(
                replicas = state.replicas,
                minReplicas = state.minReplicas,
                maxReplicas = state.maxReplicas,
                phase = state.scalePhase,
                onScale = onScaleClick
            )
            MaintenanceToggle(
                enabled = state.maintenanceEnabled,
                phase = state.maintenancePhase,
                onToggle = onToggleMaintenance
            )
            AnimatedContent(
                targetState = state.feedback,
                transitionSpec = {
                    fadeIn(tween(180)) togetherWith fadeOut(tween(140))
                },
                label = "actions_feedback"
            ) { feedback ->
                if (feedback != null) {
                    val tone = when (feedback.phase) {
                        ActionPhase.Success -> MaterialTheme.colorScheme.primary
                        ActionPhase.Failure -> MaterialTheme.colorScheme.error
                        ActionPhase.InProgress -> MaterialTheme.colorScheme.onSurfaceVariant
                        ActionPhase.Idle -> MaterialTheme.colorScheme.onSurfaceVariant
                    }
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Spacer(modifier = Modifier.size(8.dp))
                        Text(
                            text = feedback.message,
                            style = MaterialTheme.typography.bodySmall,
                            color = tone
                        )
                    }
                }
            }
        }
    }
}
