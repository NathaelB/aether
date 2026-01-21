package fr.aether.android.presentation.activity

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Build
import androidx.compose.material.icons.outlined.Notifications
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material.icons.outlined.SignalCellularAlt
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import java.time.Duration
import java.time.Instant

@Composable
fun ActivityEventItem(
    event: DeploymentEvent,
    modifier: Modifier = Modifier
) {
    val severityColor by animateColorAsState(
        targetValue = severityContainer(event.severity),
        animationSpec = tween(300),
        label = "severity_container"
    )
    Surface(
        modifier = modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        shape = MaterialTheme.shapes.large
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Box(
                modifier = Modifier
                    .size(36.dp)
                    .background(severityColor, CircleShape),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = iconForType(event.type),
                    contentDescription = null,
                    tint = onSeverityContainer(event.severity)
                )
            }
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = event.message,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis
                )
                Spacer(modifier = Modifier.size(4.dp))
                Text(
                    text = relativeTime(event.timestamp),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            if (event.severity != EventSeverity.INFO) {
                SeverityPill(severity = event.severity)
            }
        }
    }
}

@Composable
private fun SeverityPill(severity: EventSeverity) {
    Surface(
        color = severityContainer(severity),
        shape = CircleShape
    ) {
        Text(
            text = when (severity) {
                EventSeverity.INFO -> "Info"
                EventSeverity.WARNING -> "Warning"
                EventSeverity.CRITICAL -> "Critical"
            },
            style = MaterialTheme.typography.labelSmall,
            color = onSeverityContainer(severity),
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp)
        )
    }
}

@Composable
private fun severityContainer(severity: EventSeverity) = when (severity) {
    EventSeverity.INFO -> MaterialTheme.colorScheme.surfaceContainer
    EventSeverity.WARNING -> MaterialTheme.colorScheme.tertiaryContainer
    EventSeverity.CRITICAL -> MaterialTheme.colorScheme.errorContainer
}

@Composable
private fun onSeverityContainer(severity: EventSeverity) = when (severity) {
    EventSeverity.INFO -> MaterialTheme.colorScheme.onSurfaceVariant
    EventSeverity.WARNING -> MaterialTheme.colorScheme.onTertiaryContainer
    EventSeverity.CRITICAL -> MaterialTheme.colorScheme.onErrorContainer
}

private fun iconForType(type: EventType) = when (type) {
    EventType.ACTION -> Icons.Outlined.Refresh
    EventType.ALERT -> Icons.Outlined.Notifications
    EventType.SCALING -> Icons.Outlined.Tune
    EventType.STATUS_CHANGE -> Icons.Outlined.SignalCellularAlt
    EventType.SYSTEM -> Icons.Outlined.Build
}

private fun relativeTime(timestamp: Instant): String {
    val now = Instant.now()
    val rawDuration = Duration.between(timestamp, now)
    val duration = if (rawDuration.isNegative) rawDuration.negated() else rawDuration
    val minutes = duration.toMinutes()
    val hours = duration.toHours()
    return when {
        minutes < 1 -> "Just now"
        minutes < 60 -> "$minutes min ago"
        hours < 24 -> "${hours}h ago"
        else -> "${duration.toDays()}d ago"
    }
}
