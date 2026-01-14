package fr.aether.android.presentation.deployments

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import fr.aether.android.domain.model.DeploymentStatus

@Composable
fun DeploymentStatusBadge(
    status: DeploymentStatus,
    modifier: Modifier = Modifier
) {
    val colors = statusColors(status)
    val containerColor by animateColorAsState(
        targetValue = colors.container,
        animationSpec = tween(220),
        label = "status_container"
    )
    val contentColor by animateColorAsState(
        targetValue = colors.content,
        animationSpec = tween(220),
        label = "status_content"
    )
    val isDeploying = status == DeploymentStatus.DEPLOYING
    val pulseAlpha = if (isDeploying) {
        val transition = rememberInfiniteTransition(label = "deploying_pulse")
        val alpha by transition.animateFloat(
            initialValue = 0.5f,
            targetValue = 1f,
            animationSpec = infiniteRepeatable(
                animation = tween(900),
                repeatMode = RepeatMode.Reverse
            ),
            label = "deploying_alpha"
        )
        alpha
    } else {
        1f
    }

    Surface(
        modifier = modifier,
        shape = MaterialTheme.shapes.extraLarge,
        color = containerColor,
        contentColor = contentColor
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            if (isDeploying) {
                Box(
                    modifier = Modifier
                        .size(8.dp)
                        .clip(CircleShape)
                        .background(contentColor.copy(alpha = pulseAlpha))
                )
            }
            AnimatedContent(
                targetState = status,
                label = "status_label"
            ) { targetStatus ->
                Text(
                    text = targetStatus.label(),
                    style = MaterialTheme.typography.labelMedium
                )
            }
        }
    }
}

@Composable
fun EnvironmentBadge(
    environment: String,
    modifier: Modifier = Modifier
) {
    val colors = environmentColors(environment)
    val containerColor by animateColorAsState(
        targetValue = colors.container,
        animationSpec = tween(220),
        label = "environment_container"
    )
    val contentColor by animateColorAsState(
        targetValue = colors.content,
        animationSpec = tween(220),
        label = "environment_content"
    )
    Surface(
        modifier = modifier,
        shape = MaterialTheme.shapes.extraLarge,
        color = containerColor,
        contentColor = contentColor
    ) {
        Text(
            text = environmentLabel(environment),
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
            style = MaterialTheme.typography.labelMedium
        )
    }
}

private data class BadgeColors(
    val container: androidx.compose.ui.graphics.Color,
    val content: androidx.compose.ui.graphics.Color
)

@Composable
private fun statusColors(status: DeploymentStatus): BadgeColors {
    val scheme = MaterialTheme.colorScheme
    return when (status) {
        DeploymentStatus.RUNNING -> BadgeColors(
            container = scheme.tertiaryContainer,
            content = scheme.onTertiaryContainer
        )
        DeploymentStatus.DEPLOYING -> BadgeColors(
            container = scheme.primaryContainer,
            content = scheme.onPrimaryContainer
        )
        DeploymentStatus.STOPPED -> BadgeColors(
            container = scheme.surfaceVariant,
            content = scheme.onSurfaceVariant
        )
        DeploymentStatus.FAILED -> BadgeColors(
            container = scheme.errorContainer,
            content = scheme.onErrorContainer
        )
    }
}

@Composable
private fun environmentColors(environment: String): BadgeColors {
    val scheme = MaterialTheme.colorScheme
    return when (environment.lowercase()) {
        "production", "prod" -> BadgeColors(
            container = scheme.primaryContainer,
            content = scheme.onPrimaryContainer
        )
        "staging", "stage" -> BadgeColors(
            container = scheme.tertiaryContainer,
            content = scheme.onTertiaryContainer
        )
        "development", "dev" -> BadgeColors(
            container = scheme.secondaryContainer,
            content = scheme.onSecondaryContainer
        )
        else -> BadgeColors(
            container = scheme.surfaceContainer,
            content = scheme.onSurfaceVariant
        )
    }
}

private fun environmentLabel(environment: String): String {
    return when (environment.lowercase()) {
        "production", "prod" -> "Prod"
        "staging", "stage" -> "Staging"
        "development", "dev" -> "Dev"
        else -> environment
    }
}

private fun DeploymentStatus.label(): String {
    return when (this) {
        DeploymentStatus.RUNNING -> "Running"
        DeploymentStatus.DEPLOYING -> "Deploying"
        DeploymentStatus.STOPPED -> "Stopped"
        DeploymentStatus.FAILED -> "Failed"
    }
}
