package fr.aether.android.presentation.deployments

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import fr.aether.android.domain.model.DeploymentStatus
import fr.aether.android.domain.model.IamProvider
import fr.aether.android.ui.theme.AndroidTheme
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import fr.aether.android.presentation.observability.ObservabilitySection
import fr.aether.android.presentation.observability.ObservabilityViewModel
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ArrowBack
import fr.aether.android.notifications.CpuAlertNotifier
import fr.aether.android.notifications.AlertPreferences
import java.util.Locale

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun DeploymentDetailScreen(
    deployment: DeploymentUiModel?,
    isLoading: Boolean = false,
    onBack: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    if (isLoading) {
        Column(
            modifier = modifier
                .fillMaxSize()
                .padding(24.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            LoadingIndicator()
            Spacer(modifier = Modifier.height(12.dp))
            Text(
                text = "Loading deployment details...",
                style = MaterialTheme.typography.bodyLarge
            )
        }
        return
    }
    if (deployment == null) {
        Column(
            modifier = modifier
                .fillMaxSize()
                .padding(24.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Text(
                text = "Deployment not found.",
                style = MaterialTheme.typography.bodyLarge
            )
        }
        return
    }

    val observabilityViewModel: ObservabilityViewModel = viewModel()
    val observabilityState by observabilityViewModel.uiState.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val requiresNotificationPermission = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU
    val hasNotificationPermission = remember {
        mutableStateOf(isNotificationPermissionGranted(context))
    }
    val permissionLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.RequestPermission()
    ) { granted ->
        hasNotificationPermission.value = granted
    }
    val activity = context as? Activity
    val shouldShowRationale = requiresNotificationPermission &&
        !hasNotificationPermission.value &&
        activity != null &&
        ActivityCompat.shouldShowRequestPermissionRationale(
            activity,
            Manifest.permission.POST_NOTIFICATIONS
        )
    LaunchedEffect(Unit) {
        hasNotificationPermission.value = isNotificationPermissionGranted(context)
    }
    val cpuThreshold = rememberSaveable { mutableStateOf(AlertPreferences.cpuThreshold(context)) }
    val memoryThreshold = rememberSaveable { mutableStateOf(AlertPreferences.memoryThreshold(context)) }
    val lastCpuAlertAt = rememberSaveable { mutableStateOf(0L) }
    val lastMemoryAlertAt = rememberSaveable { mutableStateOf(0L) }
    val cooldownMs = 5 * 60 * 1000L
    LaunchedEffect(observabilityState, hasNotificationPermission.value) {
        val data = observabilityState as? fr.aether.android.presentation.observability.ObservabilityUiState.Data
            ?: return@LaunchedEffect
        val shouldNotify = !requiresNotificationPermission || hasNotificationPermission.value
        val now = System.currentTimeMillis()
        if (shouldNotify && data.metrics.cpuUsage >= cpuThreshold.value && now - lastCpuAlertAt.value >= cooldownMs) {
            CpuAlertNotifier.showHighCpu(context, deployment.name, data.metrics.cpuUsage)
            lastCpuAlertAt.value = now
        }
        if (shouldNotify && data.metrics.memoryUsage >= memoryThreshold.value && now - lastMemoryAlertAt.value >= cooldownMs) {
            CpuAlertNotifier.showHighMemory(context, deployment.name, data.metrics.memoryUsage)
            lastMemoryAlertAt.value = now
        }
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically
        ) {
            if (onBack != null) {
                IconButton(onClick = onBack) {
                    Icon(
                        imageVector = Icons.Outlined.ArrowBack,
                        contentDescription = "Back"
                    )
                }
            }
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                Text(
                    text = deployment.name,
                    style = MaterialTheme.typography.displaySmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    EnvironmentBadge(environment = deployment.environment)
                    DeploymentStatusBadge(status = deployment.status)
                }
            }
        }
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.surfaceContainerLow
            ),
            elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
            shape = MaterialTheme.shapes.large
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = deployment.name,
                        style = MaterialTheme.typography.titleLarge,
                        modifier = Modifier.weight(1f),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    DeploymentStatusBadge(status = deployment.status)
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = deployment.provider.displayName(),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.weight(1f)
                    )
                    EnvironmentBadge(environment = deployment.environment)
                }
            }
        }

        InfoCard(
            title = "Cluster",
            lines = listOf(
                "Cluster: ${deployment.cluster}",
                "Namespace: ${deployment.namespace}",
                "Region: ${deployment.region}"
            )
        )
        InfoCard(
            title = "Runtime",
            lines = listOf(
                "Version: ${deployment.version}",
                "Endpoint: ${deployment.endpoint}",
                "Updated: ${deployment.updatedAt}"
            )
        )
        AlertThresholdsCard(
            cpuThreshold = cpuThreshold.value,
            memoryThreshold = memoryThreshold.value,
            onCpuThresholdChange = { cpuThreshold.value = it },
            onMemoryThresholdChange = { memoryThreshold.value = it },
            onCpuThresholdSave = { AlertPreferences.setCpuThreshold(context, cpuThreshold.value) },
            onMemoryThresholdSave = { AlertPreferences.setMemoryThreshold(context, memoryThreshold.value) }
        )
        if (requiresNotificationPermission && !hasNotificationPermission.value) {
            NotificationPermissionCard(
                showRationale = shouldShowRationale,
                onEnable = {
                    permissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
                }
            )
        }
        ObservabilitySection(
            uiState = observabilityState,
            onRetry = observabilityViewModel::refresh
        )
    }
}

@Composable
private fun InfoCard(
    title: String,
    lines: List<String>
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
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

@Composable
private fun NotificationPermissionCard(
    showRationale: Boolean,
    onEnable: () -> Unit
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                text = "Enable CPU alerts",
                style = MaterialTheme.typography.titleMedium
            )
            Text(
                text = if (showRationale) {
                    "Notifications let us alert you when CPU or memory usage exceeds 70%."
                } else {
                    "Turn on notifications to get alerted when CPU or memory usage is high."
                },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            androidx.compose.material3.Button(onClick = onEnable) {
                Text(text = "Enable notifications")
            }
        }
    }
}

@Composable
private fun AlertThresholdsCard(
    cpuThreshold: Float,
    memoryThreshold: Float,
    onCpuThresholdChange: (Float) -> Unit,
    onMemoryThresholdChange: (Float) -> Unit,
    onCpuThresholdSave: () -> Unit,
    onMemoryThresholdSave: () -> Unit
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(
                text = "Alert thresholds",
                style = MaterialTheme.typography.titleMedium
            )
            ThresholdSliderRow(
                label = "CPU alert",
                value = cpuThreshold,
                onValueChange = onCpuThresholdChange,
                onValueChangeFinished = onCpuThresholdSave
            )
            ThresholdSliderRow(
                label = "Memory alert",
                value = memoryThreshold,
                onValueChange = onMemoryThresholdChange,
                onValueChangeFinished = onMemoryThresholdSave
            )
            Text(
                text = "Alerts fire when usage exceeds the selected percentage.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

@Composable
private fun ThresholdSliderRow(
    label: String,
    value: Float,
    onValueChange: (Float) -> Unit,
    onValueChangeFinished: () -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.weight(1f)
            )
            Text(
                text = formatPercent(value),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        Slider(
            value = value,
            onValueChange = onValueChange,
            valueRange = 50f..95f,
            steps = 8,
            onValueChangeFinished = onValueChangeFinished
        )
    }
}

private fun isNotificationPermissionGranted(context: android.content.Context): Boolean {
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.POST_NOTIFICATIONS
        ) == PackageManager.PERMISSION_GRANTED
    } else {
        true
    }
}

private fun formatPercent(value: Float): String {
    return String.format(Locale.US, "%.0f%%", value)
}

@Preview(showBackground = true)
@Composable
private fun DeploymentDetailScreenPreview() {
    AndroidTheme {
        DeploymentDetailScreen(
            deployment = DeploymentUiModel(
                id = "dep-1",
                name = "Keycloak - Core",
                environment = "Production",
                status = DeploymentStatus.RUNNING,
                provider = IamProvider.KEYCLOAK,
                cluster = "iam-prod-01",
                namespace = "keycloak",
                version = "24.0.2",
                endpoint = "https://iam.aether.io",
                region = "eu-west-1",
                updatedAt = "2024-08-12 10:24"
            ),
            onBack = {}
        )
    }
}
