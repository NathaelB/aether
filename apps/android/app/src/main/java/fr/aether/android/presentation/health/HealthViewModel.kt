package fr.aether.android.presentation.health

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import fr.aether.android.domain.model.DeploymentStatus
import fr.aether.android.presentation.observability.DeploymentMetrics
import kotlin.math.absoluteValue
import kotlin.math.roundToInt
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class HealthViewModel : ViewModel() {
    private val _uiState = MutableStateFlow<HealthUiState>(HealthUiState.Loading)
    val uiState: StateFlow<HealthUiState> = _uiState.asStateFlow()

    private var lastScore: Int? = null

    fun setLoading() {
        _uiState.value = HealthUiState.Loading
    }

    fun setError(message: String = "Health unavailable") {
        _uiState.value = HealthUiState.Error(message)
    }

    fun updateMetrics(metrics: DeploymentMetrics, status: DeploymentStatus) {
        viewModelScope.launch {
            val target = computeDeploymentHealth(metrics, status)
            val previous = lastScore ?: target.score
            val smoothed = smoothScore(previous, target.score)
            lastScore = smoothed
            val health = target.copy(
                score = smoothed,
                level = levelForScore(smoothed)
            )
            _uiState.value = HealthUiState.Data(health)
        }
    }
}

fun computeDeploymentHealth(metrics: DeploymentMetrics, status: DeploymentStatus): DeploymentHealth {
    var score = 100f
    val cpuPenalty = penaltyForUsage(metrics.cpuUsage, 70f, 0.6f, 22f)
    val memoryPenalty = penaltyForUsage(metrics.memoryUsage, 70f, 0.55f, 20f)
    val variationPenalty = variationPenalty(
        metrics.cpuHistory,
        metrics.memoryHistory
    )
    val replicasPenalty = when {
        metrics.replicas <= 1 -> 16f
        metrics.replicas == 2 -> 9f
        metrics.replicas == 3 -> 5f
        else -> 0f
    }
    val statusPenalty = when (status) {
        DeploymentStatus.RUNNING -> 0f
        DeploymentStatus.DEPLOYING -> 8f
        DeploymentStatus.STOPPED -> 32f
        DeploymentStatus.FAILED -> 46f
    }

    score -= cpuPenalty + memoryPenalty + variationPenalty + replicasPenalty + statusPenalty
    val clamped = score.coerceIn(0f, 100f).roundToInt()
    val level = levelForScore(clamped)
    val summary = buildSummary(metrics, status, cpuPenalty, memoryPenalty, variationPenalty)
    return DeploymentHealth(score = clamped, level = level, summary = summary)
}

fun computeMockHealth(deploymentId: String, status: DeploymentStatus): DeploymentHealth {
    val seed = deploymentId.hashCode().absoluteValue
    val cpu = 55f + (seed % 30)
    val memory = 52f + (seed % 28)
    val replicas = (seed % 5 + 2)
    val history = List(8) { cpu + ((it - 4) * 0.8f) }
    val memoryHistory = List(8) { memory + ((4 - it) * 0.7f) }
    val metrics = DeploymentMetrics(
        cpuUsage = cpu,
        memoryUsage = memory,
        replicas = replicas,
        maxReplicas = 8,
        cpuHistory = history,
        memoryHistory = memoryHistory,
        replicasHistory = List(8) { replicas }
    )
    return computeDeploymentHealth(metrics, status)
}

private fun smoothScore(previous: Int, target: Int): Int {
    val delta = (target - previous).coerceIn(-4, 4)
    return (previous + delta).coerceIn(0, 100)
}

private fun levelForScore(score: Int): HealthLevel {
    return when {
        score >= 85 -> HealthLevel.HEALTHY
        score >= 70 -> HealthLevel.DEGRADED
        score >= 55 -> HealthLevel.UNSTABLE
        else -> HealthLevel.CRITICAL
    }
}

private fun penaltyForUsage(value: Float, threshold: Float, slope: Float, cap: Float): Float {
    return ((value - threshold).coerceAtLeast(0f) * slope).coerceAtMost(cap)
}

private fun variationPenalty(cpuHistory: List<Float>, memoryHistory: List<Float>): Float {
    val cpuVariation = averageDelta(cpuHistory)
    val memoryVariation = averageDelta(memoryHistory)
    val combined = (cpuVariation + memoryVariation) / 2f
    return if (combined < 2.5f) 0f else ((combined - 2.5f) * 2.4f).coerceAtMost(14f)
}

private fun averageDelta(values: List<Float>): Float {
    if (values.size < 2) return 0f
    val deltas = values.zipWithNext { a, b -> (a - b).absoluteValue }
    return deltas.sum() / deltas.size
}

private fun buildSummary(
    metrics: DeploymentMetrics,
    status: DeploymentStatus,
    cpuPenalty: Float,
    memoryPenalty: Float,
    variationPenalty: Float
): String {
    return when (status) {
        DeploymentStatus.FAILED -> "Deployment reporting failures"
        DeploymentStatus.STOPPED -> "Deployment is currently paused"
        DeploymentStatus.DEPLOYING -> "Deploy in progress, stability settling"
        DeploymentStatus.RUNNING -> {
            when {
                cpuPenalty > 8f -> "High CPU usage detected"
                memoryPenalty > 8f -> "High memory usage detected"
                variationPenalty > 6f -> "Frequent metric fluctuations"
                metrics.replicas <= 2 -> "Low replica headroom"
                else -> "Stable over the last few minutes"
            }
        }
    }
}
