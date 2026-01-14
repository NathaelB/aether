package fr.aether.android.presentation.observability

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun ReplicasCard(
    replicas: Int,
    maxReplicas: Int,
    history: List<Int>,
    modifier: Modifier = Modifier
) {
    val animatedRatio by animateFloatAsState(
        targetValue = replicas.toFloat() / maxReplicas.coerceAtLeast(1),
        animationSpec = tween(300),
        label = "replicas_ratio"
    )
    val containerColor = MaterialTheme.colorScheme.surfaceContainer

    Card(
        modifier = modifier.animateContentSize(),
        colors = CardDefaults.cardColors(containerColor = containerColor),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = "Instances",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurface
                )
                Row(
                    modifier = Modifier.weight(1f),
                    horizontalArrangement = Arrangement.End
                ) {
                    AnimatedContent(
                        targetState = "$replicas/$maxReplicas",
                        label = "replicas_value"
                    ) { value ->
                        Text(
                            text = value,
                            style = MaterialTheme.typography.titleMedium,
                            color = MaterialTheme.colorScheme.onSurface
                        )
                    }
                }
            }
            AreaChart(
                values = history.map { it.toFloat() },
                maxValue = maxReplicas.toFloat().coerceAtLeast(1f),
                lineColor = MaterialTheme.colorScheme.primary,
                fillColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.18f),
                modifier = Modifier
                    .fillMaxWidth()
                    .height(56.dp)
            )
            Text(
                text = "Availability ${((animatedRatio * 100f).toInt())}%",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
