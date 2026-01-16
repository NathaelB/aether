package fr.aether.android.domain.usecase

import fr.aether.android.domain.repository.AuthRepository

class LogoutUseCase(
    private val repository: AuthRepository
) {
    suspend operator fun invoke(): Result<Unit> = repository.logout()
}
