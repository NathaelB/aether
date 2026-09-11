import { createRootRoute, createRoute, createRouter } from '@tanstack/react-router'
import DeploymentsOverviewFeature from './domain/deployments/pages/feature/page-deployments-overview-feature'
import PageCreateDeploymentFeature from './domain/deployments/pages/feature/page-create-deployment-feature'
import PageDeploymentDetailFeature from './domain/deployments/pages/feature/page-deployment-detail-feature'
import PageDashboardFeature from './domain/dashboard/pages/feature/page-dashboard-feature'
import { AppLayout } from './components/layout/main-layout'
import { AppShell } from './components/layout/app-shell'
import { OnboardingLayout } from './components/layout/onboarding-layout'
import PageCreateOrganisationFeature from './domain/organisations/pages/feature/page-create-organisation-feature'
import PageDataPlanesFeature from './domain/dataplanes/pages/feature/page-dataplanes-feature'
import PageDataPlaneDetailFeature from './domain/dataplanes/pages/feature/page-dataplane-detail-feature'
import PageReleasesFeature from './domain/releases/pages/feature/page-releases-feature'
import PageUpgradesFeature from './domain/upgrades/pages/feature/page-upgrades-feature'
import PageUsageFeature from './domain/usage/pages/feature/page-usage-feature'

// Root Route
const rootRoute = createRootRoute({
  component: AppShell,
})

// App Layout Route (with sidebar + breadcrumb)
const appLayoutRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/organisations/$organisationId',
  component: AppLayout,
})

// Onboarding Layout Route (no sidebar/breadcrumb)
const onboardingLayoutRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/organisations',
  component: OnboardingLayout,
})

// Deployments Route
const deploymentsRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: '/deployments',
  component: DeploymentsOverviewFeature,
})

// Create Deployment Route
const createDeploymentRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: '/deployments/create',
  component: PageCreateDeploymentFeature,
})

// Deployment Detail Route
const deploymentDetailRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: '/deployments/$deploymentId',
  component: PageDeploymentDetailFeature,
})

// Upgrades, per deployment: what it runs, what it can move to, and what the
// platform may apply without asking.
const deploymentUpgradesRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: '/deployments/$deploymentId/upgrades',
  component: PageUpgradesFeature,
})

// How much a deployment is being used, per deployment.
const deploymentUsageRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: '/deployments/$deploymentId/usage',
  component: PageUsageFeature,
})

// Data Plane Routes
const dataplanesRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: '/dataplanes',
  component: PageDataPlanesFeature,
})

const dataplaneDetailRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: '/dataplanes/$dataplaneId',
  component: PageDataPlaneDetailFeature,
})

// Release catalogue, operator only. The tab is hidden for everyone else and
// the API refuses them, so a customer reaching this URL sees an empty list
// rather than someone else's planning.
const releasesRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: '/releases',
  component: PageReleasesFeature,
})

// Create Organisation Route
const createOrganisationRoute = createRoute({
  getParentRoute: () => onboardingLayoutRoute,
  path: 'create',
  component: PageCreateOrganisationFeature,
})

// Index Route
const indexRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: '/',
  component: PageDashboardFeature,
})

const routeTree = rootRoute.addChildren([
  appLayoutRoute.addChildren([
    indexRoute,
    deploymentsRoute,
    createDeploymentRoute,
    deploymentDetailRoute,
    deploymentUpgradesRoute,
    deploymentUsageRoute,
    dataplanesRoute,
    dataplaneDetailRoute,
    releasesRoute,
  ]),
  onboardingLayoutRoute.addChildren([createOrganisationRoute]),
])

export const router = createRouter({ routeTree })

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}
