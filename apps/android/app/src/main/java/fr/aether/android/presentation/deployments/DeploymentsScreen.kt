package fr.aether.android.presentation.deployments

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.ExperimentalMaterialApi
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.material.pullrefresh.PullRefreshIndicator
import androidx.compose.material.pullrefresh.pullRefresh
import androidx.compose.material.pullrefresh.rememberPullRefreshState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import fr.aether.android.domain.model.DeploymentStatus
import fr.aether.android.domain.model.IamProvider
import fr.aether.android.presentation.components.SpinningProgressIndicator
import fr.aether.android.ui.theme.AndroidTheme
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Security
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LoadingIndicator
import fr.aether.android.presentation.components.ExpressiveCircularLoadingIndicator
import fr.aether.android.presentation.components.ExpressiveLinearLoadingIndicator

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterialApi::class)
@Composable
fun DeploymentsScreen(
    uiState: DeploymentsUiState,
    onRefresh: () -> Unit,
    onDeploymentClick: (DeploymentUiModel) -> Unit,
    modifier: Modifier = Modifier
) {
    val isRefreshing = (uiState as? DeploymentsUiState.Success)?.isRefreshing == true
    val pullRefreshState = rememberPullRefreshState(
        refreshing = isRefreshing,
        onRefresh = onRefresh
    )

    Box(
        modifier = modifier
            .fillMaxSize()
            .pullRefresh(pullRefreshState)
    ) {
        when (uiState) {
            DeploymentsUiState.Loading -> LoadingState()
            is DeploymentsUiState.Error -> ErrorState(message = uiState.message)
            is DeploymentsUiState.Success -> DeploymentsList(
                deployments = uiState.deployments,
                onDeploymentClick = onDeploymentClick
            )
        }

        PullRefreshIndicator(
            refreshing = isRefreshing,
            state = pullRefreshState,
            modifier = Modifier.align(Alignment.TopCenter)
        )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun LoadingState() {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        LoadingIndicator()
        Spacer(modifier = Modifier.height(12.dp))
        Text(
            text = "Loading IAM deployments...",
            style = MaterialTheme.typography.bodyLarge
        )
    }
}

@Composable
private fun ErrorState(message: String) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(
            text = message,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.error
        )
    }
}

@Composable
private fun DeploymentsList(
    deployments: List<DeploymentUiModel>,
    onDeploymentClick: (DeploymentUiModel) -> Unit
) {
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        items(deployments, key = { it.id }) { deployment ->
            DeploymentCard(
                deployment = deployment,
                onClick = { onDeploymentClick(deployment) }
            )
        }
    }
}

@Composable
private fun DeploymentCard(
    deployment: DeploymentUiModel,
    onClick: () -> Unit
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainer
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = 1.dp)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Icon(
                    imageVector = Icons.Outlined.Security,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.primary
                )
                Spacer(modifier = Modifier.width(12.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = deployment.name,
                        style = MaterialTheme.typography.titleMedium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Spacer(modifier = Modifier.height(4.dp))
                    Text(
                        text = "${deployment.provider.displayName()} • ${deployment.cluster}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
                Spacer(modifier = Modifier.width(12.dp))
                StatusBadge(status = deployment.status)
            }
            Spacer(modifier = Modifier.height(12.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = deployment.environment,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.weight(1f)
                )
                Text(
                    text = deployment.region,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.wrapContentWidth()
                )
            }
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun DeploymentsScreenPreview() {
    AndroidTheme {
        DeploymentsScreen(
            uiState = DeploymentsUiState.Success(
                deployments = listOf(
                    DeploymentUiModel(
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
                    ),
                    DeploymentUiModel(
                        id = "dep-2",
                        name = "Ferriskey - Edge",
                        environment = "Staging",
                        status = DeploymentStatus.FAILED,
                        provider = IamProvider.FERRISKEY,
                        cluster = "iam-stg-02",
                        namespace = "ferriskey",
                        version = "2.3.1",
                        endpoint = "https://iam-stg.aether.io",
                        region = "eu-west-1",
                        updatedAt = "2024-08-11 18:03"
                    )
                ),
                isRefreshing = false
            ),
            onRefresh = {},
            onDeploymentClick = {}
        )
    }
}
