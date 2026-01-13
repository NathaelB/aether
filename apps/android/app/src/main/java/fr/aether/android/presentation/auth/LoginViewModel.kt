package fr.aether.android.presentation.auth

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import fr.aether.android.domain.usecase.CompleteLoginUseCase
import fr.aether.android.domain.usecase.LoginUseCase
import javax.inject.Inject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

@HiltViewModel
class LoginViewModel @Inject constructor(
    private val loginUseCase: LoginUseCase,
    private val completeLoginUseCase: CompleteLoginUseCase,
    private val authResultBroadcaster: AuthResultBroadcaster
) : ViewModel() {
    private val _uiState = MutableStateFlow<LoginUiState>(LoginUiState.Idle)
    val uiState: StateFlow<LoginUiState> = _uiState.asStateFlow()

    init {
        observeAuthResults()
    }

    fun onLoginClicked() {
        if (_uiState.value is LoginUiState.Loading ||
            _uiState.value is LoginUiState.AwaitingCallback
        ) {
            return
        }
        _uiState.value = LoginUiState.Loading
        viewModelScope.launch {
            val result = loginUseCase()
            _uiState.value = result.fold(
                onSuccess = { request -> LoginUiState.Launching(request) },
                onFailure = { LoginUiState.Error("Login failed. Please try again.") }
            )
        }
    }

    fun onAuthLaunched() {
        _uiState.value = LoginUiState.AwaitingCallback
    }

    private fun observeAuthResults() {
        viewModelScope.launch {
            authResultBroadcaster.results.collect { result ->
                when (result) {
                    is AuthResult.Error -> {
                        _uiState.value = LoginUiState.Error(result.message)
                    }
                    is AuthResult.Success -> {
                        completeLogin(result.authorizationCode, result.state)
                    }
                }
            }
        }
    }

    private fun completeLogin(authorizationCode: String, state: String) {
        _uiState.value = LoginUiState.Loading
        viewModelScope.launch {
            val result = completeLoginUseCase(authorizationCode, state)
            _uiState.value = result.fold(
                onSuccess = { token -> LoginUiState.Success(token) },
                onFailure = { throwable ->
                    LoginUiState.Error(throwable.message ?: "Token exchange failed.")
                }
            )
        }
    }
}
