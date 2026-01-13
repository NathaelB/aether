# Guide de Migration - LoadingIndicator Material 3 Expressive

Ce guide vous aide à migrer vers les nouveaux LoadingIndicator expressifs de Material 3.

## 📋 Table des Matières

1. [Nouveautés](#nouveautés)
2. [Migration Rapide](#migration-rapide)
3. [Composants Disponibles](#composants-disponibles)
4. [Exemples d'Utilisation](#exemples-dutilisation)
5. [Bonnes Pratiques](#bonnes-pratiques)

## 🎨 Nouveautés

Les nouveaux LoadingIndicator Material 3 Expressive apportent :

- **Animations plus fluides** avec des mouvements expressifs
- **Support de la track color** pour un meilleur contraste
- **Variantes déterminées et indéterminées** pour tous les types d'indicateurs
- **API cohérente** avec paramètres personnalisables
- **Meilleure accessibilité** avec des couleurs adaptées au thème

## 🚀 Migration Rapide

### Avant (Ancien CircularProgressIndicator)

```kotlin
CircularProgressIndicator(
    modifier = Modifier.size(48.dp),
    color = MaterialTheme.colorScheme.primary,
    strokeWidth = 4.dp
)
```

### Après (Nouveau ExpressiveCircularLoadingIndicator)

```kotlin
ExpressiveCircularLoadingIndicator(
    size = 48.dp,
    strokeWidth = 4.dp,
    color = MaterialTheme.colorScheme.primary,
    trackColor = MaterialTheme.colorScheme.surfaceVariant
)
```

### SpinningProgressIndicator (Déjà Mis à Jour)

Le composant `SpinningProgressIndicator` a été mis à jour automatiquement et supporte maintenant les nouvelles fonctionnalités Material 3 :

```kotlin
// Utilisation simple (aucun changement requis)
SpinningProgressIndicator()

// Avec personnalisation (nouveaux paramètres disponibles)
SpinningProgressIndicator(
    indicatorSize = 56.dp,
    strokeWidth = 5.dp,
    color = MaterialTheme.colorScheme.secondary,
    trackColor = MaterialTheme.colorScheme.surfaceVariant
)
```

## 📦 Composants Disponibles

### 1. ExpressiveCircularLoadingIndicator

Indicateur circulaire indéterminé (animation continue).

```kotlin
ExpressiveCircularLoadingIndicator(
    size = 48.dp,              // Taille de l'indicateur
    strokeWidth = 4.dp,        // Épaisseur du trait
    color = MaterialTheme.colorScheme.primary,
    trackColor = MaterialTheme.colorScheme.surfaceVariant
)
```

**Quand l'utiliser :** Chargement de données, opérations dont la durée est inconnue.

### 2. ExpressiveLinearLoadingIndicator

Indicateur linéaire indéterminé (barre de progression).

```kotlin
ExpressiveLinearLoadingIndicator(
    modifier = Modifier.fillMaxWidth(),
    color = MaterialTheme.colorScheme.primary,
    trackColor = MaterialTheme.colorScheme.surfaceVariant
)
```

**Quand l'utiliser :** Chargement de page, requêtes réseau, opérations en arrière-plan.

### 3. ExpressiveDeterminateCircularIndicator

Indicateur circulaire avec progression (0.0 à 1.0).

```kotlin
var progress by remember { mutableStateOf(0.5f) }

ExpressiveDeterminateCircularIndicator(
    progress = progress,
    size = 64.dp,
    strokeWidth = 5.dp
)
```

**Quand l'utiliser :** Upload/download de fichiers, progression de tâches, timers.

### 4. ExpressiveDeterminateLinearIndicator

Indicateur linéaire avec progression (0.0 à 1.0).

```kotlin
var progress by remember { mutableStateOf(0.75f) }

ExpressiveDeterminateLinearIndicator(
    progress = progress,
    modifier = Modifier.fillMaxWidth()
)
```

**Quand l'utiliser :** Progression de téléchargement, étapes d'un processus.

### 5. ExpressiveLoadingState

Composant complet avec indicateur + texte.

```kotlin
ExpressiveLoadingState(
    text = "Chargement des déploiements...",
    indicatorSize = 48.dp
)
```

**Quand l'utiliser :** États de chargement plein écran, écrans initiaux.

## 💡 Exemples d'Utilisation

### Exemple 1 : Écran de Chargement Simple

```kotlin
@Composable
fun LoadingScreen() {
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center
    ) {
        ExpressiveLoadingState(
            text = "Chargement en cours...",
            indicatorSize = 56.dp
        )
    }
}
```

### Exemple 2 : Liste avec Pull-to-Refresh

```kotlin
@Composable
fun DeploymentsScreen(uiState: DeploymentsUiState) {
    Box(modifier = Modifier.fillMaxSize()) {
        when (uiState) {
            is DeploymentsUiState.Loading -> {
                ExpressiveLoadingState(
                    text = "Chargement des déploiements...",
                    modifier = Modifier.align(Alignment.Center)
                )
            }
            is DeploymentsUiState.Success -> {
                // Votre liste ici
            }
        }

        // Indicateur de rafraîchissement en haut
        if (uiState.isRefreshing) {
            ExpressiveLinearLoadingIndicator(
                modifier = Modifier
                    .fillMaxWidth()
                    .align(Alignment.TopCenter)
            )
        }
    }
}
```

### Exemple 3 : Progression de Téléchargement

```kotlin
@Composable
fun DownloadCard(downloadProgress: Float) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column {
                    Text("Téléchargement en cours",
                         style = MaterialTheme.typography.titleMedium)
                    Text("document.pdf",
                         style = MaterialTheme.typography.bodySmall)
                }
                ExpressiveDeterminateCircularIndicator(
                    progress = downloadProgress,
                    size = 48.dp
                )
            }

            ExpressiveDeterminateLinearIndicator(
                progress = downloadProgress,
                modifier = Modifier.fillMaxWidth()
            )

            Text(
                text = "${(downloadProgress * 100).toInt()}% - ${downloadProgress * 500}MB / 500MB",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
```

### Exemple 4 : Bouton avec Chargement

```kotlin
@Composable
fun LoadingButton(
    text: String,
    isLoading: Boolean,
    onClick: () -> Unit
) {
    Button(
        onClick = onClick,
        enabled = !isLoading,
        modifier = Modifier.fillMaxWidth()
    ) {
        if (isLoading) {
            ExpressiveCircularLoadingIndicator(
                size = 20.dp,
                strokeWidth = 2.dp,
                color = MaterialTheme.colorScheme.onPrimary,
                trackColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.3f)
            )
            Spacer(modifier = Modifier.width(8.dp))
        }
        Text(if (isLoading) "Chargement..." else text)
    }
}
```

### Exemple 5 : Indicateur Inline dans une Card

```kotlin
@Composable
fun SyncStatusCard(isSyncing: Boolean) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainer
        )
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column {
                Text("Synchronisation",
                     style = MaterialTheme.typography.titleMedium)
                Text(
                    if (isSyncing) "En cours..." else "Terminée",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            if (isSyncing) {
                ExpressiveCircularLoadingIndicator(
                    size = 32.dp,
                    strokeWidth = 3.dp
                )
            } else {
                Icon(
                    Icons.Default.CheckCircle,
                    contentDescription = "Synchronisé",
                    tint = MaterialTheme.colorScheme.primary
                )
            }
        }
    }
}
```

## ✨ Bonnes Pratiques

### 1. Choisir le Bon Indicateur

| Situation | Indicateur Recommandé | Raison |
|-----------|---------------------|---------|
| Chargement initial d'écran | `ExpressiveLoadingState` | Composant complet avec texte |
| Requête réseau courte | `ExpressiveCircularLoadingIndicator` | Compact et visible |
| Chargement de page web | `ExpressiveLinearLoadingIndicator` | Moins intrusif en haut |
| Upload/Download | `ExpressiveDeterminateLinearIndicator` | Montre la progression exacte |
| Timer ou countdown | `ExpressiveDeterminateCircularIndicator` | Représentation visuelle du temps |

### 2. Tailles Recommandées

```kotlin
// Small - Pour les boutons, cards compactes
ExpressiveCircularLoadingIndicator(size = 24.dp, strokeWidth = 2.dp)

// Medium - Usage standard
ExpressiveCircularLoadingIndicator(size = 48.dp, strokeWidth = 4.dp)

// Large - Écrans de chargement principaux
ExpressiveCircularLoadingIndicator(size = 64.dp, strokeWidth = 5.dp)
```

### 3. Couleurs et Thèmes

```kotlin
// Standard (recommandé)
ExpressiveCircularLoadingIndicator(
    color = MaterialTheme.colorScheme.primary,
    trackColor = MaterialTheme.colorScheme.surfaceVariant
)

// Sur fond coloré
ExpressiveCircularLoadingIndicator(
    color = MaterialTheme.colorScheme.onPrimary,
    trackColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.3f)
)

// Variante secondaire
ExpressiveCircularLoadingIndicator(
    color = MaterialTheme.colorScheme.secondary,
    trackColor = MaterialTheme.colorScheme.secondaryContainer
)
```

### 4. Accessibilité

- Toujours fournir un contexte textuel pour les lecteurs d'écran
- Utiliser des contrastes suffisants entre l'indicateur et la track
- Ne pas utiliser uniquement la couleur pour communiquer l'état

```kotlin
@Composable
fun AccessibleLoadingState() {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        ExpressiveCircularLoadingIndicator()
        // Toujours accompagner d'un texte descriptif
        Text(
            text = "Chargement des données en cours",
            style = MaterialTheme.typography.bodyMedium
        )
    }
}
```

### 5. Animations et Transitions

Pour une progression animée :

```kotlin
val animatedProgress by animateFloatAsState(
    targetValue = progress,
    animationSpec = tween(durationMillis = 300),
    label = "progress"
)

ExpressiveDeterminateCircularIndicator(
    progress = animatedProgress
)
```

## 🔍 Voir Plus

- Fichier d'exemples complets : `LoadingIndicatorExamples.kt`
- Composant mis à jour : `SpinningProgressIndicator.kt`
- Nouveaux composants : `ExpressiveLoadingIndicator.kt`

## 📝 Notes de Version

**Version 1.0** - Janvier 2026
- Ajout des LoadingIndicator Material 3 Expressive
- Mise à jour de SpinningProgressIndicator
- Création de composants déterminés et indéterminés
- Exemples complets et guide de migration
