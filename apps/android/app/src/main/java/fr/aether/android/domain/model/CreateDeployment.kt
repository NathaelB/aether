package fr.aether.android.domain.model

data class CreateDeploymentRequest(
    val name: String,
    val environment: Environment,
    val replicas: Int,
    val cpuLimit: CpuPreset,
    val memoryLimit: MemoryPreset,
    val autoScalingEnabled: Boolean
)

enum class Environment {
    DEV,
    STAGING,
    PROD
}

enum class CpuPreset {
    SMALL,
    MEDIUM,
    LARGE
}

enum class MemoryPreset {
    SMALL,
    MEDIUM,
    LARGE
}
