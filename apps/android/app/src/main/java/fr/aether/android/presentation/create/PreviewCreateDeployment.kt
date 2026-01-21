package fr.aether.android.presentation.create

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview
import fr.aether.android.domain.model.CpuPreset
import fr.aether.android.domain.model.Environment
import fr.aether.android.domain.model.MemoryPreset
import fr.aether.android.ui.theme.AndroidTheme

@Preview(showBackground = true)
@Composable
private fun CreateDeploymentFormPreview() {
    AndroidTheme {
        CreateDeploymentScreen(
            uiState = CreateDeploymentUiState(
                name = "Aether API",
                environment = Environment.STAGING,
                replicas = 3,
                cpuPreset = CpuPreset.MEDIUM,
                memoryPreset = MemoryPreset.MEDIUM,
                autoScalingEnabled = true
            ),
            onBack = {},
            onNameChange = {},
            onEnvironmentChange = {},
            onReplicasChange = {},
            onCpuPresetChange = {},
            onMemoryPresetChange = {},
            onAutoScalingChange = {},
            onReview = {},
            onEdit = {},
            onCreate = {},
            onRetry = {},
            onDone = {}
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun CreateDeploymentReviewPreview() {
    AndroidTheme {
        CreateDeploymentScreen(
            uiState = CreateDeploymentUiState(
                step = CreateDeploymentStep.REVIEW,
                name = "Aether API",
                environment = Environment.PROD,
                replicas = 4,
                cpuPreset = CpuPreset.LARGE,
                memoryPreset = MemoryPreset.MEDIUM,
                autoScalingEnabled = false
            ),
            onBack = {},
            onNameChange = {},
            onEnvironmentChange = {},
            onReplicasChange = {},
            onCpuPresetChange = {},
            onMemoryPresetChange = {},
            onAutoScalingChange = {},
            onReview = {},
            onEdit = {},
            onCreate = {},
            onRetry = {},
            onDone = {}
        )
    }
}
