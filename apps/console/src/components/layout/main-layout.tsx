import { Outlet } from '@tanstack/react-router'
import { Boxes, LayoutGrid, ShieldCheck, Users } from 'lucide-react'
import { useMyPermissions } from '@/domain/organisations/hooks/use-my-permissions'
import { CAN } from '@/domain/organisations/permissions'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { NavTabs, type Tab } from './nav-tabs'
import { TopBar } from './top-bar'

/**
 * What a customer sees.
 *
 * Data planes and the release catalogue are deliberately absent, for
 * operators too: they belong to whoever runs the installation, not to
 * whoever is inside one organisation, and mixing the two in one tab bar
 * makes every customer's console look like a control room they are locked
 * out of. They live under `/platform`.
 */
export function AppLayout() {
  const organisationPath = useOrganisationPath()
  const { can } = useMyPermissions()

  // A tab leading to a page that answers 403 is worse than no tab: it invites
  // the click and then blames the person for it.
  const tabs: Tab[] = [
    { label: 'Overview', to: organisationPath(), icon: LayoutGrid, exact: true },
    { label: 'Deployments', to: organisationPath('/deployments'), icon: Boxes },
    ...(can(CAN.viewMembers)
      ? [{ label: 'Members', to: organisationPath('/members'), icon: Users }]
      : []),
    ...(can(CAN.viewRoles)
      ? [{ label: 'Roles', to: organisationPath('/roles'), icon: ShieldCheck }]
      : []),
  ]

  return (
    <div className='min-h-svh bg-background'>
      <header className='sticky top-0 z-20 border-b bg-background'>
        <TopBar />
        <NavTabs tabs={tabs} />
      </header>
      <main>
        <Outlet />
      </main>
    </div>
  )
}
