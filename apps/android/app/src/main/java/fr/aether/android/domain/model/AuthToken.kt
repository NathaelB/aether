package fr.aether.android.domain.model

data class AuthToken(
    val accessToken: String,
    val expiresIn: Long,
    val refreshToken: String? = null,
    val idToken: String? = null
)
