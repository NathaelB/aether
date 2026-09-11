import { useState } from 'react'
import { Link, useRouterState } from '@tanstack/react-router'
import { ChevronDown } from 'lucide-react'
import { cn } from '@/lib/utils'
import { isActive } from './nav-active'

export interface SideNavItem {
  label: string
  to: string
  icon?: React.ComponentType<{ className?: string }>
  exact?: boolean
}

export interface SideNavGroup {
  label: string
  icon?: React.ComponentType<{ className?: string }>
  items: SideNavItem[]
}

export type SideNavEntry = SideNavItem | SideNavGroup

function isGroup(entry: SideNavEntry): entry is SideNavGroup {
  return 'items' in entry
}

function itemClasses(active: boolean) {
  return cn(
    'flex items-center gap-2.5 rounded-md px-3 py-2 text-sm transition-colors',
    active
      ? 'bg-accent font-medium text-foreground'
      : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground',
  )
}

function Entry({ item, pathname }: { item: SideNavItem; pathname: string }) {
  const Icon = item.icon
  const active = isActive(pathname, item.to, item.exact)

  return (
    <Link to={item.to} className={itemClasses(active)}>
      {Icon && <Icon className='h-4 w-4 shrink-0' />}
      <span className='truncate'>{item.label}</span>
    </Link>
  )
}

function Group({ group, pathname }: { group: SideNavGroup; pathname: string }) {
  const holdsActive = group.items.some((item) => isActive(pathname, item.to, item.exact))
  // Opened because something inside it is where we are. Collapsing a group
  // while looking at one of its pages would hide the thing that is lit.
  const [open, setOpen] = useState(holdsActive)
  const Icon = group.icon

  return (
    <div>
      <button
        type='button'
        onClick={() => setOpen((wasOpen) => !wasOpen)}
        className={cn(itemClasses(false), 'w-full')}
      >
        {Icon && <Icon className='h-4 w-4 shrink-0' />}
        <span className='flex-1 truncate text-left'>{group.label}</span>
        <ChevronDown
          className={cn('h-3.5 w-3.5 shrink-0 transition-transform', !open && '-rotate-90')}
        />
      </button>

      {(open || holdsActive) && (
        <div className='ml-[1.4rem] space-y-0.5 border-l pl-2'>
          {group.items.map((item) => (
            <Entry key={item.to} item={item} pathname={pathname} />
          ))}
        </div>
      )}
    </div>
  )
}

/**
 * The vertical navigation inside a section.
 *
 * Sits beside the content rather than above it because these are many short
 * entries in a few categories, and a tab bar would either wrap or hide the
 * grouping that makes them findable.
 */
export function SideNav({ entries }: { entries: SideNavEntry[] }) {
  const pathname = useRouterState({ select: (state) => state.location.pathname })

  return (
    <nav className='w-full shrink-0 space-y-0.5 sm:w-60'>
      {entries.map((entry) =>
        isGroup(entry) ? (
          <Group key={entry.label} group={entry} pathname={pathname} />
        ) : (
          <Entry key={entry.to} item={entry} pathname={pathname} />
        ),
      )}
    </nav>
  )
}

/** A section laid out as a vertical navigation beside its content. */
export function SideNavLayout({
  entries,
  children,
}: {
  entries: SideNavEntry[]
  children: React.ReactNode
}) {
  return (
    <div className='mx-auto flex w-full max-w-6xl flex-col gap-8 px-6 py-8 sm:flex-row'>
      <SideNav entries={entries} />
      <div className='min-w-0 flex-1'>{children}</div>
    </div>
  )
}
