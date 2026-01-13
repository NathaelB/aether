package fr.aether.android.domain.usecase

import fr.aether.android.domain.model.Deployment
import fr.aether.android.domain.repository.DeploymentRepository

class GetDeploymentsUseCase(
    private val repository: DeploymentRepository
) {
    suspend operator fun invoke(): List<Deployment> = repository.getDeployments()
}
