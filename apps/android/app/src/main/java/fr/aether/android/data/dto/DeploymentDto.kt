package fr.aether.android.data.dto

import fr.aether.android.domain.model.Deployment
import fr.aether.android.domain.model.DeploymentStatus
import fr.aether.android.domain.model.IamProvider

data class DeploymentDto(
    val id: String,
    val name: String,
    val environment: String,
    val status: String,
    val provider: String? = null,
    val cluster: String? = null,
    val namespace: String? = null,
    val version: String? = null,
    val endpoint: String? = null,
    val region: String? = null,
    val updatedAt: String? = null
) {
    fun toDomain(): Deployment {
        val mappedStatus = when (status) {
            "RUNNING" -> DeploymentStatus.RUNNING
            "DEPLOYING" -> DeploymentStatus.DEPLOYING
            "STOPPED" -> DeploymentStatus.STOPPED
            else -> DeploymentStatus.FAILED
        }
        val mappedProvider = when (provider) {
            "FERRISKEY" -> IamProvider.FERRISKEY
            else -> IamProvider.KEYCLOAK
        }
        return Deployment(
            id = id,
            name = name,
            environment = environment,
            status = mappedStatus,
            provider = mappedProvider,
            cluster = cluster ?: "unknown-cluster",
            namespace = namespace ?: "default",
            version = version ?: "unknown",
            endpoint = endpoint ?: "-",
            region = region ?: "-",
            updatedAt = updatedAt ?: "-"
        )
    }
}
