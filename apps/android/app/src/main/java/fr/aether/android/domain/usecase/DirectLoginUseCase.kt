package fr.aether.android.domain.usecase

import fr.aether.android.domain.repository.AuthRepository
import fr.aether.android.domain.model.AuthToken

class DirectLoginUseCase(
    private val repository: AuthRepository
) {
    suspend operator fun invoke(
        username: String,
        password: String
    ): Result<AuthToken> = repository.loginWithPassword(username, password)
}
