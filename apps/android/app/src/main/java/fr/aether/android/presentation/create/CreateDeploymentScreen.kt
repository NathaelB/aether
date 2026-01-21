package fr.aether.android.presentation.create

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ArrowBack
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Remove
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import fr.aether.android.domain.model.CpuPreset
import fr.aether.android.domain.model.Environment
import fr.aether.android.domain.model.MemoryPreset

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun CreateDeploymentScreen(
    uiState: CreateDeploymentUiState,
    onBack: () -> Unit,
    onNameChange: (String) -> Unit,
    onEnvironmentChange: (Environment) -> Unit,
    onReplicasChange: (Int) -> Unit,
    onCpuPresetChange: (CpuPreset) -> Unit,
    onMemoryPresetChange: (MemoryPreset) -> Unit,
    onAutoScalingChange: (Boolean) -> Unit,
    onReview: () -> Unit,
    onEdit: () -> Unit,
    onCreate: () -> Unit,
    onRetry: () -> Unit,
    onDone: () -> Unit,
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.fillMaxWidth()
        ) {
            IconButton(onClick = onBack) {
                Icon(imageVector = Icons.Outlined.ArrowBack, contentDescription = "Back")
            }
            Text(
                text = "Create deployment",
                style = MaterialTheme.typography.titleLarge
            )
        }

        AnimatedContent(
            targetState = uiState.step,
            transitionSpec = { fadeIn(tween(220)) togetherWith fadeOut(tween(160)) },
            label = "create_step"
        ) { step ->
            when (step) {
                CreateDeploymentStep.EDIT -> CreateDeploymentForm(
                    uiState = uiState,
                    onNameChange = onNameChange,
                    onEnvironmentChange = onEnvironmentChange,
                    onReplicasChange = onReplicasChange,
                    onCpuPresetChange = onCpuPresetChange,
                    onMemoryPresetChange = onMemoryPresetChange,
                    onAutoScalingChange = onAutoScalingChange,
                    onReview = onReview
                )
                CreateDeploymentStep.REVIEW -> ReviewStep(
                    uiState = uiState,
                    onEdit = onEdit,
                    onCreate = onCreate
                )
                CreateDeploymentStep.CREATING -> CreatingStep(
                    message = uiState.progressMessage
                )
                CreateDeploymentStep.SUCCESS -> SuccessStep(
                    name = uiState.name,
                    onDone = onDone
                )
                CreateDeploymentStep.ERROR -> ErrorStep(
                    message = uiState.errorMessage ?: "Unable to create deployment.",
                    onBack = onEdit,
                    onRetry = onRetry
                )
            }
        }
    }
}

@Composable
private fun CreateDeploymentForm(
    uiState: CreateDeploymentUiState,
    onNameChange: (String) -> Unit,
    onEnvironmentChange: (Environment) -> Unit,
    onReplicasChange: (Int) -> Unit,
    onCpuPresetChange: (CpuPreset) -> Unit,
    onMemoryPresetChange: (MemoryPreset) -> Unit,
    onAutoScalingChange: (Boolean) -> Unit,
    onReview: () -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        Card(
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.surfaceContainer
            ),
            shape = MaterialTheme.shapes.large
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                Text(
                    text = "Basic details",
                    style = MaterialTheme.typography.titleMedium
                )
                OutlinedTextField(
                    value = uiState.name,
                    onValueChange = onNameChange,
                    label = { Text(text = "Deployment name") },
                    supportingText = {
                        Text(text = uiState.nameError ?: "Clear, human-readable name.")
                    },
                    isError = uiState.nameError != null,
                    modifier = Modifier.fillMaxWidth()
                )
                Text(
                    text = "Environment",
                    style = MaterialTheme.typography.labelLarge
                )
                SegmentedRow(
                    options = Environment.values().toList(),
                    selected = uiState.environment,
                    label = { it.displayName() },
                    onSelected = onEnvironmentChange
                )
            }
        }

        Card(
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.surfaceContainer
            ),
            shape = MaterialTheme.shapes.large
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                Text(text = "Capacity", style = MaterialTheme.typography.titleMedium)
                StepperRow(
                    label = "Replicas",
                    value = uiState.replicas,
                    min = 1,
                    max = 8,
                    onValueChange = onReplicasChange
                )
                Text(
                    text = "CPU preset",
                    style = MaterialTheme.typography.labelLarge
                )
                SegmentedRow(
                    options = CpuPreset.values().toList(),
                    selected = uiState.cpuPreset,
                    label = { it.displayName() },
                    onSelected = onCpuPresetChange
                )
                Text(
                    text = "Memory preset",
                    style = MaterialTheme.typography.labelLarge
                )
                SegmentedRow(
                    options = MemoryPreset.values().toList(),
                    selected = uiState.memoryPreset,
                    label = { it.displayName() },
                    onSelected = onMemoryPresetChange
                )
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Column {
                        Text(
                            text = "Autoscaling",
                            style = MaterialTheme.typography.labelLarge
                        )
                        Text(
                            text = "Scale safely under load.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    Switch(
                        checked = uiState.autoScalingEnabled,
                        onCheckedChange = onAutoScalingChange
                    )
                }
            }
        }

        Button(
            onClick = onReview,
            modifier = Modifier.fillMaxWidth(),
            enabled = uiState.name.isNotBlank()
        ) {
            Text(text = "Review deployment")
        }
    }
}

@Composable
private fun ReviewStep(
    uiState: CreateDeploymentUiState,
    onEdit: () -> Unit,
    onCreate: () -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        ReviewDeploymentScreen(
            name = uiState.name,
            environment = uiState.environment,
            replicas = uiState.replicas,
            cpuPreset = uiState.cpuPreset,
            memoryPreset = uiState.memoryPreset,
            autoScalingEnabled = uiState.autoScalingEnabled
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            FilledTonalButton(
                onClick = onEdit,
                modifier = Modifier.weight(1f)
            ) {
                Text(text = "Edit")
            }
            Button(
                onClick = onCreate,
                modifier = Modifier.weight(1f)
            ) {
                Text(text = "Create deployment")
            }
        }
    }
}

@Composable
private fun CreatingStep(
    message: String
) {
    Card(
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainer
        ),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(text = "Provisioning", style = MaterialTheme.typography.titleMedium)
            Text(
                text = message,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
        }
    }
}

@Composable
private fun SuccessStep(
    name: String,
    onDone: () -> Unit
) {
    Card(
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainer
        ),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Text(text = "Deployment created", style = MaterialTheme.typography.titleMedium)
            Text(
                text = "$name is being prepared. We'll open the deployment details.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Button(onClick = onDone, modifier = Modifier.fillMaxWidth()) {
                Text(text = "View deployment")
            }
        }
    }
}

@Composable
private fun ErrorStep(
    message: String,
    onBack: () -> Unit,
    onRetry: () -> Unit
) {
    Card(
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainer
        ),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Text(text = "Unable to create", style = MaterialTheme.typography.titleMedium)
            Text(
                text = message,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Row(
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                modifier = Modifier.fillMaxWidth()
            ) {
                TextButton(
                    onClick = onBack,
                    modifier = Modifier.weight(1f)
                ) {
                    Text(text = "Back")
                }
                Button(
                    onClick = onRetry,
                    modifier = Modifier.weight(1f)
                ) {
                    Text(text = "Retry")
                }
            }
        }
    }
}

@Composable
private fun <T> SegmentedRow(
    options: List<T>,
    selected: T,
    label: (T) -> String,
    onSelected: (T) -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        options.forEach { option ->
            FilterChip(
                selected = option == selected,
                onClick = { onSelected(option) },
                label = { Text(text = label(option)) },
                modifier = Modifier.weight(1f)
            )
        }
    }
}

@Composable
private fun StepperRow(
    label: String,
    value: Int,
    min: Int,
    max: Int,
    onValueChange: (Int) -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Column {
            Text(text = label, style = MaterialTheme.typography.labelLarge)
            Text(
                text = "Recommended: 2-4 replicas",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconButton(
                onClick = { if (value > min) onValueChange(value - 1) },
                enabled = value > min
            ) {
                Icon(imageVector = Icons.Outlined.Remove, contentDescription = "Decrease")
            }
            Text(
                text = value.toString(),
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.width(32.dp)
            )
            IconButton(
                onClick = { if (value < max) onValueChange(value + 1) },
                enabled = value < max
            ) {
                Icon(imageVector = Icons.Outlined.Add, contentDescription = "Increase")
            }
        }
    }
}

private fun Environment.displayName() = when (this) {
    Environment.DEV -> "Dev"
    Environment.STAGING -> "Staging"
    Environment.PROD -> "Prod"
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
