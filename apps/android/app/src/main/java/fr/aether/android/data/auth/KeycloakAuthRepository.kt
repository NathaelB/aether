package fr.aether.android.data.auth

import fr.aether.android.domain.model.AuthRequest
import fr.aether.android.domain.model.AuthToken
import fr.aether.android.domain.repository.AuthRepository
import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.forms.submitForm
import io.ktor.http.Parameters
import io.ktor.http.isSuccess
import javax.inject.Inject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

class KeycloakAuthRepository @Inject constructor(
    private val authConfig: AuthConfig,
    private val httpClient: HttpClient,
    private val authSession: AuthSession
) : AuthRepository {
    private var pendingAuth: PendingAuth? = null
    private val json = Json { ignoreUnknownKeys = true }

    override suspend fun createAuthorizationRequest(): Result<AuthRequest> {
        return withContext(Dispatchers.IO) {
            val codeVerifier = PkceUtil.generateCodeVerifier()
            val codeChallenge = PkceUtil.generateCodeChallenge(codeVerifier)
            val state = PkceUtil.generateCodeVerifier().take(32)

            val authorizationUrl = buildAuthorizationUrl(
                codeChallenge = codeChallenge,
                state = state
            )
            pendingAuth = PendingAuth(
                codeVerifier = codeVerifier,
                state = state,
                redirectUri = authConfig.redirectUri
            )
            Result.success(AuthRequest(authorizationUrl = authorizationUrl, state = state))
        }
    }

    override suspend fun exchangeToken(
        authorizationCode: String,
        state: String
    ): Result<AuthToken> {
        return withContext(Dispatchers.IO) {
            val currentAuth = pendingAuth
            if (currentAuth == null || currentAuth.state != state) {
                return@withContext Result.failure(
                    IllegalStateException("Invalid login session.")
                )
            }

            try {
                val response = httpClient.submitForm(
                    url = tokenEndpoint(),
                    formParameters = Parameters.build {
                        append("grant_type", "authorization_code")
                        append("client_id", authConfig.clientId)
                        append("code", authorizationCode)
                        append("redirect_uri", currentAuth.redirectUri)
                        append("code_verifier", currentAuth.codeVerifier)
                    }
                )
                val bodyText = response.body<String>()
                if (!response.status.isSuccess()) {
                    val errorMessage = parseTokenError(bodyText)
                    return@withContext Result.failure(
                        IllegalStateException(
                            errorMessage ?: "Token exchange failed: ${response.status}."
                        )
                    )
                }

                val token = parseToken(bodyText)
                if (token == null) {
                    val errorMessage = parseTokenError(bodyText)
                    return@withContext Result.failure(
                        IllegalStateException(
                            errorMessage ?: "Token response missing fields."
                        )
                    )
                }

                pendingAuth = null
                authSession.setToken(token)
                Result.success(token)
            } catch (exception: Exception) {
                Result.failure(exception)
            }
        }
    }

    override suspend fun loginWithPassword(
        username: String,
        password: String
    ): Result<AuthToken> {
        return withContext(Dispatchers.IO) {
            try {
                val response = httpClient.submitForm(
                    url = tokenEndpoint(),
                    formParameters = Parameters.build {
                        append("grant_type", "password")
                        append("client_id", authConfig.clientId)
                        append("username", username)
                        append("password", password)
                        append("scope", "openid profile email")
                    }
                )
                val bodyText = response.body<String>()
                if (!response.status.isSuccess()) {
                    val errorMessage = parseTokenError(bodyText)
                    return@withContext Result.failure(
                        IllegalStateException(
                            errorMessage ?: "Login failed: ${response.status}."
                        )
                    )
                }

                val token = parseToken(bodyText)
                if (token == null) {
                    val errorMessage = parseTokenError(bodyText)
                    return@withContext Result.failure(
                        IllegalStateException(
                            errorMessage ?: "Token response missing fields."
                        )
                    )
                }

                authSession.setToken(token)
                Result.success(token)
            } catch (exception: Exception) {
                Result.failure(exception)
            }
        }
    }

    override suspend fun logout(): Result<Unit> {
        return withContext(Dispatchers.IO) {
            val refreshToken = authSession.token.value?.refreshToken
            if (refreshToken.isNullOrBlank()) {
                authSession.clear()
                return@withContext Result.success(Unit)
            }

            try {
                val response = httpClient.submitForm(
                    url = logoutEndpoint(),
                    formParameters = Parameters.build {
                        append("client_id", authConfig.clientId)
                        append("refresh_token", refreshToken)
                    }
                )
                if (!response.status.isSuccess()) {
                    return@withContext Result.failure(
                        IllegalStateException("Logout failed: ${response.status}.")
                    )
                }
                Result.success(Unit)
            } catch (exception: Exception) {
                Result.failure(exception)
            } finally {
                authSession.clear()
            }
        }
    }

    private fun buildAuthorizationUrl(codeChallenge: String, state: String): String {
        val base = authEndpoint()
        val encodedRedirect = java.net.URLEncoder.encode(authConfig.redirectUri, "UTF-8")
        val encodedScope = java.net.URLEncoder.encode("openid profile email", "UTF-8")

        return buildString {
            append(base)
            append("?response_type=code")
            append("&client_id=").append(authConfig.clientId)
            append("&redirect_uri=").append(encodedRedirect)
            append("&scope=").append(encodedScope)
            append("&code_challenge_method=S256")
            append("&code_challenge=").append(codeChallenge)
            append("&state=").append(state)
        }
    }

    private fun authEndpoint(): String {
        return "${authConfig.baseUrl}/realms/${authConfig.realm}/protocol/openid-connect/auth"
    }

    private fun tokenEndpoint(): String {
        return "${authConfig.baseUrl}/realms/${authConfig.realm}/protocol/openid-connect/token"
    }

    private fun logoutEndpoint(): String {
        return "${authConfig.baseUrl}/realms/${authConfig.realm}/protocol/openid-connect/logout"
    }

    private fun parseTokenError(bodyText: String): String? {
        return runCatching {
            val jsonObject = json.parseToJsonElement(bodyText).jsonObject
            val error = jsonObject["error"]?.jsonPrimitive?.content
            val description = jsonObject["error_description"]?.jsonPrimitive?.content
            when {
                !error.isNullOrBlank() && !description.isNullOrBlank() ->
                    "$error: $description"
                !description.isNullOrBlank() -> description
                !error.isNullOrBlank() -> error
                else -> null
            }
        }.getOrNull()
    }

    private fun parseToken(bodyText: String): AuthToken? {
        return runCatching {
            val jsonObject = json.parseToJsonElement(bodyText).jsonObject
            val accessToken = jsonObject["access_token"]
                ?.jsonPrimitive
                ?.content
            val expiresIn = jsonObject["expires_in"]
                ?.jsonPrimitive
                ?.content
                ?.toLongOrNull()
            val refreshToken = jsonObject["refresh_token"]
                ?.jsonPrimitive
                ?.content
            val idToken = jsonObject["id_token"]
                ?.jsonPrimitive
                ?.content

            if (accessToken.isNullOrBlank() || expiresIn == null) {
                return@runCatching null
            }
            AuthToken(
                accessToken = accessToken,
                expiresIn = expiresIn,
                refreshToken = refreshToken,
                idToken = idToken
            )
        }.getOrNull()
    }
}
