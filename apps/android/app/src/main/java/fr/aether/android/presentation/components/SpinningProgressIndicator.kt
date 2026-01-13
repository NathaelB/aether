package fr.aether.android.presentation.components

import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Spinning progress indicator with Material 3 expressive motion.
 *
 * This is a wrapper around Material 3's CircularProgressIndicator with
 * enhanced expressive design features including track color and rounded caps.
 *
 * @param modifier The modifier to be applied to the indicator
 * @param indicatorSize The size of the circular indicator
 * @param strokeWidth The width of the indicator's stroke
 * @param color The color of the indicator (defaults to primary color)
 * @param trackColor The color of the track behind the indicator
 */
@Composable
fun SpinningProgressIndicator(
    modifier: Modifier = Modifier,
    indicatorSize: Dp = 48.dp,
    strokeWidth: Dp = 4.dp,
    color: Color = MaterialTheme.colorScheme.primary,
    trackColor: Color = MaterialTheme.colorScheme.surfaceVariant
) {
    ExpressiveCircularLoadingIndicator(
        modifier = modifier.size(indicatorSize),
        color = color,
        strokeWidth = strokeWidth,
        trackColor = trackColor
    )
}
