package fr.aether.android.domain.model

data class AuthRequest(
    val authorizationUrl: String,
    val state: String
)
