package fr.aether.android.data.deployment

import fr.aether.android.domain.model.Deployment
import fr.aether.android.domain.model.DeploymentStatus
import fr.aether.android.domain.model.IamProvider
import fr.aether.android.domain.repository.DeploymentRepository
import kotlinx.coroutines.delay

class MockDeploymentRepository : DeploymentRepository {
    override suspend fun getDeployments(): List<Deployment> {
        // Mocked deployment data for local development.
        delay(500)
        return listOf(
            Deployment(
                id = "dep-001",
                name = "Keycloak - Core",
                environment = "Production",
                status = DeploymentStatus.RUNNING,
                provider = IamProvider.KEYCLOAK,
                cluster = "iam-prod-01",
                namespace = "keycloak",
                version = "24.0.2",
                endpoint = "https://iam.aether.io",
                region = "eu-west-1",
                updatedAt = "2024-08-12 10:24"
            ),
            Deployment(
                id = "dep-002",
                name = "Ferriskey - Edge",
                environment = "Staging",
                status = DeploymentStatus.STOPPED,
                provider = IamProvider.FERRISKEY,
                cluster = "iam-stg-02",
                namespace = "ferriskey",
                version = "2.3.1",
                endpoint = "https://iam-stg.aether.io",
                region = "eu-west-1",
                updatedAt = "2024-08-11 18:03"
            ),
            Deployment(
                id = "dep-003",
                name = "Keycloak - IAM Portal",
                environment = "Production",
                status = DeploymentStatus.FAILED,
                provider = IamProvider.KEYCLOAK,
                cluster = "iam-prod-02",
                namespace = "keycloak",
                version = "24.0.1",
                endpoint = "https://iam-portal.aether.io",
                region = "us-east-1",
                updatedAt = "2024-08-12 08:41"
            ),
            Deployment(
                id = "dep-004",
                name = "Ferriskey - Dev",
                environment = "Development",
                status = DeploymentStatus.DEPLOYING,
                provider = IamProvider.FERRISKEY,
                cluster = "iam-dev-01",
                namespace = "ferriskey",
                version = "2.3.1",
                endpoint = "https://iam-dev.aether.io",
                region = "eu-central-1",
                updatedAt = "2024-08-10 15:15"
            )
        )
    }
}
