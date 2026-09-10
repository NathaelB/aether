import { Outlet } from '@tanstack/react-router'
import { Boxes, LayoutGrid, Server } from 'lucide-react'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { useIsOperator } from '@/domain/organisations/hooks/use-is-operator'
import { NavTabs, type Tab } from './nav-tabs'
import { TopBar } from './top-bar'

export function AppLayout() {
  const organisationPath = useOrganisationPath()
  const isOperator = useIsOperator()

  const tabs: Tab[] = [
    { label: 'Overview', to: organisationPath(), icon: LayoutGrid, exact: true },
    { label: 'Deployments', to: organisationPath('/deployments'), icon: Boxes },
  ]

  if (isOperator) {
    tabs.push({ label: 'Data planes', to: organisationPath('/dataplanes'), icon: Server })
  }

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
