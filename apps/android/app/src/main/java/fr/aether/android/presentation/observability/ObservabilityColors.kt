package fr.aether.android.presentation.observability

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable

data class MetricColors(
    val container: androidx.compose.ui.graphics.Color,
    val content: androidx.compose.ui.graphics.Color
)

@Composable
fun usageColors(usage: Float): MetricColors {
    val scheme = MaterialTheme.colorScheme
    return when {
        usage >= 85f -> MetricColors(scheme.errorContainer, scheme.onErrorContainer)
        usage >= 70f -> MetricColors(scheme.tertiaryContainer, scheme.onTertiaryContainer)
        else -> MetricColors(scheme.primaryContainer, scheme.onPrimaryContainer)
    }
}
