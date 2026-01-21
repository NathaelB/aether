package fr.aether.android.domain.repository

import fr.aether.android.domain.model.Deployment
import fr.aether.android.domain.model.CreateDeploymentRequest

interface DeploymentRepository {
    suspend fun getDeployments(): List<Deployment>
    suspend fun createDeployment(request: CreateDeploymentRequest): Deployment
    suspend fun deleteDeployment(id: String)
}
