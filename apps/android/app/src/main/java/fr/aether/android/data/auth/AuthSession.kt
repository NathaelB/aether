package fr.aether.android.data.auth

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import fr.aether.android.domain.model.AuthToken
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

@Singleton
class AuthSession @Inject constructor(
    @ApplicationContext context: Context
) {
    private val preferences = context.getSharedPreferences(PrefsName, Context.MODE_PRIVATE)
    private val _token = MutableStateFlow<AuthToken?>(loadToken())
    val token: StateFlow<AuthToken?> = _token.asStateFlow()

    fun setToken(token: AuthToken) {
        _token.value = token
        saveToken(token)
    }

    fun clear() {
        _token.value = null
        preferences.edit().clear().apply()
    }

    private fun loadToken(): AuthToken? {
        val accessToken = preferences.getString(KeyAccessToken, null)
        val expiresIn = preferences.getLong(KeyExpiresIn, 0L)
        if (accessToken.isNullOrBlank() || expiresIn == 0L) {
            return null
        }
        return AuthToken(
            accessToken = accessToken,
            expiresIn = expiresIn,
            refreshToken = preferences.getString(KeyRefreshToken, null),
            idToken = preferences.getString(KeyIdToken, null)
        )
    }

    private fun saveToken(token: AuthToken) {
        preferences.edit()
            .putString(KeyAccessToken, token.accessToken)
            .putLong(KeyExpiresIn, token.expiresIn)
            .putString(KeyRefreshToken, token.refreshToken)
            .putString(KeyIdToken, token.idToken)
            .apply()
    }

    private companion object {
        private const val PrefsName = "auth_session"
        private const val KeyAccessToken = "access_token"
        private const val KeyRefreshToken = "refresh_token"
        private const val KeyIdToken = "id_token"
        private const val KeyExpiresIn = "expires_in"
    }
}
