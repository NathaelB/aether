package fr.aether.android

import android.app.Application
import dagger.hilt.android.HiltAndroidApp
import fr.aether.android.notifications.NotificationChannels

@HiltAndroidApp
class AetherApp : Application() {
    override fun onCreate() {
        super.onCreate()
        NotificationChannels.create(this)
    }
}
