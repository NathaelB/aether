package fr.aether.android.presentation.health

data class DeploymentHealth(
    val score: Int,
    val level: HealthLevel,
    val summary: String
)

enum class HealthLevel {
    HEALTHY,
    DEGRADED,
    UNSTABLE,
    CRITICAL
}

sealed interface HealthUiState {
    data object Loading : HealthUiState
    data class Error(val message: String) : HealthUiState
    data class Data(val health: DeploymentHealth) : HealthUiState
}
