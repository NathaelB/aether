package fr.aether.android.presentation.observability

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlin.random.Random
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class ObservabilityViewModel : ViewModel() {
    private val _uiState = MutableStateFlow<ObservabilityUiState>(ObservabilityUiState.Loading)
    val uiState: StateFlow<ObservabilityUiState> = _uiState.asStateFlow()

    private val random = Random.Default

    init {
        viewModelScope.launch {
            delay(650)
            _uiState.value = ObservabilityUiState.Data(initialMetrics())
            startTicker()
        }
    }

    fun refresh() {
        _uiState.value = ObservabilityUiState.Loading
        viewModelScope.launch {
            delay(450)
            _uiState.value = ObservabilityUiState.Data(initialMetrics())
        }
    }

    private fun startTicker() {
        viewModelScope.launch {
            while (true) {
                delay(random.nextLong(2000, 3001))
                val current = (_uiState.value as? ObservabilityUiState.Data)?.metrics ?: continue
                val cpu = nudge(current.cpuUsage, 2.8f, 0f, 100f)
                val memory = nudge(current.memoryUsage, 2.2f, 0f, 100f)
                val replicas = nudgeReplicas(current.replicas, current.maxReplicas)
                _uiState.value = ObservabilityUiState.Data(
                    metrics = current.copy(
                        cpuUsage = cpu,
                        memoryUsage = memory,
                        replicas = replicas,
                        cpuHistory = appendHistory(current.cpuHistory, cpu),
                        memoryHistory = appendHistory(current.memoryHistory, memory),
                        replicasHistory = appendHistory(current.replicasHistory, replicas)
                    )
                )
            }
        }
    }

    private fun initialMetrics(): DeploymentMetrics {
        val maxReplicas = 6
        val replicas = random.nextInt(3, maxReplicas + 1)
        val cpu = random.nextFloat() * 30f + 45f
        val memory = random.nextFloat() * 25f + 50f
        return DeploymentMetrics(
            cpuUsage = cpu,
            memoryUsage = memory,
            replicas = replicas,
            maxReplicas = maxReplicas,
            cpuHistory = buildHistory(cpu),
            memoryHistory = buildHistory(memory),
            replicasHistory = buildHistory(replicas)
        )
    }

    private fun nudge(value: Float, step: Float, min: Float, max: Float): Float {
        val delta = random.nextFloat() * step * 2f - step
        return (value + delta).coerceIn(min, max)
    }

    private fun nudgeReplicas(current: Int, maxReplicas: Int): Int {
        val roll = random.nextFloat()
        val delta = when {
            roll < 0.25f -> -1
            roll > 0.75f -> 1
            else -> 0
        }
        return (current + delta).coerceIn(1, maxReplicas)
    }

    private fun buildHistory(value: Float, size: Int = 24): List<Float> {
        return List(size) { value }
    }

    private fun buildHistory(value: Int, size: Int = 24): List<Int> {
        return List(size) { value }
    }

    private fun appendHistory(values: List<Float>, next: Float, size: Int = 24): List<Float> {
        val updated = (values + next).takeLast(size)
        return updated
    }

    private fun appendHistory(values: List<Int>, next: Int, size: Int = 24): List<Int> {
        val updated = (values + next).takeLast(size)
        return updated
    }
}
