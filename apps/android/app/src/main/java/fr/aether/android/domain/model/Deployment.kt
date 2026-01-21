package fr.aether.android.domain.model

import kotlinx.serialization.Serializable

data class Deployment(
    val id: String,
    val name: String,
    val environment: String,
    val status: DeploymentStatus,
    val provider: IamProvider,
    val cluster: String,
    val namespace: String,
    val version: String,
    val endpoint: String,
    val region: String,
    val updatedAt: String
)

@Serializable
enum class DeploymentStatus {
    RUNNING,
    DEPLOYING,
    STOPPED,
    FAILED
}

@Serializable
enum class IamProvider {
    KEYCLOAK,
    FERRISKEY
}
