import { Outlet, useParams } from '@tanstack/react-router'
import { Boxes, ChartLine, LayoutGrid, ScrollText, Settings } from 'lucide-react'
import { useGetDeployment } from '@/api/deployment.api'
import { useDeploymentPath } from '@/domain/deployments/hooks/use-deployment-path'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { NavTabs, type Tab } from './nav-tabs'
import { TopBar, type Crumb } from './top-bar'

/**
 * A deployment gets the whole bar to itself.
 *
 * Keeping the organisation's tabs here would mean the header answers a
 * question nobody is asking any more: once a deployment is open, everything
 * on screen is about that one instance, and "Overview" would mean two
 * different things one line apart.
 */
export function DeploymentLayout() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationPath = useOrganisationPath()
  const deploymentPath = useDeploymentPath()

  const deployment = useGetDeployment(deploymentId ?? null)
  const name = deployment.data?.data.name

  const tabs: Tab[] = [
    { label: 'Overview', to: deploymentPath(), icon: LayoutGrid, exact: true },
    { label: 'Logs', to: deploymentPath('/logs'), icon: ScrollText },
    { label: 'Usage', to: deploymentPath('/usage'), icon: ChartLine },
    { label: 'Settings', to: deploymentPath('/settings'), icon: Settings },
  ]

  const crumbs: Crumb[] = [
    {
      label: 'Deployments',
      to: organisationPath('/deployments'),
      icon: <Boxes className='h-3.5 w-3.5 text-muted-foreground' />,
    },
    // Named while it loads rather than left blank: an empty crumb makes the
    // header jump, and the id in the URL is not a name anybody recognises.
    { label: name ?? 'Loading' },
  ]

  return (
    <div className='min-h-svh bg-background'>
      <header className='sticky top-0 z-20 border-b bg-background'>
        <TopBar crumbs={crumbs} />
        <NavTabs tabs={tabs} />
      </header>
      <main>
        <Outlet />
      </main>
    </div>
  )
}
