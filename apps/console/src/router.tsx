import { createRootRoute, createRoute, createRouter } from '@tanstack/react-router'
import DeploymentsOverviewFeature from './domain/deployments/pages/feature/page-deployments-overview-feature'
import PageCreateDeploymentFeature from './domain/deployments/pages/feature/page-create-deployment-feature'
import PageDeploymentDetailFeature from './domain/deployments/pages/feature/page-deployment-detail-feature'
import { PageDashboard } from './domain/dashboard/pages/ui/page-dashboard'
import { MainLayout } from './components/layout/main-layout'

// Root Route
const rootRoute = createRootRoute({
  component: MainLayout,
})

// Deployments Route
const deploymentsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/deployments',
  component: DeploymentsOverviewFeature,
})

// Create Deployment Route
const createDeploymentRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/deployments/create',
  component: PageCreateDeploymentFeature,
})

// Deployment Detail Route
const deploymentDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/deployments/$deploymentId',
  component: PageDeploymentDetailFeature,
})

// Index Route
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: PageDashboard,
})

const routeTree = rootRoute.addChildren([indexRoute, deploymentsRoute, createDeploymentRoute, deploymentDetailRoute])

export const router = createRouter({ routeTree })

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}
