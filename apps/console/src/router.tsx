import { createRootRoute, createRoute, createRouter } from '@tanstack/react-router'
import DeploymentsOverviewFeature from './domain/deployments/pages/feature/page-deployments-overview-feature'
import PageCreateDeploymentFeature from './domain/deployments/pages/feature/page-create-deployment-feature'
import PageDeploymentDetailFeature from './domain/deployments/pages/feature/page-deployment-detail-feature'
import { PageDashboard } from './domain/dashboard/pages/ui/page-dashboard'
import { AppLayout } from './components/layout/main-layout'
import { AppShell } from './components/layout/app-shell'
import { OnboardingLayout } from './components/layout/onboarding-layout'
import PageCreateOrganisationFeature from './domain/organisations/pages/feature/page-create-organisation-feature'

// Root Route
const rootRoute = createRootRoute({
  component: AppShell,
})

// App Layout Route (with sidebar + breadcrumb)
const appLayoutRoute = createRoute({
  getParentRoute: () => rootRoute,
  id: 'app',
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
  component: PageDashboard,
})

const routeTree = rootRoute.addChildren([
  appLayoutRoute.addChildren([
    indexRoute,
    deploymentsRoute,
    createDeploymentRoute,
    deploymentDetailRoute,
  ]),
  onboardingLayoutRoute.addChildren([createOrganisationRoute]),
])

export const router = createRouter({ routeTree })

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}
