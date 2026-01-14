package fr.aether.android.presentation.deployments

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import fr.aether.android.domain.model.DeploymentStatus
import fr.aether.android.domain.model.IamProvider
import fr.aether.android.ui.theme.AndroidTheme
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import fr.aether.android.presentation.observability.ObservabilitySection
import fr.aether.android.presentation.observability.ObservabilityViewModel

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun DeploymentDetailScreen(
    deployment: DeploymentUiModel?,
    isLoading: Boolean = false,
    modifier: Modifier = Modifier
) {
    if (isLoading) {
        Column(
            modifier = modifier
                .fillMaxSize()
                .padding(24.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            LoadingIndicator()
            Spacer(modifier = Modifier.height(12.dp))
            Text(
                text = "Loading deployment details...",
                style = MaterialTheme.typography.bodyLarge
            )
        }
        return
    }
    if (deployment == null) {
        Column(
            modifier = modifier
                .fillMaxSize()
                .padding(24.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Text(
                text = "Deployment not found.",
                style = MaterialTheme.typography.bodyLarge
            )
        }
        return
    }

    val observabilityViewModel: ObservabilityViewModel = viewModel()
    val observabilityState by observabilityViewModel.uiState.collectAsStateWithLifecycle()

    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.surfaceContainerHigh
            ),
            elevation = CardDefaults.cardElevation(defaultElevation = 2.dp),
            shape = MaterialTheme.shapes.large
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = deployment.name,
                        style = MaterialTheme.typography.titleLarge,
                        modifier = Modifier.weight(1f),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    DeploymentStatusBadge(status = deployment.status)
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = deployment.provider.displayName(),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.weight(1f)
                    )
                    EnvironmentBadge(environment = deployment.environment)
                }
            }
        }

        InfoCard(
            title = "Cluster",
            lines = listOf(
                "Cluster: ${deployment.cluster}",
                "Namespace: ${deployment.namespace}",
                "Region: ${deployment.region}"
            )
        )
        InfoCard(
            title = "Runtime",
            lines = listOf(
                "Version: ${deployment.version}",
                "Endpoint: ${deployment.endpoint}",
                "Updated: ${deployment.updatedAt}"
            )
        )
        ObservabilitySection(
            uiState = observabilityState,
            onRetry = observabilityViewModel::refresh
        )
    }
}

@Composable
private fun InfoCard(
    title: String,
    lines: List<String>
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = 1.dp),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium
            )
            lines.forEach { line ->
                Text(
                    text = line,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun DeploymentDetailScreenPreview() {
    AndroidTheme {
        DeploymentDetailScreen(
            deployment = DeploymentUiModel(
                id = "dep-1",
                name = "Keycloak - Core",
                environment = "Production",
                status = DeploymentStatus.RUNNING,
                provider = IamProvider.KEYCLOAK,
                cluster = "iam-prod-01",
                namespace = "keycloak",
                version = "24.0.2",
                endpoint = "https://iam.aether.io",
                region = "eu-west-1",
                updatedAt = "2024-08-12 10:24"
            )
        )
    }
}
