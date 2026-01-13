package fr.aether.android.presentation.components

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import fr.aether.android.ui.theme.AndroidTheme

/**
 * Exemples d'utilisation des LoadingIndicator Material 3 Expressive
 *
 * Ce fichier démontre toutes les variantes disponibles des indicateurs de chargement
 * avec les nouvelles animations expressives de Material 3.
 */

@Composable
fun LoadingIndicatorShowcase(
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(24.dp)
    ) {
        // Header
        Text(
            text = "Material 3 Expressive Loading Indicators",
            style = MaterialTheme.typography.headlineMedium,
            color = MaterialTheme.colorScheme.primary
        )

        // Section 1: Circular Indeterminate
        ShowcaseSection(
            title = "Indicateurs Circulaires Indéterminés",
            description = "Pour les opérations dont la durée est inconnue"
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceEvenly,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    ExpressiveCircularLoadingIndicator(size = 32.dp, strokeWidth = 3.dp)
                    Spacer(modifier = Modifier.height(8.dp))
                    Text("Small", style = MaterialTheme.typography.labelSmall)
                }
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    ExpressiveCircularLoadingIndicator(size = 48.dp, strokeWidth = 4.dp)
                    Spacer(modifier = Modifier.height(8.dp))
                    Text("Medium", style = MaterialTheme.typography.labelSmall)
                }
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    ExpressiveCircularLoadingIndicator(size = 64.dp, strokeWidth = 5.dp)
                    Spacer(modifier = Modifier.height(8.dp))
                    Text("Large", style = MaterialTheme.typography.labelSmall)
                }
            }
        }

        // Section 2: Linear Indeterminate
        ShowcaseSection(
            title = "Indicateur Linéaire Indéterminé",
            description = "Idéal pour les barres de chargement en haut d'écran"
        ) {
            ExpressiveLinearLoadingIndicator()
        }

        // Section 3: Circular Determinate
        ShowcaseSection(
            title = "Indicateur Circulaire Déterminé",
            description = "Affiche la progression exacte d'une tâche"
        ) {
            var progress by remember { mutableFloatStateOf(0.3f) }
            val animatedProgress by animateFloatAsState(
                targetValue = progress,
                label = "progress"
            )

            Column(
                modifier = Modifier.fillMaxWidth(),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                ExpressiveDeterminateCircularIndicator(
                    progress = animatedProgress,
                    size = 80.dp,
                    strokeWidth = 6.dp
                )
                Text(
                    text = "${(animatedProgress * 100).toInt()}%",
                    style = MaterialTheme.typography.headlineSmall
                )
                Slider(
                    value = progress,
                    onValueChange = { progress = it },
                    modifier = Modifier.fillMaxWidth()
                )
            }
        }

        // Section 4: Linear Determinate
        ShowcaseSection(
            title = "Indicateur Linéaire Déterminé",
            description = "Barre de progression pour les téléchargements, uploads, etc."
        ) {
            var progress by remember { mutableFloatStateOf(0.65f) }
            val animatedProgress by animateFloatAsState(
                targetValue = progress,
                label = "progress"
            )

            Column(
                modifier = Modifier.fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                ExpressiveDeterminateLinearIndicator(progress = animatedProgress)
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        text = "${(animatedProgress * 100).toInt()}%",
                        style = MaterialTheme.typography.bodyMedium
                    )
                    Text(
                        text = "250 MB / 384 MB",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                Slider(
                    value = progress,
                    onValueChange = { progress = it },
                    modifier = Modifier.fillMaxWidth()
                )
            }
        }

        // Section 5: Loading State Complete
        ShowcaseSection(
            title = "État de Chargement Complet",
            description = "Composant prêt à l'emploi avec texte et indicateur"
        ) {
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainer
                )
            ) {
                ExpressiveLoadingState(
                    text = "Chargement des données...",
                    modifier = Modifier.fillMaxWidth()
                )
            }
        }

        // Section 6: Real World Example
        ShowcaseSection(
            title = "Exemple d'Utilisation Réelle",
            description = "Simulation d'un écran de chargement d'application"
        ) {
            RealWorldLoadingExample()
        }

        // Section 7: SpinningProgressIndicator Updated
        ShowcaseSection(
            title = "SpinningProgressIndicator Mis à Jour",
            description = "Ancien composant maintenant avec support Material 3 Expressive"
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceEvenly,
                verticalAlignment = Alignment.CenterVertically
            ) {
                SpinningProgressIndicator(indicatorSize = 40.dp)
                SpinningProgressIndicator(indicatorSize = 56.dp)
            }
        }
    }
}

@Composable
private fun ShowcaseSection(
    title: String,
    description: String,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit
) {
    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onSurface
        )
        Text(
            text = description,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        HorizontalDivider()
        content()
    }
}

@Composable
private fun RealWorldLoadingExample() {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surface
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = 2.dp)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text(
                text = "Connexion au serveur",
                style = MaterialTheme.typography.titleMedium
            )

            ExpressiveCircularLoadingIndicator(
                size = 56.dp,
                strokeWidth = 5.dp
            )

            Text(
                text = "Authentification en cours...",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            Spacer(modifier = Modifier.height(8.dp))

            ExpressiveLinearLoadingIndicator(
                modifier = Modifier.fillMaxWidth()
            )

            Spacer(modifier = Modifier.height(8.dp))

            Button(
                onClick = { /* Cancel action */ },
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("Annuler")
            }
        }
    }
}

@Preview(name = "Loading Indicators Showcase - Light", showBackground = true)
@Composable
private fun LoadingIndicatorShowcasePreviewLight() {
    AndroidTheme(darkTheme = false) {
        Surface {
            LoadingIndicatorShowcase()
        }
    }
}

@Preview(name = "Loading Indicators Showcase - Dark", showBackground = true)
@Composable
private fun LoadingIndicatorShowcasePreviewDark() {
    AndroidTheme(darkTheme = true) {
        Surface {
            LoadingIndicatorShowcase()
        }
    }
}

/**
 * Exemples d'utilisation rapide
 *
 * // Indicateur circulaire simple
 * ExpressiveCircularLoadingIndicator()
 *
 * // Indicateur linéaire en haut d'écran
 * ExpressiveLinearLoadingIndicator(modifier = Modifier.fillMaxWidth())
 *
 * // Indicateur avec progression
 * ExpressiveDeterminateCircularIndicator(progress = 0.75f)
 *
 * // État de chargement complet
 * ExpressiveLoadingState(text = "Chargement...")
 *
 * // Indicateur personnalisé
 * ExpressiveCircularLoadingIndicator(
 *     size = 64.dp,
 *     strokeWidth = 6.dp,
 *     color = MaterialTheme.colorScheme.secondary,
 *     trackColor = MaterialTheme.colorScheme.surfaceVariant
 * )
 */
