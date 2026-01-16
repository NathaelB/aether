package fr.aether.android.domain.repository

import fr.aether.android.domain.model.AuthToken
import fr.aether.android.domain.model.AuthRequest

interface AuthRepository {
    suspend fun createAuthorizationRequest(): Result<AuthRequest>
    suspend fun exchangeToken(authorizationCode: String, state: String): Result<AuthToken>
    suspend fun loginWithPassword(username: String, password: String): Result<AuthToken>
    suspend fun logout(): Result<Unit>
}
