import { createRootRoute, createRoute, createRouter } from '@tanstack/react-router'
import { AppShell } from './components/layout/app-shell'
import { AppLayout } from './components/layout/main-layout'
import { DeploymentLayout } from './components/layout/deployment-layout'
import { DeploymentSettingsLayout } from './components/layout/deployment-settings-layout'
import { OnboardingLayout } from './components/layout/onboarding-layout'
import { PlatformLayout } from './components/layout/platform-layout'
import PageDashboardFeature from './domain/dashboard/pages/feature/page-dashboard-feature'
import DeploymentsOverviewFeature from './domain/deployments/pages/feature/page-deployments-overview-feature'
import PageCreateDeploymentFeature from './domain/deployments/pages/feature/page-create-deployment-feature'
import PageDeploymentDetailFeature from './domain/deployments/pages/feature/page-deployment-detail-feature'
import PageDeploymentGeneralFeature from './domain/deployments/pages/feature/page-deployment-general-feature'
import PageDeploymentResourcesFeature from './domain/deployments/pages/feature/page-deployment-resources-feature'
import PageDeploymentDangerFeature from './domain/deployments/pages/feature/page-deployment-danger-feature'
import PageCreateOrganisationFeature from './domain/organisations/pages/feature/page-create-organisation-feature'
import PageDataPlanesFeature from './domain/dataplanes/pages/feature/page-dataplanes-feature'
import PageDataPlaneDetailFeature from './domain/dataplanes/pages/feature/page-dataplane-detail-feature'
import PageReleasesFeature from './domain/releases/pages/feature/page-releases-feature'
import PageVersionFeature from './domain/upgrades/pages/feature/page-version-feature'
import PageAutomaticUpgradesFeature from './domain/upgrades/pages/feature/page-automatic-upgrades-feature'
import PageUsageFeature from './domain/usage/pages/feature/page-usage-feature'
import PageLogsFeature from './domain/logs/pages/feature/page-logs-feature'

const rootRoute = createRootRoute({
  component: AppShell,
})

// ---------------------------------------------------------------- the customer

// Everything a customer does happens inside one organisation, so the id is in
// the path rather than in a store: a link somebody pastes has to open the same
// thing it opened for them.
const appLayoutRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/organisations/$organisationId',
  component: AppLayout,
})

const indexRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: '/',
  component: PageDashboardFeature,
})

const deploymentsRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: '/deployments',
  component: DeploymentsOverviewFeature,
})

const createDeploymentRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: '/deployments/create',
  component: PageCreateDeploymentFeature,
})

// ------------------------------------------------------------- one deployment

// A sibling of the organisation's shell rather than a child of it, and
// therefore carrying the whole path. Nested, it would render its header
// inside the organisation's, and the page would open with two stacked
// headers and two rows of tabs.
const deploymentLayoutRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/organisations/$organisationId/deployments/$deploymentId',
  component: DeploymentLayout,
})

const deploymentOverviewRoute = createRoute({
  getParentRoute: () => deploymentLayoutRoute,
  path: '/',
  component: PageDeploymentDetailFeature,
})

const deploymentLogsRoute = createRoute({
  getParentRoute: () => deploymentLayoutRoute,
  path: '/logs',
  component: PageLogsFeature,
})

const deploymentUsageRoute = createRoute({
  getParentRoute: () => deploymentLayoutRoute,
  path: '/usage',
  component: PageUsageFeature,
})

const deploymentSettingsLayoutRoute = createRoute({
  getParentRoute: () => deploymentLayoutRoute,
  path: '/settings',
  component: DeploymentSettingsLayout,
})

const deploymentGeneralRoute = createRoute({
  getParentRoute: () => deploymentSettingsLayoutRoute,
  path: '/',
  component: PageDeploymentGeneralFeature,
})

const deploymentResourcesRoute = createRoute({
  getParentRoute: () => deploymentSettingsLayoutRoute,
  path: '/resources',
  component: PageDeploymentResourcesFeature,
})

const deploymentVersionRoute = createRoute({
  getParentRoute: () => deploymentSettingsLayoutRoute,
  path: '/version',
  component: PageVersionFeature,
})

const deploymentAutomaticUpgradesRoute = createRoute({
  getParentRoute: () => deploymentSettingsLayoutRoute,
  path: '/automatic-upgrades',
  component: PageAutomaticUpgradesFeature,
})

const deploymentDangerRoute = createRoute({
  getParentRoute: () => deploymentSettingsLayoutRoute,
  path: '/danger',
  component: PageDeploymentDangerFeature,
})

// ---------------------------------------------------------------- the platform

// Outside any organisation, because a data plane hosts several of them and the
// catalogue is offered to all of them. Nesting these under one organisation
// would say they belonged to it.
const platformLayoutRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/platform',
  component: PlatformLayout,
})

const platformDataPlanesRoute = createRoute({
  getParentRoute: () => platformLayoutRoute,
  path: '/dataplanes',
  component: PageDataPlanesFeature,
})

const platformDataPlaneDetailRoute = createRoute({
  getParentRoute: () => platformLayoutRoute,
  path: '/dataplanes/$dataplaneId',
  component: PageDataPlaneDetailFeature,
})

const platformReleasesRoute = createRoute({
  getParentRoute: () => platformLayoutRoute,
  path: '/releases',
  component: PageReleasesFeature,
})

// -------------------------------------------------------------------- onboarding

const onboardingLayoutRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/organisations',
  component: OnboardingLayout,
})

const createOrganisationRoute = createRoute({
  getParentRoute: () => onboardingLayoutRoute,
  path: 'create',
  component: PageCreateOrganisationFeature,
})

const routeTree = rootRoute.addChildren([
  appLayoutRoute.addChildren([indexRoute, deploymentsRoute, createDeploymentRoute]),
  deploymentLayoutRoute.addChildren([
    deploymentOverviewRoute,
    deploymentLogsRoute,
    deploymentUsageRoute,
    deploymentSettingsLayoutRoute.addChildren([
      deploymentGeneralRoute,
      deploymentResourcesRoute,
      deploymentVersionRoute,
      deploymentAutomaticUpgradesRoute,
      deploymentDangerRoute,
    ]),
  ]),
  platformLayoutRoute.addChildren([
    platformDataPlanesRoute,
    platformDataPlaneDetailRoute,
    platformReleasesRoute,
  ]),
  onboardingLayoutRoute.addChildren([createOrganisationRoute]),
])

export const router = createRouter({ routeTree })

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}
