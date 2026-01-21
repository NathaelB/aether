package fr.aether.android.presentation.health

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun HealthScoreIndicator(
    score: Int,
    level: HealthLevel,
    compact: Boolean,
    modifier: Modifier = Modifier
) {
    val containerColor by animateColorAsState(
        targetValue = levelColor(level),
        animationSpec = tween(durationMillis = 450),
        label = "health_container"
    )
    val onContainer = levelOnColor(level)
    if (compact) {
        Surface(
            modifier = modifier,
            color = containerColor,
            shape = RoundedCornerShape(999.dp)
        ) {
            Row(
                modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Text(
                    text = "Health",
                    style = MaterialTheme.typography.labelMedium,
                    color = onContainer
                )
                AnimatedContent(
                    targetState = score,
                    transitionSpec = { fadeIn(tween(180)) togetherWith fadeOut(tween(140)) },
                    label = "health_score_compact"
                ) { target ->
                    Text(
                        text = target.toString(),
                        style = MaterialTheme.typography.labelLarge,
                        color = onContainer
                    )
                }
            }
        }
        return
    }

    Surface(
        modifier = modifier,
        color = containerColor,
        shape = CircleShape
    ) {
        Box(
            modifier = Modifier
                .size(132.dp)
                .padding(16.dp),
            contentAlignment = Alignment.Center
        ) {
            CircularProgressIndicator(
                progress = score / 100f,
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(1f),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                trackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                strokeWidth = 8.dp
            )
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center
            ) {
                AnimatedContent(
                    targetState = score,
                    label = "health_score",
                    transitionSpec = { fadeIn(tween(200)) togetherWith fadeOut(tween(160)) }
                ) { target ->
                    Text(
                        text = target.toString(),
                        style = MaterialTheme.typography.displaySmall,
                        color = onContainer
                    )
                }
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = levelLabel(level),
                    style = MaterialTheme.typography.labelMedium,
                    color = onContainer
                )
            }
        }
    }
}

@Composable
private fun levelColor(level: HealthLevel) = when (level) {
    HealthLevel.HEALTHY -> MaterialTheme.colorScheme.primaryContainer
    HealthLevel.DEGRADED -> MaterialTheme.colorScheme.tertiaryContainer
    HealthLevel.UNSTABLE -> MaterialTheme.colorScheme.surfaceVariant
    HealthLevel.CRITICAL -> MaterialTheme.colorScheme.errorContainer
}

@Composable
private fun levelOnColor(level: HealthLevel) = when (level) {
    HealthLevel.HEALTHY -> MaterialTheme.colorScheme.onPrimaryContainer
    HealthLevel.DEGRADED -> MaterialTheme.colorScheme.onTertiaryContainer
    HealthLevel.UNSTABLE -> MaterialTheme.colorScheme.onSurfaceVariant
    HealthLevel.CRITICAL -> MaterialTheme.colorScheme.onErrorContainer
}

private fun levelLabel(level: HealthLevel): String = when (level) {
    HealthLevel.HEALTHY -> "Healthy"
    HealthLevel.DEGRADED -> "Degraded"
    HealthLevel.UNSTABLE -> "Unstable"
    HealthLevel.CRITICAL -> "Critical"
}
