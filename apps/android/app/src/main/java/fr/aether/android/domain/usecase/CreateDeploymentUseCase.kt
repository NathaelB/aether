package fr.aether.android.domain.usecase

import fr.aether.android.domain.model.CreateDeploymentRequest
import fr.aether.android.domain.model.Deployment
import fr.aether.android.domain.repository.DeploymentRepository

class CreateDeploymentUseCase(
    private val repository: DeploymentRepository
) {
    suspend operator fun invoke(request: CreateDeploymentRequest): Deployment {
        return repository.createDeployment(request)
    }
}
