package fr.aether.android.presentation.actions

data class DeploymentActionState(
    val deploymentId: String? = null,
    val replicas: Int = 3,
    val minReplicas: Int = 1,
    val maxReplicas: Int = 8,
    val pendingReplicas: Int = 3,
    val maintenanceEnabled: Boolean = false,
    val restartPhase: ActionPhase = ActionPhase.Idle,
    val scalePhase: ActionPhase = ActionPhase.Idle,
    val maintenancePhase: ActionPhase = ActionPhase.Idle,
    val isScaleSheetVisible: Boolean = false,
    val feedback: ActionFeedback? = null
)

enum class ActionPhase {
    Idle,
    InProgress,
    Success,
    Failure
}

data class ActionFeedback(
    val message: String,
    val phase: ActionPhase
)
