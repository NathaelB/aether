import { Outlet } from '@tanstack/react-router'
import { Server, ShieldAlert, Tag } from 'lucide-react'
import { EmptyState, Page } from '@/components/layout/page'
import { useIsOperator } from '@/domain/organisations/hooks/use-is-operator'
import { platformPath } from '@/lib/paths'
import { NavTabs, type Tab } from './nav-tabs'
import { TopBar } from './top-bar'

const TABS: Tab[] = [
  { label: 'Data planes', to: platformPath('/dataplanes'), icon: Server },
  { label: 'Releases', to: platformPath('/releases'), icon: Tag },
]

/**
 * What whoever runs the installation sees.
 *
 * Outside any organisation, because everything here is installation-wide: a
 * data plane hosts several organisations and the catalogue is offered to all
 * of them. Nesting these under one organisation would say they belonged to it.
 */
export function PlatformLayout() {
  const isOperator = useIsOperator()

  return (
    <div className='min-h-svh bg-background'>
      <header className='sticky top-0 z-20 border-b bg-background'>
        <TopBar variant='platform' />
        {isOperator && <NavTabs tabs={TABS} />}
      </header>
      <main>
        {isOperator ? (
          <Outlet />
        ) : (
          // Said plainly rather than left as an empty list. The API refuses
          // these endpoints too, so someone who reaches this URL would
          // otherwise see screens that look broken rather than closed.
          <Page>
            <EmptyState
              icon={<ShieldAlert className='h-5 w-5' />}
              title='This is not yours to see'
              description='Running the installation is a separate right from using it. Ask whoever operates this platform if you need it.'
            />
          </Page>
        )}
      </main>
    </div>
  )
}
