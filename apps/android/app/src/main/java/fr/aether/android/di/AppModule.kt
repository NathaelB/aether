package fr.aether.android.di

import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import fr.aether.android.data.auth.AuthConfig
import fr.aether.android.data.auth.AuthSession
import fr.aether.android.data.auth.KeycloakAuthRepository
import fr.aether.android.data.deployment.MockDeploymentRepository
import fr.aether.android.domain.repository.AuthRepository
import fr.aether.android.domain.repository.DeploymentRepository
import fr.aether.android.domain.usecase.CompleteLoginUseCase
import fr.aether.android.domain.usecase.GetDeploymentsUseCase
import fr.aether.android.domain.usecase.LoginUseCase
import fr.aether.android.domain.usecase.LogoutUseCase
import io.ktor.client.HttpClient
import io.ktor.client.engine.okhttp.OkHttp
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.serialization.kotlinx.json.json
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object AppModule {
    private const val KeycloakBaseUrl = "http://10.0.2.2:8888"
    private const val KeycloakRealm = "aether"
    private const val KeycloakClientId = "mobile"
    private const val KeycloakRedirectUri = "aether://auth/callback"

    @Provides
    @Singleton
    fun provideAuthRepository(
        authConfig: AuthConfig,
        httpClient: HttpClient,
        authSession: AuthSession
    ): AuthRepository {
        return KeycloakAuthRepository(authConfig, httpClient, authSession)
    }

    @Provides
    @Singleton
    fun provideAuthConfig(): AuthConfig {
        return AuthConfig(
            baseUrl = KeycloakBaseUrl,
            realm = KeycloakRealm,
            clientId = KeycloakClientId,
            redirectUri = KeycloakRedirectUri
        )
    }

    @Provides
    @Singleton
    fun provideHttpClient(): HttpClient {
        return HttpClient(OkHttp) {
            install(ContentNegotiation) {
                json()
            }
        }
    }

    @Provides
    @Singleton
    fun provideDeploymentRepository(): DeploymentRepository = MockDeploymentRepository()

    @Provides
    fun provideLoginUseCase(repository: AuthRepository): LoginUseCase {
        return LoginUseCase(repository)
    }

    @Provides
    fun provideCompleteLoginUseCase(repository: AuthRepository): CompleteLoginUseCase {
        return CompleteLoginUseCase(repository)
    }

    @Provides
    fun provideLogoutUseCase(repository: AuthRepository): LogoutUseCase {
        return LogoutUseCase(repository)
    }

    @Provides
    fun provideGetDeploymentsUseCase(
        repository: DeploymentRepository
    ): GetDeploymentsUseCase {
        return GetDeploymentsUseCase(repository)
    }
}
