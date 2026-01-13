package fr.aether.android.presentation.auth

import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow

@Singleton
class AuthResultBroadcaster @Inject constructor() {
    private val _results = MutableSharedFlow<AuthResult>(extraBufferCapacity = 1)
    val results: SharedFlow<AuthResult> = _results.asSharedFlow()

    fun emit(result: AuthResult) {
        _results.tryEmit(result)
    }
}
