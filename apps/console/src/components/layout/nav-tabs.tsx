import { Link, useRouterState } from '@tanstack/react-router'
import { cn } from '@/lib/utils'
import { isUnder } from '@/lib/paths'

export interface Tab {
  label: string
  to: string
  icon: React.ComponentType<{ className?: string }>
  exact?: boolean
}

export function NavTabs({ tabs }: { tabs: Tab[] }) {
  const pathname = useRouterState({ select: (state) => state.location.pathname })

  return (
    <nav className='flex items-center gap-1 px-4'>
      {tabs.map(({ label, to, icon: Icon, exact }) => {
        const active = isUnder(pathname, to, exact)

        return (
          <Link
            key={to}
            to={to}
            className={cn(
              '-mb-px flex items-center gap-2 border-b-2 px-3 py-2.5 text-sm transition-colors',
              active
                ? 'border-primary font-medium text-primary'
                : 'border-transparent text-muted-foreground hover:text-foreground',
            )}
          >
            <Icon className='h-4 w-4' />
            {label}
          </Link>
        )
      })}
    </nav>
  )
}
