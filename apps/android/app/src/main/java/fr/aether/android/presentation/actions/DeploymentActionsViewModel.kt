package fr.aether.android.presentation.actions

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlin.random.Random
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class DeploymentActionsViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(DeploymentActionState())
    val uiState: StateFlow<DeploymentActionState> = _uiState.asStateFlow()

    private val random = Random.Default

    fun initialize(deploymentId: String) {
        val current = _uiState.value
        if (current.deploymentId == deploymentId) return
        val base = (deploymentId.hashCode().absoluteValue % 4) + 3
        val min = 1
        val max = 8
        val replicas = base.coerceIn(min, max)
        _uiState.value = current.copy(
            deploymentId = deploymentId,
            replicas = replicas,
            pendingReplicas = replicas,
            minReplicas = min,
            maxReplicas = max,
            maintenanceEnabled = deploymentId.hashCode() % 2 == 0
        )
    }

    fun requestRestart() {
        val state = _uiState.value
        if (state.restartPhase == ActionPhase.InProgress) return
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(
                restartPhase = ActionPhase.InProgress,
                feedback = ActionFeedback("Restarting deployment…", ActionPhase.InProgress)
            )
            delay(randomDelay())
            val success = random.nextFloat() > 0.2f
            _uiState.value = _uiState.value.copy(
                restartPhase = if (success) ActionPhase.Success else ActionPhase.Failure,
                feedback = ActionFeedback(
                    if (success) "Deployment restarted." else "Restart failed. Please retry.",
                    if (success) ActionPhase.Success else ActionPhase.Failure
                )
            )
            delay(1300)
            _uiState.value = _uiState.value.copy(
                restartPhase = ActionPhase.Idle,
                feedback = null
            )
        }
    }

    fun openScaleSheet() {
        _uiState.value = _uiState.value.copy(
            isScaleSheetVisible = true,
            pendingReplicas = _uiState.value.replicas
        )
    }

    fun closeScaleSheet() {
        _uiState.value = _uiState.value.copy(
            isScaleSheetVisible = false,
            pendingReplicas = _uiState.value.replicas
        )
    }

    fun updatePendingReplicas(value: Int) {
        val state = _uiState.value
        _uiState.value = state.copy(
            pendingReplicas = value.coerceIn(state.minReplicas, state.maxReplicas)
        )
    }

    fun confirmScale() {
        val state = _uiState.value
        if (state.scalePhase == ActionPhase.InProgress) return
        val previous = state.replicas
        val target = state.pendingReplicas
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(
                replicas = target,
                isScaleSheetVisible = false,
                scalePhase = ActionPhase.InProgress,
                feedback = ActionFeedback("Scaling to $target replicas…", ActionPhase.InProgress)
            )
            delay(randomDelay())
            val success = random.nextFloat() > 0.15f
            _uiState.value = if (success) {
                _uiState.value.copy(
                    scalePhase = ActionPhase.Success,
                    feedback = ActionFeedback("Scale complete at $target replicas.", ActionPhase.Success)
                )
            } else {
                _uiState.value.copy(
                    replicas = previous,
                    pendingReplicas = previous,
                    scalePhase = ActionPhase.Failure,
                    feedback = ActionFeedback("Scaling failed. Reverted to $previous.", ActionPhase.Failure)
                )
            }
            delay(1400)
            _uiState.value = _uiState.value.copy(
                scalePhase = ActionPhase.Idle,
                feedback = null
            )
        }
    }

    fun toggleMaintenance() {
        val state = _uiState.value
        if (state.maintenancePhase == ActionPhase.InProgress) return
        val target = !state.maintenanceEnabled
        val previous = state.maintenanceEnabled
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(
                maintenanceEnabled = target,
                maintenancePhase = ActionPhase.InProgress,
                feedback = ActionFeedback(
                    if (target) "Enabling maintenance mode…" else "Disabling maintenance mode…",
                    ActionPhase.InProgress
                )
            )
            delay(randomDelay())
            val success = random.nextFloat() > 0.1f
            _uiState.value = if (success) {
                _uiState.value.copy(
                    maintenancePhase = ActionPhase.Success,
                    feedback = ActionFeedback(
                        if (target) "Maintenance mode enabled." else "Maintenance mode disabled.",
                        ActionPhase.Success
                    )
                )
            } else {
                _uiState.value.copy(
                    maintenanceEnabled = previous,
                    maintenancePhase = ActionPhase.Failure,
                    feedback = ActionFeedback(
                        "Maintenance update failed. Restored previous state.",
                        ActionPhase.Failure
                    )
                )
            }
            delay(1400)
            _uiState.value = _uiState.value.copy(
                maintenancePhase = ActionPhase.Idle,
                feedback = null
            )
        }
    }

    private fun randomDelay(): Long = random.nextLong(1000L, 1900L)
}

private val Int.absoluteValue: Int
    get() = if (this < 0) -this else this
