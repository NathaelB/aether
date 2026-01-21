package fr.aether.android.presentation.deployments

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.Icon
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.compose.runtime.DisposableEffect
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Add
import kotlinx.coroutines.delay

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DeploymentsScreen(
    uiState: DeploymentsUiState,
    onRefresh: () -> Unit,
    onDeploymentClick: (DeploymentUiModel) -> Unit,
    onCreateDeployment: () -> Unit,
    modifier: Modifier = Modifier
) {
    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                onRefresh()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }
    val isRefreshing = (uiState as? DeploymentsUiState.Success)?.isRefreshing == true
    val pullToRefreshState = rememberPullToRefreshState()

    Box(modifier = modifier.fillMaxSize()) {
        PullToRefreshBox(
            isRefreshing = isRefreshing,
            onRefresh = onRefresh,
            state = pullToRefreshState,
            modifier = Modifier.fillMaxSize()
        ) {
            AnimatedContent(
                targetState = uiState,
                transitionSpec = {
                    fadeIn(tween(220)) + expandVertically(tween(220)) togetherWith
                        fadeOut(tween(150))
                },
                label = "deployments_state"
            ) { state ->
                when (state) {
                    DeploymentsUiState.Loading -> LoadingState()
                    is DeploymentsUiState.Error -> ErrorState(
                        message = state.message,
                        onRetry = onRefresh
                    )
                    is DeploymentsUiState.Success -> Crossfade(
                        targetState = state.deployments.isEmpty(),
                        label = "deployments_empty"
                    ) { isEmpty ->
                        if (isEmpty) {
                            EmptyState(onRetry = onRefresh)
                        } else {
                            DeploymentsList(
                                deployments = state.deployments,
                                onDeploymentClick = onDeploymentClick
                            )
                        }
                    }
                }
            }
        }
        FloatingActionButton(
            onClick = onCreateDeployment,
            containerColor = MaterialTheme.colorScheme.primaryContainer,
            contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
            modifier = Modifier
                .align(Alignment.BottomEnd)
                .padding(20.dp)
        ) {
            Icon(imageVector = Icons.Outlined.Add, contentDescription = "Create deployment")
        }
    }
}

@Composable
private fun LoadingState() {
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        items(5) { index ->
            DeploymentSkeletonCard(
                modifier = Modifier.fillMaxWidth(),
                delayMillis = index * 120
            )
        }
    }
}

@Composable
private fun EmptyState(onRetry: () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(
            text = "No deployments yet",
            style = MaterialTheme.typography.titleLarge
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Once environments are connected, they will appear here.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(20.dp))
        Button(onClick = onRetry) {
            Text(text = "Refresh")
        }
    }
}

@Composable
private fun ErrorState(
    message: String,
    onRetry: () -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(
            text = "Something went wrong",
            style = MaterialTheme.typography.titleLarge
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = message,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(20.dp))
        Button(onClick = onRetry) {
            Text(text = "Try again")
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun DeploymentsList(
    deployments: List<DeploymentUiModel>,
    onDeploymentClick: (DeploymentUiModel) -> Unit
) {
    BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
        val contentPadding = PaddingValues(start = 20.dp, top = 16.dp, end = 20.dp, bottom = 36.dp)
        val arrangement = Arrangement.spacedBy(16.dp)
        val useGrid = maxWidth >= 600.dp

        if (useGrid) {
            LazyVerticalGrid(
                modifier = Modifier.fillMaxSize(),
                columns = GridCells.Adaptive(320.dp),
                contentPadding = contentPadding,
                horizontalArrangement = arrangement,
                verticalArrangement = arrangement
            ) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    DeploymentsHeader()
                }
                itemsIndexed(deployments, key = { _, item -> item.id }) { index, deployment ->
                    AnimatedDeploymentItem(
                        index = index,
                        deployment = deployment,
                        onClick = { onDeploymentClick(deployment) }
                    )
                }
            }
        } else {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = contentPadding,
                verticalArrangement = arrangement
            ) {
                item {
                    DeploymentsHeader()
                }
                itemsIndexed(deployments, key = { _, item -> item.id }) { index, deployment ->
                    AnimatedDeploymentItem(
                        index = index,
                        deployment = deployment,
                        onClick = { onDeploymentClick(deployment) }
                    )
                }
            }
        }
    }
}

@Composable
private fun DeploymentsHeader() {
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        Text(
            text = "Deployments",
            style = MaterialTheme.typography.displaySmall
        )
        Text(
            text = "Live status across your environments.",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(4.dp))
    }
}

@Composable
private fun AnimatedDeploymentItem(
    index: Int,
    deployment: DeploymentUiModel,
    onClick: () -> Unit
) {
    var isVisible by remember(deployment.id) { mutableStateOf(false) }

    LaunchedEffect(deployment.id) {
        delay(index * 60L)
        isVisible = true
    }

    AnimatedVisibility(
        visible = isVisible,
        enter = fadeIn(tween(220)) + slideInVertically(
            animationSpec = tween(220),
            initialOffsetY = { it / 6 }
        ),
        exit = fadeOut(tween(120))
    ) {
        DeploymentCard(
            deployment = deployment,
            onClick = onClick,
            modifier = Modifier.fillMaxWidth()
        )
    }
}

@Composable
private fun DeploymentSkeletonCard(
    modifier: Modifier = Modifier,
    delayMillis: Int
) {
    val transition = rememberInfiniteTransition(label = "skeleton")
    val shimmerAlpha by transition.animateFloat(
        initialValue = 0.6f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(900, delayMillis = delayMillis),
            repeatMode = RepeatMode.Reverse
        ),
        label = "skeleton_alpha"
    )
    val surface = MaterialTheme.colorScheme.surfaceContainerHigh
    val placeholder = MaterialTheme.colorScheme.surfaceContainerHighest.copy(alpha = shimmerAlpha)

    Card(
        modifier = modifier,
        colors = CardDefaults.cardColors(containerColor = surface),
        shape = MaterialTheme.shapes.large,
        elevation = CardDefaults.cardElevation(defaultElevation = 2.dp)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Box(
                    modifier = Modifier
                        .clip(MaterialTheme.shapes.small)
                        .background(placeholder)
                        .height(32.dp)
                        .fillMaxWidth(0.5f)
                )
                Spacer(modifier = Modifier.weight(1f))
                Box(
                    modifier = Modifier
                        .clip(MaterialTheme.shapes.extraLarge)
                        .background(placeholder)
                        .height(24.dp)
                        .fillMaxWidth(0.25f)
                )
            }
            Spacer(modifier = Modifier.height(12.dp))
            Box(
                modifier = Modifier
                    .clip(MaterialTheme.shapes.small)
                    .background(placeholder)
                    .height(16.dp)
                    .fillMaxWidth(0.7f)
            )
            Spacer(modifier = Modifier.height(10.dp))
            Box(
                modifier = Modifier
                    .clip(MaterialTheme.shapes.small)
                    .background(placeholder)
                    .height(14.dp)
                    .fillMaxWidth(0.4f)
            )
        }
    }
}
