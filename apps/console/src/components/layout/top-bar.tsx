import { Link } from '@tanstack/react-router'
import { ChevronsUpDown, Search } from 'lucide-react'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { useAuthStore } from '@/stores/auth'
import {
  selectActiveOrganisationId,
  selectOrganisations,
  useOrganisationsStore,
} from '@/stores/organisations'
import { useNavigate } from '@tanstack/react-router'
import { cn } from '@/lib/utils'

export interface Crumb {
  label: string
  to?: string
  icon?: React.ReactNode
}

function Initial({ label }: { label: string }) {
  return (
    <span className='flex h-6 w-6 shrink-0 items-center justify-center rounded-full border text-[11px] font-medium uppercase'>
      {label.charAt(0)}
    </span>
  )
}

export function TopBar({ crumbs = [] }: { crumbs?: Crumb[] }) {
  const navigate = useNavigate()
  const { profile } = useAuthStore()
  const organisations = useOrganisationsStore(selectOrganisations)
  const activeId = useOrganisationsStore(selectActiveOrganisationId)
  const setActiveId = useOrganisationsStore((state) => state.setActiveOrganisationId)

  const active = organisations.find((organisation) => organisation.id === activeId)

  return (
    <div className='flex h-14 items-center gap-3 px-4'>
      <Link to='/' className='shrink-0'>
        <span className='flex h-7 w-7 items-center justify-center rounded-lg bg-primary text-sm font-semibold text-primary-foreground'>
          A
        </span>
      </Link>

      <span className='text-muted-foreground/60'>/</span>

      <DropdownMenu>
        <DropdownMenuTrigger className='flex items-center gap-2 rounded-md px-1.5 py-1 text-sm font-medium hover:bg-accent'>
          {active && <Initial label={active.name} />}
          <span className='max-w-40 truncate'>{active?.name ?? 'Organisation'}</span>
          <ChevronsUpDown className='h-3.5 w-3.5 text-muted-foreground' />
        </DropdownMenuTrigger>
        <DropdownMenuContent align='start' className='w-56'>
          <DropdownMenuLabel className='text-xs text-muted-foreground'>
            Organisations
          </DropdownMenuLabel>
          {organisations.map((organisation) => (
            <DropdownMenuItem
              key={organisation.id}
              onClick={() => {
                setActiveId(organisation.id)
                navigate({ to: `/organisations/${organisation.id}` })
              }}
            >
              <Initial label={organisation.name} />
              {organisation.name}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>

      {crumbs.map((crumb, index) => (
        <div key={index} className='flex min-w-0 items-center gap-3'>
          <span className='text-muted-foreground/60'>/</span>
          {crumb.to ? (
            <Link
              to={crumb.to}
              className='flex min-w-0 items-center gap-1.5 text-sm hover:underline'
            >
              {crumb.icon}
              <span className='truncate'>{crumb.label}</span>
            </Link>
          ) : (
            <span className='flex min-w-0 items-center gap-1.5 text-sm font-medium'>
              {crumb.icon}
              <span className='truncate'>{crumb.label}</span>
            </span>
          )}
        </div>
      ))}

      <div className='ml-auto flex items-center gap-2'>
        <button
          type='button'
          className={cn(
            'hidden items-center gap-2 rounded-md border px-2.5 py-1.5 text-sm text-muted-foreground sm:flex',
            'hover:bg-accent',
          )}
        >
          <Search className='h-3.5 w-3.5' />
          <span className='w-40 text-left'>Search</span>
          <kbd className='rounded border bg-muted px-1 text-[10px]'>⌘K</kbd>
        </button>

        <DropdownMenu>
          <DropdownMenuTrigger className='rounded-full outline-none'>
            <Avatar className='h-7 w-7'>
              <AvatarFallback className='text-xs'>
                {(profile?.preferred_username ?? '?').charAt(0).toUpperCase()}
              </AvatarFallback>
            </Avatar>
          </DropdownMenuTrigger>
          <DropdownMenuContent align='end' className='w-56'>
            <DropdownMenuLabel className='space-y-0.5'>
              <p className='truncate text-sm'>{profile?.preferred_username}</p>
              <p className='truncate text-xs font-normal text-muted-foreground'>{profile?.email}</p>
            </DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => useAuthStore.getState().clear()}>
              Sign out
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  )
}
