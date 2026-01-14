package fr.aether.android.presentation.navigation

import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.IconButton
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import androidx.navigation.navOptions
import fr.aether.android.presentation.auth.LoginScreen
import fr.aether.android.presentation.auth.LoginUiState
import fr.aether.android.presentation.auth.LoginViewModel
import fr.aether.android.presentation.account.AccountScreen
import fr.aether.android.presentation.deployments.DeploymentDetailScreen
import fr.aether.android.presentation.deployments.DeploymentsScreen
import fr.aether.android.presentation.deployments.DeploymentsUiState
import fr.aether.android.presentation.deployments.DeploymentsViewModel
import fr.aether.android.presentation.session.SessionViewModel
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ArrowBack
import androidx.compose.material.icons.outlined.Dashboard
import androidx.compose.material.icons.outlined.Person

private const val NavAnimationMillis = 320

private fun AnimatedContentTransitionScope<*>.slideInFromRight() =
    slideInHorizontally(
        initialOffsetX = { fullWidth -> fullWidth },
        animationSpec = tween(NavAnimationMillis)
    )

private fun AnimatedContentTransitionScope<*>.slideOutToLeft() =
    slideOutHorizontally(
        targetOffsetX = { fullWidth -> -fullWidth },
        animationSpec = tween(NavAnimationMillis)
    )

private fun AnimatedContentTransitionScope<*>.slideInFromLeft() =
    slideInHorizontally(
        initialOffsetX = { fullWidth -> -fullWidth },
        animationSpec = tween(NavAnimationMillis)
    )

private fun AnimatedContentTransitionScope<*>.slideOutToRight() =
    slideOutHorizontally(
        targetOffsetX = { fullWidth -> fullWidth },
        animationSpec = tween(NavAnimationMillis)
    )

private fun AnimatedContentTransitionScope<*>.fadeInFast() =
    fadeIn(animationSpec = tween(180))

private fun AnimatedContentTransitionScope<*>.fadeOutFast() =
    fadeOut(animationSpec = tween(140))

object Routes {
    const val Login = "login"
    const val Main = "main"
}

private object MainRoutes {
    const val Deployments = "deployments"
    const val Account = "account"
    const val DeploymentDetail = "deployment/{deploymentId}"

    fun deploymentDetail(deploymentId: String): String {
        return "deployment/$deploymentId"
    }
}

@Composable
fun AppNavGraph(
    navController: NavHostController = rememberNavController()
) {
    val sessionViewModel: SessionViewModel = hiltViewModel()
    val token by sessionViewModel.token.collectAsStateWithLifecycle()
    val startDestination = if (token != null) Routes.Main else Routes.Login

    LaunchedEffect(token) {
        if (token != null && navController.currentDestination?.route != Routes.Main) {
            navController.navigate(Routes.Main) {
                popUpTo(Routes.Login) { inclusive = true }
                launchSingleTop = true
            }
        }
    }

    NavHost(
        navController = navController,
        startDestination = startDestination
    ) {
        composable(
            route = Routes.Login,
            enterTransition = { fadeInFast() },
            exitTransition = { fadeOutFast() },
            popEnterTransition = { fadeInFast() },
            popExitTransition = { fadeOutFast() }
        ) {
            val viewModel: LoginViewModel = hiltViewModel()
            val uiState by viewModel.uiState.collectAsStateWithLifecycle()

            LoginScreen(
                uiState = uiState,
                onLoginClicked = viewModel::onLoginClicked,
                onAuthLaunched = viewModel::onAuthLaunched
            )

            LaunchedEffect(uiState) {
                if (uiState is LoginUiState.Success) {
                    navController.navigate(Routes.Main) {
                        popUpTo(Routes.Login) { inclusive = true }
                        launchSingleTop = true
                    }
                }
            }
        }
        composable(
            route = Routes.Main,
            enterTransition = { fadeInFast() },
            exitTransition = { fadeOutFast() },
            popEnterTransition = { fadeInFast() },
            popExitTransition = { fadeOutFast() }
        ) {
            val sessionViewModel: SessionViewModel = hiltViewModel()
            MainScaffold(
                onLogout = {
                    sessionViewModel.logout()
                    navController.navigate(Routes.Login) {
                        popUpTo(Routes.Main) { inclusive = true }
                        launchSingleTop = true
                    }
                }
            )
        }
    }
}

private data class MainNavItem(
    val route: String,
    val label: String,
    val icon: androidx.compose.ui.graphics.vector.ImageVector
)

@Composable
@OptIn(ExperimentalMaterial3Api::class)
private fun MainScaffold(
    onLogout: () -> Unit
) {
    val navController = rememberNavController()
    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentRoute = navBackStackEntry?.destination?.route ?: MainRoutes.Deployments
    val scrollBehavior = TopAppBarDefaults.enterAlwaysScrollBehavior()
    val items = listOf(
        MainNavItem(
            route = MainRoutes.Deployments,
            label = "Deployments",
            icon = Icons.Outlined.Dashboard
        ),
        MainNavItem(
            route = MainRoutes.Account,
            label = "Account",
            icon = Icons.Outlined.Person
        )
    )
    val title = when (currentRoute) {
        MainRoutes.Account -> "Account"
        MainRoutes.DeploymentDetail -> "Deployment details"
        else -> "Deployments"
    }
    val showBack = currentRoute == MainRoutes.DeploymentDetail
    val showBottomBar = currentRoute != MainRoutes.DeploymentDetail

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        containerColor = MaterialTheme.colorScheme.surface,
        topBar = {
            TopAppBar(
                title = { Text(text = title, style = MaterialTheme.typography.titleLarge) },
                navigationIcon = if (showBack) {
                    {
                        IconButton(onClick = { navController.popBackStack() }) {
                            Icon(
                                imageVector = Icons.Outlined.ArrowBack,
                                contentDescription = "Back"
                            )
                        }
                    }
                } else {
                    {}
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainer,
                    titleContentColor = MaterialTheme.colorScheme.onSurface
                ),
                scrollBehavior = scrollBehavior
            )
        },
        bottomBar = {
            if (showBottomBar) {
                NavigationBar(
                    containerColor = MaterialTheme.colorScheme.surfaceContainer,
                    tonalElevation = 2.dp
                ) {
                    items.forEach { item ->
                        val selected = currentRoute == item.route
                        NavigationBarItem(
                            selected = selected,
                            onClick = {
                                if (!selected) {
                                    navController.navigate(item.route, navOptions {
                                        launchSingleTop = true
                                        popUpTo(MainRoutes.Deployments)
                                    })
                                }
                            },
                            icon = { Icon(imageVector = item.icon, contentDescription = null) },
                            label = { Text(text = item.label) },
                            colors = NavigationBarItemDefaults.colors(
                                indicatorColor = MaterialTheme.colorScheme.secondaryContainer
                            )
                        )
                    }
                }
            }
        }
    ) { padding ->
        NavHost(
            navController = navController,
            startDestination = MainRoutes.Deployments
        ) {
            composable(MainRoutes.Deployments) {
                val viewModel: DeploymentsViewModel = hiltViewModel()
                val uiState by viewModel.uiState.collectAsStateWithLifecycle()

                DeploymentsScreen(
                    uiState = uiState,
                    onRefresh = viewModel::refresh,
                    onDeploymentClick = { deployment ->
                        navController.navigate(MainRoutes.deploymentDetail(deployment.id))
                    },
                    modifier = Modifier.padding(padding)
                )
            }
            composable(
                route = MainRoutes.DeploymentDetail,
                arguments = listOf(navArgument("deploymentId") { type = NavType.StringType }),
                enterTransition = { slideInFromRight() },
                exitTransition = { slideOutToLeft() },
                popEnterTransition = { slideInFromLeft() },
                popExitTransition = { slideOutToRight() }
            ) { backStackEntry ->
                val viewModel: DeploymentsViewModel = hiltViewModel()
                val uiState by viewModel.uiState.collectAsStateWithLifecycle()
                val deploymentId = backStackEntry.arguments?.getString("deploymentId")
                val deployment = deploymentId?.let { viewModel.deploymentById(it) }
                DeploymentDetailScreen(
                    deployment = deployment,
                    isLoading = uiState is DeploymentsUiState.Loading,
                    modifier = Modifier.padding(padding)
                )
            }
            composable(MainRoutes.Account) {
                val sessionViewModel: SessionViewModel = hiltViewModel()
                val profile by sessionViewModel.profile.collectAsStateWithLifecycle()
                AccountScreen(
                    profile = profile,
                    onLogout = onLogout,
                    modifier = Modifier.padding(padding)
                )
            }
        }
    }
}
