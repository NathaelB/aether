package fr.aether.android.domain.repository

import fr.aether.android.domain.model.Deployment

interface DeploymentRepository {
    suspend fun getDeployments(): List<Deployment>
}
