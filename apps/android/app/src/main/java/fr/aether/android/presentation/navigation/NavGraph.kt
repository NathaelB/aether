package fr.aether.android.presentation.navigation

import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.ExperimentalMaterial3Api
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
import fr.aether.android.presentation.home.HomeScreen
import fr.aether.android.presentation.home.HomeSummary
import fr.aether.android.domain.model.DeploymentStatus
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Dashboard
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.Person
import kotlinx.coroutines.delay

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
    const val Home = "home"
    const val Deployments = "deployments"
    const val Account = "account"
    const val DeploymentDetail = "deployment/{deploymentId}"
    const val CreateDeployment = "deployment/create"

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
                onPasswordLogin = viewModel::onPasswordLogin
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
    val items = listOf(
        MainNavItem(
            route = MainRoutes.Home,
            label = "Home",
            icon = Icons.Outlined.Home
        ),
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
    val showBottomBar = currentRoute != MainRoutes.DeploymentDetail &&
        currentRoute != MainRoutes.CreateDeployment

    Scaffold(
        containerColor = MaterialTheme.colorScheme.surface,
        topBar = {},
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
            startDestination = MainRoutes.Home
        ) {
            composable(
                route = MainRoutes.Home,
                enterTransition = { fadeInFast() },
                exitTransition = { fadeOutFast() },
                popEnterTransition = { fadeInFast() },
                popExitTransition = { fadeOutFast() }
            ) {
                val sessionViewModel: SessionViewModel = hiltViewModel()
                val profile by sessionViewModel.profile.collectAsStateWithLifecycle()
                val deploymentsViewModel: DeploymentsViewModel = hiltViewModel()
                val deploymentsState by deploymentsViewModel.uiState.collectAsStateWithLifecycle()
                val deployments = (deploymentsState as? DeploymentsUiState.Success)
                    ?.deployments
                    .orEmpty()
                val environments = deployments.map { it.environment }.distinct().size
                val regions = deployments.map { it.region }.distinct().size
                val running = deployments.count { it.status == DeploymentStatus.RUNNING }
                val attention = deployments.count {
                    it.status == DeploymentStatus.FAILED ||
                        it.status == DeploymentStatus.STOPPED
                }
                val lastUpdated = deployments.firstOrNull()?.updatedAt
                HomeScreen(
                    summary = HomeSummary(
                        displayName = profile?.displayName ?: "Operator",
                        totalDeployments = deployments.size,
                        runningDeployments = running,
                        attentionDeployments = attention,
                        environments = environments,
                        regions = regions,
                        lastUpdated = lastUpdated
                    ),
                    modifier = Modifier.padding(padding)
                )
            }
            composable(
                route = MainRoutes.Deployments,
                enterTransition = { fadeInFast() },
                exitTransition = { fadeOutFast() },
                popEnterTransition = { fadeInFast() },
                popExitTransition = { fadeOutFast() }
            ) {
                val viewModel: DeploymentsViewModel = hiltViewModel()
                val uiState by viewModel.uiState.collectAsStateWithLifecycle()

                DeploymentsScreen(
                    uiState = uiState,
                    onRefresh = viewModel::refresh,
                    onDeploymentClick = { deployment ->
                        navController.navigate(MainRoutes.deploymentDetail(deployment.id))
                    },
                    onCreateDeployment = {
                        navController.navigate(MainRoutes.CreateDeployment)
                    },
                    modifier = Modifier.padding(padding)
                )
            }
            composable(
                route = MainRoutes.CreateDeployment,
                enterTransition = { slideInFromRight() },
                exitTransition = { slideOutToLeft() },
                popEnterTransition = { slideInFromLeft() },
                popExitTransition = { slideOutToRight() }
            ) {
                val createViewModel: fr.aether.android.presentation.create.CreateDeploymentViewModel =
                    hiltViewModel()
                val createState by createViewModel.uiState.collectAsStateWithLifecycle()
                val deploymentsViewModel: DeploymentsViewModel = hiltViewModel()
                LaunchedEffect(createState.step, createState.createdDeployment?.id) {
                    val created = createState.createdDeployment
                    if (createState.step == fr.aether.android.presentation.create.CreateDeploymentStep.SUCCESS &&
                        created != null
                    ) {
                        delay(600)
                        deploymentsViewModel.addDeployment(created)
                        createViewModel.markHandled()
                        navController.navigate(MainRoutes.deploymentDetail(created.id)) {
                            popUpTo(MainRoutes.CreateDeployment) { inclusive = true }
                        }
                    }
                }
                fr.aether.android.presentation.create.CreateDeploymentScreen(
                    uiState = createState,
                    onBack = { navController.popBackStack() },
                    onNameChange = createViewModel::updateName,
                    onEnvironmentChange = createViewModel::updateEnvironment,
                    onReplicasChange = createViewModel::updateReplicas,
                    onCpuPresetChange = createViewModel::updateCpuPreset,
                    onMemoryPresetChange = createViewModel::updateMemoryPreset,
                    onAutoScalingChange = createViewModel::toggleAutoScaling,
                    onReview = createViewModel::goToReview,
                    onEdit = createViewModel::backToEdit,
                    onCreate = createViewModel::startCreate,
                    onRetry = createViewModel::retryCreate,
                    onDone = {
                        val created = createState.createdDeployment
                        if (created != null) {
                            deploymentsViewModel.addDeployment(created)
                            createViewModel.markHandled()
                            navController.navigate(MainRoutes.deploymentDetail(created.id)) {
                                popUpTo(MainRoutes.CreateDeployment) { inclusive = true }
                            }
                        } else {
                            navController.popBackStack()
                        }
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
                    onBack = { navController.popBackStack() },
                    onDelete = { id ->
                        viewModel.deleteDeployment(id)
                        navController.popBackStack()
                    },
                    modifier = Modifier.padding(padding)
                )
            }
            composable(
                route = MainRoutes.Account,
                enterTransition = { fadeInFast() },
                exitTransition = { fadeOutFast() },
                popEnterTransition = { fadeInFast() },
                popExitTransition = { fadeOutFast() }
            ) {
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
