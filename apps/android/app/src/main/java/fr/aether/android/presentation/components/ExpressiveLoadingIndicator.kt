package fr.aether.android.presentation.components

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ProgressIndicatorDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import fr.aether.android.ui.theme.AndroidTheme

/**
 * Material 3 Expressive Circular Loading Indicator
 *
 * This component provides an indeterminate circular progress indicator
 * with Material 3's expressive motion design patterns.
 *
 * @param modifier The modifier to be applied to the indicator
 * @param size The size of the circular indicator
 * @param strokeWidth The width of the indicator's stroke
 * @param color The color of the indicator (defaults to primary color)
 * @param trackColor The color of the track behind the indicator
 */
@Composable
fun ExpressiveCircularLoadingIndicator(
    modifier: Modifier = Modifier,
    size: Dp = 48.dp,
    strokeWidth: Dp = 4.dp,
    color: Color = MaterialTheme.colorScheme.primary,
    trackColor: Color = MaterialTheme.colorScheme.surfaceVariant
) {
    CircularProgressIndicator(
        modifier = modifier.size(size),
        color = color,
        strokeWidth = strokeWidth,
        trackColor = trackColor,
        strokeCap = StrokeCap.Round
    )
}

/**
 * Material 3 Expressive Linear Loading Indicator
 *
 * This component provides an indeterminate linear progress indicator
 * with Material 3's expressive motion design patterns.
 *
 * @param modifier The modifier to be applied to the indicator
 * @param color The color of the indicator (defaults to primary color)
 * @param trackColor The color of the track behind the indicator
 * @param strokeCap The stroke cap style for the indicator ends
 * @param gapSize The size of the gap between indicator segments
 */
@Composable
fun ExpressiveLinearLoadingIndicator(
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.primary,
    trackColor: Color = MaterialTheme.colorScheme.surfaceVariant,
    strokeCap: StrokeCap = StrokeCap.Round,
    gapSize: Dp = 0.dp
) {
    LinearProgressIndicator(
        modifier = modifier.fillMaxWidth(),
        color = color,
        trackColor = trackColor,
        strokeCap = strokeCap,
        gapSize = gapSize
    )
}

/**
 * Material 3 Expressive Determinate Circular Loading Indicator
 *
 * Shows a circular progress indicator with a specific progress value.
 *
 * @param progress The progress value between 0.0 and 1.0
 * @param modifier The modifier to be applied to the indicator
 * @param size The size of the circular indicator
 * @param strokeWidth The width of the indicator's stroke
 * @param color The color of the indicator
 * @param trackColor The color of the track behind the indicator
 */
@Composable
fun ExpressiveDeterminateCircularIndicator(
    progress: Float,
    modifier: Modifier = Modifier,
    size: Dp = 48.dp,
    strokeWidth: Dp = 4.dp,
    color: Color = MaterialTheme.colorScheme.primary,
    trackColor: Color = MaterialTheme.colorScheme.surfaceVariant
) {
    CircularProgressIndicator(
        progress = { progress },
        modifier = modifier.size(size),
        color = color,
        strokeWidth = strokeWidth,
        trackColor = trackColor,
        strokeCap = StrokeCap.Round
    )
}

/**
 * Material 3 Expressive Determinate Linear Loading Indicator
 *
 * Shows a linear progress indicator with a specific progress value.
 *
 * @param progress The progress value between 0.0 and 1.0
 * @param modifier The modifier to be applied to the indicator
 * @param color The color of the indicator
 * @param trackColor The color of the track behind the indicator
 * @param strokeCap The stroke cap style for the indicator ends
 * @param gapSize The size of the gap between indicator segments
 */
@Composable
fun ExpressiveDeterminateLinearIndicator(
    progress: Float,
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.primary,
    trackColor: Color = MaterialTheme.colorScheme.surfaceVariant,
    strokeCap: StrokeCap = StrokeCap.Round,
    gapSize: Dp = 0.dp
) {
    LinearProgressIndicator(
        progress = { progress },
        modifier = modifier.fillMaxWidth(),
        color = color,
        trackColor = trackColor,
        strokeCap = strokeCap,
        gapSize = gapSize
    )
}

/**
 * Complete loading state component with text and circular indicator
 *
 * @param text The loading message to display
 * @param modifier The modifier to be applied to the container
 * @param indicatorSize The size of the circular indicator
 */
@Composable
fun ExpressiveLoadingState(
    text: String,
    modifier: Modifier = Modifier,
    indicatorSize: Dp = 48.dp
) {
    Column(
        modifier = modifier.padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        ExpressiveCircularLoadingIndicator(size = indicatorSize)
        Text(
            text = text,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface
        )
    }
}

// Previews
@Preview(name = "Circular Indicator - Light", showBackground = true)
@Composable
private fun ExpressiveCircularLoadingIndicatorPreview() {
    AndroidTheme {
        Surface {
            Column(
                modifier = Modifier.padding(24.dp),
                verticalArrangement = Arrangement.spacedBy(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Text("Small (32dp)", style = MaterialTheme.typography.labelMedium)
                ExpressiveCircularLoadingIndicator(size = 32.dp, strokeWidth = 3.dp)

                Text("Medium (48dp)", style = MaterialTheme.typography.labelMedium)
                ExpressiveCircularLoadingIndicator(size = 48.dp, strokeWidth = 4.dp)

                Text("Large (64dp)", style = MaterialTheme.typography.labelMedium)
                ExpressiveCircularLoadingIndicator(size = 64.dp, strokeWidth = 5.dp)
            }
        }
    }
}

@Preview(name = "Linear Indicator", showBackground = true)
@Composable
private fun ExpressiveLinearLoadingIndicatorPreview() {
    AndroidTheme {
        Surface {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(24.dp),
                verticalArrangement = Arrangement.spacedBy(24.dp)
            ) {
                Text("Indeterminate Linear", style = MaterialTheme.typography.labelMedium)
                ExpressiveLinearLoadingIndicator()
            }
        }
    }
}

@Preview(name = "Determinate Indicators", showBackground = true)
@Composable
private fun ExpressiveDeterminateIndicatorsPreview() {
    AndroidTheme {
        Surface {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(24.dp),
                verticalArrangement = Arrangement.spacedBy(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Text("Circular - 25%", style = MaterialTheme.typography.labelMedium)
                ExpressiveDeterminateCircularIndicator(progress = 0.25f)

                Text("Circular - 75%", style = MaterialTheme.typography.labelMedium)
                ExpressiveDeterminateCircularIndicator(progress = 0.75f)

                Text("Linear - 50%", style = MaterialTheme.typography.labelMedium)
                ExpressiveDeterminateLinearIndicator(progress = 0.5f)
            }
        }
    }
}

@Preview(name = "Loading State", showBackground = true)
@Composable
private fun ExpressiveLoadingStatePreview() {
    AndroidTheme {
        Surface {
            ExpressiveLoadingState(
                text = "Chargement des déploiements...",
                modifier = Modifier.fillMaxWidth()
            )
        }
    }
}
