package fr.aether.android.presentation.observability

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview
import fr.aether.android.ui.theme.AndroidTheme

private val previewMetrics = DeploymentMetrics(
    cpuUsage = 62f,
    memoryUsage = 74f,
    replicas = 4,
    maxReplicas = 6,
    cpuHistory = List(24) { 40f + it },
    memoryHistory = List(24) { 55f + it / 2f },
    replicasHistory = List(24) { 3 + (it % 2) }
)

@Preview(showBackground = true)
@Composable
private fun ObservabilitySectionPreview() {
    AndroidTheme {
        ObservabilitySection(
            uiState = ObservabilityUiState.Data(previewMetrics),
            onRetry = {}
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun CpuCardPreview() {
    AndroidTheme {
        CpuUsageCard(cpuUsage = 86f, history = List(24) { 65f + it / 3f })
    }
}

@Preview(showBackground = true)
@Composable
private fun MemoryCardPreview() {
    AndroidTheme {
        MemoryUsageCard(memoryUsage = 45f, history = List(24) { 48f - it / 6f })
    }
}

@Preview(showBackground = true)
@Composable
private fun ReplicasCardPreview() {
    AndroidTheme {
        ReplicasCard(replicas = 3, maxReplicas = 6, history = List(24) { 3 })
    }
}

@Preview(showBackground = true)
@Composable
private fun ObservabilityLoadingPreview() {
    AndroidTheme {
        ObservabilitySection(
            uiState = ObservabilityUiState.Loading,
            onRetry = {}
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun ObservabilityErrorPreview() {
    AndroidTheme {
        ObservabilitySection(
            uiState = ObservabilityUiState.Error("Metrics service unavailable."),
            onRetry = {}
        )
    }
}
