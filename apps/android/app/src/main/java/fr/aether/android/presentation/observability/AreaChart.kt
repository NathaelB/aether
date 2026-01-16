package fr.aether.android.presentation.observability

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp

@Composable
fun AreaChart(
    values: List<Float>,
    maxValue: Float,
    lineColor: androidx.compose.ui.graphics.Color,
    fillColor: androidx.compose.ui.graphics.Color,
    modifier: Modifier = Modifier
) {
    if (values.size < 2) return
    val baselineColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.25f)
    Canvas(modifier = modifier.fillMaxSize()) {
        val width = size.width
        val height = size.height
        val max = maxValue.coerceAtLeast(1f)
        val stepX = width / (values.size - 1)

        val linePath = Path()
        val fillPath = Path()

        values.forEachIndexed { index, value ->
            val x = stepX * index
            val y = height - (value.coerceIn(0f, max) / max) * height
            if (index == 0) {
                linePath.moveTo(x, y)
                fillPath.moveTo(x, height)
                fillPath.lineTo(x, y)
            } else {
                linePath.lineTo(x, y)
                fillPath.lineTo(x, y)
            }
        }
        fillPath.lineTo(width, height)
        fillPath.close()

        drawPath(fillPath, fillColor)
        drawPath(
            linePath,
            color = lineColor,
            style = Stroke(width = 2.dp.toPx(), cap = StrokeCap.Round)
        )

        drawLine(
            color = baselineColor,
            start = Offset(0f, height),
            end = Offset(width, height),
            strokeWidth = 1.dp.toPx()
        )
    }
}
