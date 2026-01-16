package fr.aether.android.data.auth

import java.security.MessageDigest
import java.security.SecureRandom
import java.util.Base64

object PkceUtil {
    private const val VerifierLength = 64
    private val AllowedChars =
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~".toCharArray()
    private val random = SecureRandom()

    fun generateCodeVerifier(): String {
        val buffer = CharArray(VerifierLength)
        for (i in buffer.indices) {
            buffer[i] = AllowedChars[random.nextInt(AllowedChars.size)]
        }
        return String(buffer)
    }

    fun generateCodeChallenge(codeVerifier: String): String {
        val digest = MessageDigest.getInstance("SHA-256")
        val hashed = digest.digest(codeVerifier.toByteArray(Charsets.US_ASCII))
        return Base64.getUrlEncoder().withoutPadding().encodeToString(hashed)
    }
}
