package fr.aether.android.presentation.activity

import java.time.Instant

data class DeploymentEvent(
    val id: String,
    val type: EventType,
    val message: String,
    val timestamp: Instant,
    val severity: EventSeverity
)

enum class EventType {
    ACTION,
    ALERT,
    SCALING,
    STATUS_CHANGE,
    SYSTEM
}

enum class EventSeverity {
    INFO,
    WARNING,
    CRITICAL
}

sealed interface ActivityUiState {
    data object Loading : ActivityUiState
    data class Error(val message: String) : ActivityUiState
    data class Empty(val message: String = "No recent activity") : ActivityUiState
    data class Data(val events: List<DeploymentEvent>) : ActivityUiState
}
