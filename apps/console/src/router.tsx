import { createRootRoute, createRoute, createRouter } from '@tanstack/react-router';
import InstancesOverviewFeature from './domain/instances/pages/feature/page-instances-overview-feature';
import { PageDashboard } from './domain/dashboard/pages/ui/page-dashboard';
import { MainLayout } from './components/layout/main-layout';

// Root Route
const rootRoute = createRootRoute({
  component: MainLayout,
});

// Instances Route
const instancesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/instances',
  component: InstancesOverviewFeature,
});

// Index Route
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: PageDashboard,
});

const routeTree = rootRoute.addChildren([indexRoute, instancesRoute]);

export const router = createRouter({ routeTree });

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}
