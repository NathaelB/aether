package fr.aether.android.presentation.health

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun DeploymentHealthSection(
    state: HealthUiState,
    modifier: Modifier = Modifier
) {
    Card(
        modifier = modifier
            .fillMaxWidth()
            .animateContentSize(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainer
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text(
                text = "Health",
                style = MaterialTheme.typography.titleLarge
            )
            when (state) {
                HealthUiState.Loading -> HealthSkeleton()
                is HealthUiState.Error -> Text(
                    text = state.message.ifBlank { "Health unavailable" },
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                is HealthUiState.Data -> {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(16.dp)
                    ) {
                        HealthScoreIndicator(
                            score = state.health.score,
                            level = state.health.level,
                            compact = false
                        )
                        Column(
                            modifier = Modifier.weight(1f),
                            verticalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            Text(
                                text = "Overall stability",
                                style = MaterialTheme.typography.titleMedium
                            )
                            HealthSummary(summary = state.health.summary)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun HealthSkeleton() {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            PlaceholderBubble()
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                PlaceholderLine(widthFraction = 0.6f)
                PlaceholderLine(widthFraction = 0.8f)
            }
        }
    }
}

@Composable
private fun PlaceholderBubble() {
    val color = MaterialTheme.colorScheme.surfaceContainerHighest
    Box(
        modifier = Modifier
            .size(112.dp)
            .background(color, MaterialTheme.shapes.extraLarge)
    )
}

@Composable
private fun PlaceholderLine(widthFraction: Float) {
    val color = MaterialTheme.colorScheme.surfaceContainerHighest
    Spacer(
        modifier = Modifier
            .fillMaxWidth(widthFraction)
            .height(12.dp)
            .background(color, MaterialTheme.shapes.small)
    )
}
