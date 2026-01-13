package fr.aether.android.presentation.deployments

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import fr.aether.android.domain.model.DeploymentStatus
import fr.aether.android.domain.model.IamProvider
import androidx.compose.foundation.layout.padding

@Composable
fun StatusBadge(status: DeploymentStatus) {
    val (containerColor, contentColor, label) = when (status) {
        DeploymentStatus.RUNNING -> Triple(
            MaterialTheme.colorScheme.tertiaryContainer,
            MaterialTheme.colorScheme.onTertiaryContainer,
            "Running"
        )
        DeploymentStatus.STOPPED -> Triple(
            MaterialTheme.colorScheme.secondaryContainer,
            MaterialTheme.colorScheme.onSecondaryContainer,
            "Stopped"
        )
        DeploymentStatus.FAILED -> Triple(
            MaterialTheme.colorScheme.errorContainer,
            MaterialTheme.colorScheme.onErrorContainer,
            "Failed"
        )
    }

    Surface(
        color = containerColor,
        contentColor = contentColor,
        shape = MaterialTheme.shapes.small
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium,
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp)
        )
    }
}

fun IamProvider.displayName(): String {
    return when (this) {
        IamProvider.KEYCLOAK -> "Keycloak"
        IamProvider.FERRISKEY -> "Ferriskey"
    }
}
