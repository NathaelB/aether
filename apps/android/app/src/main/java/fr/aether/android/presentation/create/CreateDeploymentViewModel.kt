package fr.aether.android.presentation.create

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import android.util.Log
import dagger.hilt.android.lifecycle.HiltViewModel
import fr.aether.android.domain.model.CreateDeploymentRequest
import fr.aether.android.domain.model.CpuPreset
import fr.aether.android.domain.model.Environment
import fr.aether.android.domain.model.MemoryPreset
import fr.aether.android.domain.usecase.CreateDeploymentUseCase
import javax.inject.Inject
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

@HiltViewModel
class CreateDeploymentViewModel @Inject constructor(
    private val createDeploymentUseCase: CreateDeploymentUseCase
) : ViewModel() {
    private val tag = "CreateDeploymentVM"
    private val _uiState = MutableStateFlow(CreateDeploymentUiState())
    val uiState: StateFlow<CreateDeploymentUiState> = _uiState.asStateFlow()

    fun updateName(value: String) {
        _uiState.value = _uiState.value.copy(name = value, nameError = null)
    }

    fun updateEnvironment(value: Environment) {
        _uiState.value = _uiState.value.copy(environment = value)
    }

    fun updateReplicas(value: Int) {
        _uiState.value = _uiState.value.copy(replicas = value.coerceIn(1, 8))
    }

    fun updateCpuPreset(value: CpuPreset) {
        _uiState.value = _uiState.value.copy(cpuPreset = value)
    }

    fun updateMemoryPreset(value: MemoryPreset) {
        _uiState.value = _uiState.value.copy(memoryPreset = value)
    }

    fun toggleAutoScaling(enabled: Boolean) {
        _uiState.value = _uiState.value.copy(autoScalingEnabled = enabled)
    }

    fun goToReview() {
        val error = validateName(_uiState.value.name)
        if (error != null) {
            _uiState.value = _uiState.value.copy(nameError = error)
            return
        }
        _uiState.value = _uiState.value.copy(step = CreateDeploymentStep.REVIEW, nameError = null)
    }

    fun backToEdit() {
        _uiState.value = _uiState.value.copy(step = CreateDeploymentStep.EDIT, errorMessage = null)
    }

    fun startCreate() {
        val current = _uiState.value
        if (current.step == CreateDeploymentStep.CREATING) return
        _uiState.value = current.copy(step = CreateDeploymentStep.CREATING, errorMessage = null)
        viewModelScope.launch {
            val messages = listOf(
                "Creating deployment…",
                "Allocating capacity…",
                "Starting pods…",
                "Finalizing configuration…"
            )
            messages.forEach { message ->
                _uiState.value = _uiState.value.copy(progressMessage = message)
                delay(700)
            }
            try {
                delay(700)
                val deployment = createDeploymentUseCase(
                    CreateDeploymentRequest(
                        name = current.name.trim(),
                        environment = current.environment,
                        replicas = current.replicas,
                        cpuLimit = current.cpuPreset,
                        memoryLimit = current.memoryPreset,
                        autoScalingEnabled = current.autoScalingEnabled
                    )
                )
                _uiState.value = _uiState.value.copy(
                    step = CreateDeploymentStep.SUCCESS,
                    createdDeployment = deployment
                )
            } catch (exception: Exception) {
                Log.e(tag, "Create deployment failed", exception)
                _uiState.value = _uiState.value.copy(
                    step = CreateDeploymentStep.ERROR,
                    errorMessage = "Unable to create deployment. Please retry."
                )
            }
        }
    }

    fun retryCreate() {
        _uiState.value = _uiState.value.copy(step = CreateDeploymentStep.REVIEW)
    }

    fun markHandled() {
        _uiState.value = _uiState.value.copy(createdDeployment = null)
    }

    private fun validateName(value: String): String? {
        if (value.isBlank()) return "Name is required."
        if (value.trim().length < 3) return "Name must be at least 3 characters."
        val regex = Regex("^[a-zA-Z0-9][a-zA-Z0-9\\- ]+$")
        if (!regex.matches(value.trim())) return "Use letters, numbers, spaces, or hyphens."
        return null
    }
}
