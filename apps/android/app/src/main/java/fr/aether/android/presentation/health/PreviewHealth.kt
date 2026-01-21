package fr.aether.android.presentation.health

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview
import fr.aether.android.ui.theme.AndroidTheme

@Preview(showBackground = true)
@Composable
private fun HealthSectionPreview() {
    AndroidTheme {
        DeploymentHealthSection(
            state = HealthUiState.Data(
                DeploymentHealth(
                    score = 82,
                    level = HealthLevel.DEGRADED,
                    summary = "High memory usage detected"
                )
            )
        )
    }
}
