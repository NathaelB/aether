package fr.aether.android.presentation.session

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import fr.aether.android.data.auth.AuthSession
import fr.aether.android.data.auth.JwtUtils
import fr.aether.android.domain.model.AuthToken
import fr.aether.android.domain.usecase.LogoutUseCase
import fr.aether.android.presentation.account.UserProfile
import javax.inject.Inject
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.launch
import kotlinx.serialization.json.jsonPrimitive

@HiltViewModel
class SessionViewModel @Inject constructor(
    private val authSession: AuthSession,
    private val logoutUseCase: LogoutUseCase
) : ViewModel() {
    val token: StateFlow<AuthToken?> = authSession.token
    val profile: StateFlow<UserProfile?> = authSession.token
        .map { token -> token?.idToken?.let { buildProfile(it) } }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    fun logout() {
        viewModelScope.launch {
            logoutUseCase()
        }
    }

    private fun buildProfile(idToken: String): UserProfile? {
        val payload = JwtUtils.decodePayload(idToken) ?: return null
        val displayName = payload["name"]?.jsonPrimitive?.content
            ?: payload["preferred_username"]?.jsonPrimitive?.content
            ?: payload["email"]?.jsonPrimitive?.content
            ?: "Signed in with Keycloak"
        val email = payload["email"]?.jsonPrimitive?.content
        val username = payload["preferred_username"]?.jsonPrimitive?.content
        return UserProfile(
            displayName = displayName,
            email = email,
            username = username
        )
    }
}
