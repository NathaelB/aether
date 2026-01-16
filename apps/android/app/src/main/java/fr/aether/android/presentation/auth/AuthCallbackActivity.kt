package fr.aether.android.presentation.auth

import android.os.Bundle
import android.content.Intent
import androidx.activity.ComponentActivity
import dagger.hilt.android.AndroidEntryPoint
import fr.aether.android.MainActivity
import javax.inject.Inject

@AndroidEntryPoint
class AuthCallbackActivity : ComponentActivity() {
    @Inject lateinit var authResultBroadcaster: AuthResultBroadcaster

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val data = intent?.data
        val error = data?.getQueryParameter("error")
        val code = data?.getQueryParameter("code")
        val state = data?.getQueryParameter("state")

        val result = when {
            !error.isNullOrBlank() -> AuthResult.Error(error)
            code.isNullOrBlank() || state.isNullOrBlank() ->
                AuthResult.Error("Missing authorization response.")
            else -> AuthResult.Success(code, state)
        }

        authResultBroadcaster.emit(result)
        startActivity(
            Intent(this, MainActivity::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            }
        )
        finish()
    }
}
