package fr.aether.android.presentation.auth

sealed interface AuthResult {
    data class Success(val authorizationCode: String, val state: String) : AuthResult
    data class Error(val message: String) : AuthResult
}
