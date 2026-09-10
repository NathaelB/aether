import { Outlet } from '@tanstack/react-router'
import { Boxes, LayoutGrid, Server } from 'lucide-react'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { NavTabs } from './nav-tabs'
import { TopBar } from './top-bar'

export function AppLayout() {
  const organisationPath = useOrganisationPath()

  return (
    <div className='min-h-svh bg-background'>
      <header className='sticky top-0 z-20 border-b bg-background'>
        <TopBar />
        <NavTabs
          tabs={[
            { label: 'Overview', to: organisationPath(), icon: LayoutGrid, exact: true },
            { label: 'Deployments', to: organisationPath('/deployments'), icon: Boxes },
            { label: 'Data planes', to: organisationPath('/dataplanes'), icon: Server },
          ]}
        />
      </header>
      <main>
        <Outlet />
      </main>
    </div>
  )
}
