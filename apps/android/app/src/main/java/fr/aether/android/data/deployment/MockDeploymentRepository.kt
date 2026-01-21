package fr.aether.android.data.deployment

import android.content.Context
import android.util.Log
import dagger.hilt.android.qualifiers.ApplicationContext
import fr.aether.android.domain.model.CreateDeploymentRequest
import fr.aether.android.domain.model.Deployment
import fr.aether.android.domain.model.DeploymentStatus
import fr.aether.android.domain.model.IamProvider
import fr.aether.android.domain.repository.DeploymentRepository
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.delay
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json

@Singleton
class MockDeploymentRepository @Inject constructor(
    @ApplicationContext context: Context
) : DeploymentRepository {
    private val tag = "MockDeploymentRepo"
    private val formatter = DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm")
    private val preferences = context.getSharedPreferences(PrefsName, Context.MODE_PRIVATE)
    private val json = Json { ignoreUnknownKeys = true }
    private val deployments = mutableListOf<Deployment>()
    private var nextId = 5

    override suspend fun getDeployments(): List<Deployment> {
        delay(500)
        if (deployments.isEmpty()) {
            val cached = loadDeployments()
            if (cached.isNotEmpty()) {
                deployments.addAll(cached)
                nextId = cached.size + 1
            } else {
                deployments.addAll(seedDeployments())
                persist()
            }
        }
        return deployments.toList()
    }

    override suspend fun createDeployment(request: CreateDeploymentRequest): Deployment {
        delay(1100)
        val environmentLabel = when (request.environment) {
            fr.aether.android.domain.model.Environment.DEV -> "Development"
            fr.aether.android.domain.model.Environment.STAGING -> "Staging"
            fr.aether.android.domain.model.Environment.PROD -> "Production"
        }
        val id = "dep-${nextId.toString().padStart(3, '0')}"
        nextId += 1
        val now = LocalDateTime.now().format(formatter)
        val deployment = Deployment(
            id = id,
            name = request.name,
            environment = environmentLabel,
            status = DeploymentStatus.DEPLOYING,
            provider = IamProvider.KEYCLOAK,
            cluster = "iam-${environmentLabel.lowercase()}-01",
            namespace = request.name.lowercase().replace(" ", "-"),
            version = "1.0.0",
            endpoint = "https://${request.name.lowercase().replace(" ", "-")}.aether.io",
            region = "us-east-1",
            updatedAt = now
        )
        deployments.add(0, deployment)
        persist()
        return deployment
    }

    override suspend fun deleteDeployment(id: String) {
        delay(400)
        deployments.removeAll { it.id == id }
        persist()
    }

    private fun persist() {
        runCatching {
            val stored = deployments.map { it.toStored() }
            val encoded = json.encodeToString(stored)
            preferences.edit()
                .putString(KeyDeployments, encoded)
                .apply()
            Log.d(tag, "Persisted deployments: ${stored.size}")
        }.onFailure { error ->
            Log.e(tag, "Failed to persist deployments", error)
        }
    }

    private fun loadDeployments(): List<Deployment> {
        val raw = preferences.getString(KeyDeployments, null) ?: return emptyList()
        return runCatching {
            json.decodeFromString<List<StoredDeployment>>(raw).map { it.toDomain() }
        }.onFailure { error ->
            Log.e(tag, "Failed to load deployments", error)
        }.getOrDefault(emptyList())
    }

    private fun seedDeployments(): List<Deployment> {
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

    @Serializable
    private data class StoredDeployment(
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

    private fun Deployment.toStored() = StoredDeployment(
        id = id,
        name = name,
        environment = environment,
        status = status,
        provider = provider,
        cluster = cluster,
        namespace = namespace,
        version = version,
        endpoint = endpoint,
        region = region,
        updatedAt = updatedAt
    )

    private fun StoredDeployment.toDomain() = Deployment(
        id = id,
        name = name,
        environment = environment,
        status = status,
        provider = provider,
        cluster = cluster,
        namespace = namespace,
        version = version,
        endpoint = endpoint,
        region = region,
        updatedAt = updatedAt
    )

    private companion object {
        private const val PrefsName = "deployment_store"
        private const val KeyDeployments = "deployments_json"
    }
}
