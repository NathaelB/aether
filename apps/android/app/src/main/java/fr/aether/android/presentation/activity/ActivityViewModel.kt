package fr.aether.android.presentation.activity

import androidx.lifecycle.ViewModel
import fr.aether.android.domain.model.DeploymentStatus
import fr.aether.android.presentation.health.DeploymentHealth
import java.time.Instant
import java.util.UUID
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

class ActivityViewModel : ViewModel() {
    private val _uiState = MutableStateFlow<ActivityUiState>(ActivityUiState.Loading)
    val uiState: StateFlow<ActivityUiState> = _uiState.asStateFlow()

    private var deploymentId: String? = null
    private val events = mutableListOf<DeploymentEvent>()
    private var lastHealthLevel: String? = null
    private var lastHealthScore: Int? = null

    fun initialize(id: String, status: DeploymentStatus) {
        if (deploymentId == id && _uiState.value !is ActivityUiState.Loading) return
        deploymentId = id
        events.clear()
        seedEvents(status)
        publish()
    }

    fun recordAction(message: String, severity: EventSeverity = EventSeverity.INFO) {
        addEvent(EventType.ACTION, message, severity)
    }

    fun recordScale(from: Int, to: Int, reason: String? = null) {
        val base = "Replicas scaled from $from to $to"
        val message = if (reason.isNullOrBlank()) base else "$base — $reason"
        addEvent(EventType.SCALING, message, EventSeverity.INFO)
    }

    fun recordMaintenance(enabled: Boolean) {
        addEvent(
            EventType.ACTION,
            if (enabled) "Maintenance mode enabled" else "Maintenance mode disabled",
            EventSeverity.INFO
        )
    }

    fun recordAlert(message: String, severity: EventSeverity) {
        addEvent(EventType.ALERT, message, severity)
    }

    fun onHealthUpdate(health: DeploymentHealth) {
        val previousLevel = lastHealthLevel
        val previousScore = lastHealthScore
        lastHealthLevel = health.level.name
        lastHealthScore = health.score
        if (previousLevel == null) return
        if (previousLevel != health.level.name) {
            val message = when (health.level) {
                fr.aether.android.presentation.health.HealthLevel.HEALTHY -> "Health recovered to stable"
                fr.aether.android.presentation.health.HealthLevel.DEGRADED -> "Health degraded slightly"
                fr.aether.android.presentation.health.HealthLevel.UNSTABLE -> "Health becoming unstable"
                fr.aether.android.presentation.health.HealthLevel.CRITICAL -> "Health is critical"
            }
            val severity = when (health.level) {
                fr.aether.android.presentation.health.HealthLevel.CRITICAL -> EventSeverity.CRITICAL
                fr.aether.android.presentation.health.HealthLevel.UNSTABLE -> EventSeverity.WARNING
                else -> EventSeverity.INFO
            }
            addEvent(EventType.SYSTEM, message, severity)
            return
        }
        if (previousScore != null && kotlin.math.abs(health.score - previousScore) >= 8) {
            val direction = if (health.score > previousScore) "improved" else "declined"
            addEvent(
                EventType.SYSTEM,
                "Health score $direction to ${health.score}",
                EventSeverity.INFO
            )
        }
    }

    fun recordStatus(status: DeploymentStatus) {
        val message = when (status) {
            DeploymentStatus.RUNNING -> "Deployment is running"
            DeploymentStatus.DEPLOYING -> "Deployment update in progress"
            DeploymentStatus.STOPPED -> "Deployment paused"
            DeploymentStatus.FAILED -> "Deployment reported a failure"
        }
        val severity = when (status) {
            DeploymentStatus.FAILED -> EventSeverity.CRITICAL
            DeploymentStatus.STOPPED -> EventSeverity.WARNING
            else -> EventSeverity.INFO
        }
        addEvent(EventType.STATUS_CHANGE, message, severity)
    }

    private fun seedEvents(status: DeploymentStatus) {
        val now = Instant.now()
        val seed = deploymentId?.hashCode()?.let { kotlin.math.abs(it) } ?: 1
        val baseEvents = listOf(
            DeploymentEvent(
                id = UUID.randomUUID().toString(),
                type = EventType.STATUS_CHANGE,
                message = when (status) {
                    DeploymentStatus.RUNNING -> "Deployment running smoothly"
                    DeploymentStatus.DEPLOYING -> "Deployment update started"
                    DeploymentStatus.STOPPED -> "Deployment paused for review"
                    DeploymentStatus.FAILED -> "Deployment reported a failure"
                },
                timestamp = now.minusSeconds(120),
                severity = when (status) {
                    DeploymentStatus.FAILED -> EventSeverity.CRITICAL
                    DeploymentStatus.STOPPED -> EventSeverity.WARNING
                    else -> EventSeverity.INFO
                }
            ),
            DeploymentEvent(
                id = UUID.randomUUID().toString(),
                type = EventType.SYSTEM,
                message = "Health score stabilized",
                timestamp = now.minusSeconds(420),
                severity = EventSeverity.INFO
            )
        )
        events.addAll(baseEvents)
        if (seed % 2 == 0) {
            events.add(
                DeploymentEvent(
                    id = UUID.randomUUID().toString(),
                    type = EventType.SCALING,
                    message = "Replicas scaled from 2 to 4 due to traffic",
                    timestamp = now.minusSeconds(780),
                    severity = EventSeverity.INFO
                )
            )
        } else {
            events.add(
                DeploymentEvent(
                    id = UUID.randomUUID().toString(),
                    type = EventType.ALERT,
                    message = "CPU usage exceeded 70%",
                    timestamp = now.minusSeconds(960),
                    severity = EventSeverity.WARNING
                )
            )
        }
    }

    private fun addEvent(type: EventType, message: String, severity: EventSeverity) {
        val event = DeploymentEvent(
            id = UUID.randomUUID().toString(),
            type = type,
            message = message,
            timestamp = Instant.now(),
            severity = severity
        )
        events.add(0, event)
        publish()
    }

    private fun publish() {
        if (events.isEmpty()) {
            _uiState.value = ActivityUiState.Empty()
        } else {
            _uiState.value = ActivityUiState.Data(events.toList())
        }
    }
}
