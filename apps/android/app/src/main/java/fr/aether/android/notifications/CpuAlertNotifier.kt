package fr.aether.android.notifications

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import fr.aether.android.MainActivity
import fr.aether.android.R
import java.util.Locale

object CpuAlertNotifier {
    private const val CpuNotificationId = 1103
    private const val MemoryNotificationId = 1104

    fun showHighCpu(context: Context, deploymentName: String, cpuUsage: Float) {
        showAlert(
            context = context,
            notificationId = CpuNotificationId,
            title = "CPU alert • $deploymentName",
            message = "CPU usage is at ${formatPercent(cpuUsage)}."
        )
    }

    fun showHighMemory(context: Context, deploymentName: String, memoryUsage: Float) {
        showAlert(
            context = context,
            notificationId = MemoryNotificationId,
            title = "Memory alert • $deploymentName",
            message = "Memory usage is at ${formatPercent(memoryUsage)}."
        )
    }

    private fun showAlert(
        context: Context,
        notificationId: Int,
        title: String,
        message: String
    ) {
        val intent = Intent(context, MainActivity::class.java)
        val pendingIntent = PendingIntent.getActivity(
            context,
            notificationId,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val notification = NotificationCompat.Builder(context, NotificationChannels.CpuAlertsId)
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentTitle(title)
            .setContentText(message)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_ALARM)
            .setContentIntent(pendingIntent)
            .setAutoCancel(true)
            .build()
        NotificationManagerCompat.from(context).notify(notificationId, notification)
    }

    private fun formatPercent(value: Float): String {
        return String.format(Locale.US, "%.0f%%", value)
    }
}
