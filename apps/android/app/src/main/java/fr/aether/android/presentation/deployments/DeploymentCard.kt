package fr.aether.android.presentation.deployments

import androidx.compose.animation.Crossfade
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import fr.aether.android.domain.model.DeploymentStatus
import fr.aether.android.domain.model.IamProvider

@Composable
fun DeploymentCard(
    deployment: DeploymentUiModel,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val statusLabel = deployment.status.displayLabel()
    Card(
        modifier = modifier
            .fillMaxWidth()
            .animateContentSize()
            .clickable(onClick = onClick)
            .semantics { contentDescription = "Deployment ${deployment.name}" },
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(18.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = deployment.name,
                        style = MaterialTheme.typography.titleLarge,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Spacer(modifier = Modifier.height(4.dp))
                    Text(
                        text = "${deployment.provider.displayName()} • ${deployment.cluster}",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
                Spacer(modifier = Modifier.width(12.dp))
                DeploymentStatusBadge(status = deployment.status)
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                EnvironmentBadge(environment = deployment.environment)
                Spacer(modifier = Modifier.weight(1f))
                Crossfade(
                    targetState = deployment.status == DeploymentStatus.DEPLOYING,
                    label = "deployment_updated"
                ) { isDeploying ->
                    Text(
                        text = if (isDeploying) "Deploying now" else "Updated ${deployment.updatedAt}",
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }

            Text(
                text = statusLabel,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

private fun DeploymentStatus.displayLabel(): String {
    return when (this) {
        DeploymentStatus.RUNNING -> "Running healthy"
        DeploymentStatus.DEPLOYING -> "Deploy in progress"
        DeploymentStatus.STOPPED -> "Paused"
        DeploymentStatus.FAILED -> "Requires attention"
    }
}

fun IamProvider.displayName(): String {
    return when (this) {
        IamProvider.KEYCLOAK -> "Keycloak"
        IamProvider.FERRISKEY -> "Ferriskey"
    }
}
