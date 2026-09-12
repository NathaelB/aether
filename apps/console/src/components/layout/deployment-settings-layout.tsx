import { Outlet } from '@tanstack/react-router'
import { ArrowUpCircle, Cpu, Settings2, Shield, Skull } from 'lucide-react'
import { useDeploymentPath } from '@/domain/deployments/hooks/use-deployment-path'
import { SideNavLayout, type SideNavEntry } from './side-nav'

/**
 * The settings of one deployment, grouped.
 *
 * Grouped rather than listed flat because the categories are the map: version
 * and automatic upgrades are two sides of one decision, and a flat list of
 * five entries makes that look like five unrelated screens.
 */
export function DeploymentSettingsLayout() {
  const deploymentPath = useDeploymentPath()

  const entries: SideNavEntry[] = [
    {
      label: 'General',
      to: deploymentPath('/settings'),
      icon: Settings2,
      exact: true,
    },
    {
      label: 'Resources',
      to: deploymentPath('/settings/resources'),
      icon: Cpu,
    },
    {
      label: 'Upgrades',
      icon: ArrowUpCircle,
      items: [
        { label: 'Version', to: deploymentPath('/settings/version') },
        { label: 'Automatic upgrades', to: deploymentPath('/settings/automatic-upgrades') },
      ],
    },
    {
      label: 'Network access',
      to: deploymentPath('/settings/network-access'),
      icon: Shield,
    },
    {
      label: 'Danger zone',
      to: deploymentPath('/settings/danger'),
      icon: Skull,
    },
  ]

  return (
    <SideNavLayout entries={entries}>
      <Outlet />
    </SideNavLayout>
  )
}
