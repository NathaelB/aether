import { Outlet } from '@tanstack/react-router'
import { ChartLine, ScrollText, Waypoints } from 'lucide-react'
import { useDeploymentPath } from '@/domain/deployments/hooks/use-deployment-path'
import { SideNavLayout, type SideNavEntry } from './side-nav'

/**
 * Logs, traces and usage, grouped behind one top tab instead of three.
 *
 * The same shape as `DeploymentSettingsLayout`, on purpose: a side nav is
 * how this codebase already grows a section without growing the top tab
 * bar, and this section is going to grow (alerts, a service map) the same
 * way settings did.
 */
export function DeploymentObservabilityLayout() {
  const deploymentPath = useDeploymentPath()

  const entries: SideNavEntry[] = [
    {
      label: 'Logs',
      to: deploymentPath('/observability'),
      icon: ScrollText,
      exact: true,
    },
    {
      label: 'Traces',
      to: deploymentPath('/observability/traces'),
      icon: Waypoints,
    },
    {
      label: 'Usage',
      to: deploymentPath('/observability/usage'),
      icon: ChartLine,
    },
  ]

  return (
    <SideNavLayout entries={entries}>
      <Outlet />
    </SideNavLayout>
  )
}
