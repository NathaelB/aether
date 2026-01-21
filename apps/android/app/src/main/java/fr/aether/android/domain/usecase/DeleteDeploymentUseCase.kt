package fr.aether.android.domain.usecase

import fr.aether.android.domain.repository.DeploymentRepository

class DeleteDeploymentUseCase(
    private val repository: DeploymentRepository
) {
    suspend operator fun invoke(id: String) {
        repository.deleteDeployment(id)
    }
}
