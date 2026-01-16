package fr.aether.android.domain.usecase

import fr.aether.android.domain.model.AuthRequest
import fr.aether.android.domain.repository.AuthRepository

class LoginUseCase(
    private val repository: AuthRepository
) {
    suspend operator fun invoke(): Result<AuthRequest> = repository.createAuthorizationRequest()
}
