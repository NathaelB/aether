package fr.aether.android.presentation.deployments

sealed interface DeploymentsUiState {
    data object Loading : DeploymentsUiState
    data class Success(
        val deployments: List<DeploymentUiModel>,
        val isRefreshing: Boolean
    ) : DeploymentsUiState

    data class Error(val message: String) : DeploymentsUiState
}
