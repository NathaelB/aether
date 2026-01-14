package fr.aether.android.presentation.observability

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

@Composable
fun MemoryUsageCard(
    memoryUsage: Float,
    history: List<Float>,
    modifier: Modifier = Modifier
) {
    val colors = usageColors(memoryUsage)
    val containerColor by animateColorAsState(
        targetValue = colors.container,
        animationSpec = tween(260),
        label = "memory_container"
    )
    val contentColor by animateColorAsState(
        targetValue = colors.content,
        animationSpec = tween(260),
        label = "memory_content"
    )
    val animatedUsage by animateFloatAsState(
        targetValue = memoryUsage.coerceIn(0f, 100f),
        animationSpec = tween(350),
        label = "memory_usage"
    )

    Card(
        modifier = modifier
            .fillMaxWidth()
            .semantics { contentDescription = "Memory usage ${animatedUsage.toInt()} percent" },
        colors = CardDefaults.cardColors(containerColor = containerColor),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(
                text = "Memory",
                style = MaterialTheme.typography.labelLarge,
                color = contentColor
            )
            Text(
                text = "${animatedUsage.toInt()}%",
                style = MaterialTheme.typography.displaySmall,
                color = contentColor
            )
            AreaChart(
                values = history,
                maxValue = 100f,
                lineColor = contentColor,
                fillColor = contentColor.copy(alpha = 0.2f),
                modifier = Modifier
                    .fillMaxWidth()
                    .height(64.dp)
            )
            Text(
                text = "Last minute",
                style = MaterialTheme.typography.labelSmall,
                color = contentColor
            )
        }
    }
}
