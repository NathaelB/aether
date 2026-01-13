package fr.aether.android.data.auth

data class PendingAuth(
    val codeVerifier: String,
    val state: String,
    val redirectUri: String
)
