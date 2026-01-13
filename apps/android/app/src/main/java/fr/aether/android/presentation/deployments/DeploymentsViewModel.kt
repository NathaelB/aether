package fr.aether.android.presentation.deployments

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import fr.aether.android.domain.model.Deployment
import fr.aether.android.domain.usecase.GetDeploymentsUseCase
import javax.inject.Inject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

@HiltViewModel
class DeploymentsViewModel @Inject constructor(
    private val getDeploymentsUseCase: GetDeploymentsUseCase
) : ViewModel() {
    private val _uiState = MutableStateFlow<DeploymentsUiState>(DeploymentsUiState.Loading)
    val uiState: StateFlow<DeploymentsUiState> = _uiState.asStateFlow()
    private var cachedDeployments: List<DeploymentUiModel> = emptyList()

    init {
        loadDeployments(isRefresh = false)
    }

    fun refresh() {
        loadDeployments(isRefresh = true)
    }

    private fun loadDeployments(isRefresh: Boolean) {
        viewModelScope.launch {
            val currentList =
                (uiState.value as? DeploymentsUiState.Success)?.deployments.orEmpty()
            if (isRefresh && currentList.isNotEmpty()) {
                _uiState.value = DeploymentsUiState.Success(
                    deployments = currentList,
                    isRefreshing = true
                )
            } else {
                _uiState.value = DeploymentsUiState.Loading
            }

            try {
                val deployments = getDeploymentsUseCase().map { it.toUiModel() }
                delay(800)
                cachedDeployments = deployments
                _uiState.value = DeploymentsUiState.Success(
                    deployments = deployments,
                    isRefreshing = false
                )
            } catch (exception: Exception) {
                _uiState.value = DeploymentsUiState.Error(
                    message = "Unable to load deployments."
                )
            }
        }
    }

    fun deploymentById(id: String): DeploymentUiModel? {
        return cachedDeployments.firstOrNull { it.id == id }
            ?: (uiState.value as? DeploymentsUiState.Success)
                ?.deployments
                ?.firstOrNull { it.id == id }
    }
}

private fun Deployment.toUiModel(): DeploymentUiModel {
    return DeploymentUiModel(
        id = id,
        name = name,
        environment = environment,
        status = status,
        provider = provider,
        cluster = cluster,
        namespace = namespace,
        version = version,
        endpoint = endpoint,
        region = region,
        updatedAt = updatedAt
    )
}
