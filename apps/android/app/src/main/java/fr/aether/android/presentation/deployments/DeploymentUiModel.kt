package fr.aether.android.presentation.deployments

import fr.aether.android.domain.model.DeploymentStatus
import fr.aether.android.domain.model.IamProvider

data class DeploymentUiModel(
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
