package fr.aether.android.domain.model

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

enum class DeploymentStatus {
    RUNNING,
    STOPPED,
    FAILED
}

enum class IamProvider {
    KEYCLOAK,
    FERRISKEY
}
