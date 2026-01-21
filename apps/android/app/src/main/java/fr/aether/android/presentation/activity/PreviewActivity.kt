package fr.aether.android.presentation.activity

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview
import fr.aether.android.ui.theme.AndroidTheme
import java.time.Instant

@Preview(showBackground = true)
@Composable
private fun ActivitySectionPreview() {
    AndroidTheme {
        DeploymentActivitySection(
            state = ActivityUiState.Data(
                events = listOf(
                    DeploymentEvent(
                        id = "1",
                        type = EventType.SCALING,
                        message = "Replicas scaled from 2 to 4 due to high CPU usage",
                        timestamp = Instant.now().minusSeconds(120),
                        severity = EventSeverity.INFO
                    ),
                    DeploymentEvent(
                        id = "2",
                        type = EventType.ALERT,
                        message = "Memory usage exceeded 70%",
                        timestamp = Instant.now().minusSeconds(420),
                        severity = EventSeverity.WARNING
                    ),
                    DeploymentEvent(
                        id = "3",
                        type = EventType.STATUS_CHANGE,
                        message = "Deployment running smoothly",
                        timestamp = Instant.now().minusSeconds(900),
                        severity = EventSeverity.INFO
                    )
                )
            )
        )
    }
}
