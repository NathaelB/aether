package fr.aether.android.notifications

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.os.Build

object NotificationChannels {
    const val CpuAlertsId = "cpu_alerts"

    fun create(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val channel = NotificationChannel(
            CpuAlertsId,
            "Resource alerts",
            NotificationManager.IMPORTANCE_HIGH
        ).apply {
            description = "Alerts when deployment CPU or memory usage is high."
        }
        manager.createNotificationChannel(channel)
    }
}
