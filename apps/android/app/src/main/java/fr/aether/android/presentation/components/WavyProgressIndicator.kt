package fr.aether.android.presentation.components

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp

@Composable
fun WavyProgressIndicator(
    modifier: Modifier = Modifier,
    amplitudeDp: Float = 6f
) {
    val color = MaterialTheme.colorScheme.primary
    val transition = rememberInfiniteTransition(label = "wavy")
    val phase by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 1400, easing = LinearEasing),
            repeatMode = RepeatMode.Restart
        ),
        label = "phase"
    )
    Canvas(
        modifier = modifier
            .fillMaxWidth()
            .height(32.dp)
    ) {
        val amplitude = amplitudeDp.dp.toPx()
        val width = size.width
        val height = size.height / 2f
        val waveLength = width / 3f
        val path = Path()
        val step = width / 48f
        var x = 0f
        while (x <= width + step) {
            val y = height + amplitude *
                kotlin.math.sin(((x / waveLength) + phase) * 2f * kotlin.math.PI).toFloat()
            if (x == 0f) {
                path.moveTo(x, y)
            } else {
                path.lineTo(x, y)
            }
            x += step
        }
        drawPath(
            path = path,
            color = color,
            style = Stroke(width = 6.dp.toPx(), cap = StrokeCap.Round)
        )
    }
}
