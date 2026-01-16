package fr.aether.android.data.auth

import java.util.Base64
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject

object JwtUtils {
    private val json = Json { ignoreUnknownKeys = true }

    fun decodePayload(token: String): JsonObject? {
        val parts = token.split(".")
        if (parts.size < 2) {
            return null
        }
        return runCatching<JsonObject> {
            val decoded = Base64.getUrlDecoder().decode(padBase64(parts[1]))
            val text = String(decoded, Charsets.UTF_8)
            json.parseToJsonElement(text).jsonObject
        }.getOrNull()
    }

    private fun padBase64(value: String): String {
        val remainder = value.length % 4
        return if (remainder == 0) {
            value
        } else {
            value + "=".repeat(4 - remainder)
        }
    }
}
