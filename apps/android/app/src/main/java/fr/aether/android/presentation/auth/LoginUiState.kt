package fr.aether.android.presentation.auth

import fr.aether.android.domain.model.AuthRequest
import fr.aether.android.domain.model.AuthToken

sealed interface LoginUiState {
    data object Idle : LoginUiState
    data object Loading : LoginUiState
    data class Launching(val request: AuthRequest) : LoginUiState
    data object AwaitingCallback : LoginUiState
    data class Success(val token: AuthToken) : LoginUiState
    data class Error(val message: String) : LoginUiState
}
