package fr.aether.android.presentation.create

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.height
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import fr.aether.android.domain.model.CpuPreset
import fr.aether.android.domain.model.Environment
import fr.aether.android.domain.model.MemoryPreset

@Composable
fun ReviewDeploymentScreen(
    name: String,
    environment: Environment,
    replicas: Int,
    cpuPreset: CpuPreset,
    memoryPreset: MemoryPreset,
    autoScalingEnabled: Boolean,
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        ReviewCard(
            title = "Deployment summary",
            lines = listOf(
                "Name: $name",
                "Environment: ${environment.displayName()}",
                "Replicas: $replicas",
                "CPU preset: ${cpuPreset.displayName()}",
                "Memory preset: ${memoryPreset.displayName()}",
                "Autoscaling: ${if (autoScalingEnabled) "Enabled" else "Disabled"}"
            )
        )
        ReviewCard(
            title = "Estimated behavior",
            lines = listOf(
                "Estimated CPU per instance: ~${cpuEstimate(cpuPreset)}%",
                "Estimated memory per instance: ~${memoryEstimate(memoryPreset)}%",
                "Autoscaling policy: ${if (autoScalingEnabled) "Conservative" else "Manual"}"
            )
        )
        if (environment == Environment.PROD) {
            Card(
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.errorContainer
                ),
                shape = MaterialTheme.shapes.large,
                elevation = CardDefaults.cardElevation(defaultElevation = 0.dp)
            ) {
                Column(
                    modifier = Modifier.padding(14.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    Text(
                        text = "Production warning",
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.onErrorContainer
                    )
                    Text(
                        text = "This deployment will affect production traffic. Review settings carefully.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onErrorContainer
                    )
                }
            }
        }
    }
}

@Composable
private fun ReviewCard(
    title: String,
    lines: List<String>
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainer
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium
            )
            lines.forEach { line ->
                Text(
                    text = line,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

private fun Environment.displayName() = when (this) {
    Environment.DEV -> "Development"
    Environment.STAGING -> "Staging"
    Environment.PROD -> "Production"
}

private fun CpuPreset.displayName() = when (this) {
    CpuPreset.SMALL -> "Small"
    CpuPreset.MEDIUM -> "Medium"
    CpuPreset.LARGE -> "Large"
}

private fun MemoryPreset.displayName() = when (this) {
    MemoryPreset.SMALL -> "Small"
    MemoryPreset.MEDIUM -> "Medium"
    MemoryPreset.LARGE -> "Large"
}

private fun cpuEstimate(preset: CpuPreset) = when (preset) {
    CpuPreset.SMALL -> 22
    CpuPreset.MEDIUM -> 35
    CpuPreset.LARGE -> 55
}

private fun memoryEstimate(preset: MemoryPreset) = when (preset) {
    MemoryPreset.SMALL -> 28
    MemoryPreset.MEDIUM -> 42
    MemoryPreset.LARGE -> 60
}
