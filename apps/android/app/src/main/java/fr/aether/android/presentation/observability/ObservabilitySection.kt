package fr.aether.android.presentation.observability

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun ObservabilitySection(
    uiState: ObservabilityUiState,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .animateContentSize(),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Text(
            text = "Observability",
            style = MaterialTheme.typography.titleLarge
        )
        AnimatedContent(
            targetState = uiState,
            transitionSpec = {
                fadeIn(tween(200)) togetherWith fadeOut(tween(150))
            },
            label = "observability_state"
        ) { state ->
            when (state) {
                ObservabilityUiState.Loading -> ObservabilitySkeleton()
                is ObservabilityUiState.Error -> ObservabilityError(
                    message = state.message,
                    onRetry = onRetry
                )
                is ObservabilityUiState.Data -> ObservabilityContent(metrics = state.metrics)
            }
        }
    }
}

@Composable
private fun ObservabilityContent(metrics: DeploymentMetrics) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(IntrinsicSize.Min),
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            CpuUsageCard(
                cpuUsage = metrics.cpuUsage,
                history = metrics.cpuHistory,
                modifier = Modifier
                    .weight(1f)
                    .fillMaxHeight()
            )
            MemoryUsageCard(
                memoryUsage = metrics.memoryUsage,
                history = metrics.memoryHistory,
                modifier = Modifier
                    .weight(1f)
                    .fillMaxHeight()
            )
        }
        ReplicasCard(
            replicas = metrics.replicas,
            maxReplicas = metrics.maxReplicas,
            history = metrics.replicasHistory,
            modifier = Modifier.fillMaxWidth()
        )
    }
}

@Composable
private fun ObservabilityError(
    message: String,
    onRetry: () -> Unit
) {
    Card(
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow
        ),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            horizontalAlignment = Alignment.Start
        ) {
            Text(
                text = "Metrics unavailable",
                style = MaterialTheme.typography.titleMedium
            )
            Text(
                text = message,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(modifier = Modifier.height(4.dp))
            Button(onClick = onRetry) {
                Text(text = "Retry")
            }
        }
    }
}

@Composable
private fun ObservabilitySkeleton() {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            MetricSkeletonCard(modifier = Modifier.weight(1f))
            MetricSkeletonCard(modifier = Modifier.weight(1f))
        }
        MetricSkeletonCard(modifier = Modifier.fillMaxWidth())
    }
}

@Composable
private fun MetricSkeletonCard(modifier: Modifier = Modifier) {
    Card(
        modifier = modifier,
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow
        ),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            PlaceholderLine(widthFraction = 0.4f)
            PlaceholderLine(widthFraction = 0.7f)
            PlaceholderLine(widthFraction = 0.3f)
        }
    }
}

@Composable
private fun PlaceholderLine(widthFraction: Float) {
    val color = MaterialTheme.colorScheme.surfaceContainer
    Spacer(
        modifier = Modifier
            .fillMaxWidth(widthFraction)
            .height(12.dp)
            .padding(vertical = 2.dp)
            .background(color, MaterialTheme.shapes.small)
    )
}
