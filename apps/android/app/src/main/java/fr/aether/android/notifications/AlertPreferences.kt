package fr.aether.android.notifications

import android.content.Context

object AlertPreferences {
    private const val PrefsName = "alert_preferences"
    private const val KeyCpuThreshold = "cpu_threshold"
    private const val KeyMemoryThreshold = "memory_threshold"
    const val DefaultThreshold = 70f

    fun cpuThreshold(context: Context): Float {
        return prefs(context).getFloat(KeyCpuThreshold, DefaultThreshold)
    }

    fun memoryThreshold(context: Context): Float {
        return prefs(context).getFloat(KeyMemoryThreshold, DefaultThreshold)
    }

    fun setCpuThreshold(context: Context, value: Float) {
        prefs(context).edit().putFloat(KeyCpuThreshold, value).apply()
    }

    fun setMemoryThreshold(context: Context, value: Float) {
        prefs(context).edit().putFloat(KeyMemoryThreshold, value).apply()
    }

    private fun prefs(context: Context) = context.getSharedPreferences(PrefsName, Context.MODE_PRIVATE)
}
