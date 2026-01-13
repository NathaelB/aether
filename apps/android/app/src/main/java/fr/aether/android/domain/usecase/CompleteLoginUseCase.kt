package fr.aether.android.domain.usecase

import fr.aether.android.domain.model.AuthToken
import fr.aether.android.domain.repository.AuthRepository

class CompleteLoginUseCase(
    private val repository: AuthRepository
) {
    suspend operator fun invoke(
        authorizationCode: String,
        state: String
    ): Result<AuthToken> = repository.exchangeToken(authorizationCode, state)
}
