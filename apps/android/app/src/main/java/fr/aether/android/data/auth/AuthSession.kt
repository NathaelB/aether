package fr.aether.android.data.auth

import fr.aether.android.domain.model.AuthToken
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

@Singleton
class AuthSession @Inject constructor() {
    private val _token = MutableStateFlow<AuthToken?>(null)
    val token: StateFlow<AuthToken?> = _token.asStateFlow()

    fun setToken(token: AuthToken) {
        _token.value = token
    }

    fun clear() {
        _token.value = null
    }
}
