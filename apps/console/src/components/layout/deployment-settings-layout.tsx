import { Outlet } from '@tanstack/react-router'
import { Archive, ArrowUpCircle, Cpu, Settings2, Shield, Skull } from 'lucide-react'
import { useDeploymentPath } from '@/domain/deployments/hooks/use-deployment-path'
import { useMyPermissions } from '@/domain/organisations/hooks/use-my-permissions'
import { CAN } from '@/domain/organisations/permissions'
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
  const { can } = useMyPermissions()

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
    // Dropped rather than shown disabled. An entry somebody cannot open is a
    // question about why, and the page behind it guards itself anyway for
    // whoever arrives by a bookmark.
    ...(can(CAN.viewBackups)
      ? [
          {
            label: 'Backups',
            to: deploymentPath('/settings/backups'),
            icon: Archive,
          },
        ]
      : []),
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
