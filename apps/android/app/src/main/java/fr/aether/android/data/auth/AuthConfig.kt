package fr.aether.android.data.auth

data class AuthConfig(
    val baseUrl: String,
    val realm: String,
    val clientId: String,
    val redirectUri: String
)
