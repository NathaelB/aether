package fr.aether.android.presentation.observability

data class DeploymentMetrics(
    val cpuUsage: Float,
    val memoryUsage: Float,
    val replicas: Int,
    val maxReplicas: Int,
    val cpuHistory: List<Float>,
    val memoryHistory: List<Float>,
    val replicasHistory: List<Int>
)

sealed interface ObservabilityUiState {
    data object Loading : ObservabilityUiState
    data class Error(val message: String) : ObservabilityUiState
    data class Data(val metrics: DeploymentMetrics) : ObservabilityUiState
}
