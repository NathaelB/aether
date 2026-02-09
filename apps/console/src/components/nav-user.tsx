import {
  BadgeCheck,
  Bell,
  Check,
  ChevronsUpDown,
  CreditCard,
  Laptop,
  LogOut,
  Moon,
  Sun,
} from 'lucide-react'

import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from '@/components/ui/avatar'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  useSidebar,
} from '@/components/ui/sidebar'
import { useTheme } from './theme-provider'
import { useAuth } from 'react-oidc-context'
import { useAuthStore } from '@/stores/auth'

export function NavUser({
  user,
}: {
  user: {
    name: string
    email: string
    avatar?: string
  }
}) {
  const { isMobile } = useSidebar()
  const { theme, setTheme } = useTheme()
  const { user: oidcUser, removeUser } = useAuth()
  const { clear } = useAuthStore()

  const avatarFallback = user.name.split(' ').map((n) => n[0]).join('').slice(0, 2).toUpperCase()

  const handleSignOut = async () => {
    window.sessionStorage.setItem('aether:force_login_prompt', '1')
    clear()
    void removeUser().catch(() => undefined)

    const authority = window.oidcConfiguration?.authority?.replace(/\/$/, '')
    const clientId = window.oidcConfiguration?.client_id
    const idTokenHint = oidcUser?.id_token

    if (authority) {
      const endSessionEndpoint = await resolveEndSessionEndpoint(authority)

      if (endSessionEndpoint) {
        const logoutUrl = new URL(endSessionEndpoint)
        if (clientId) {
          logoutUrl.searchParams.set('client_id', clientId)
        }
        logoutUrl.searchParams.set('post_logout_redirect_uri', window.location.origin)
        if (idTokenHint) {
          logoutUrl.searchParams.set('id_token_hint', idTokenHint)
        }
        window.location.assign(logoutUrl.toString())
        return
      }

      window.location.assign(authority)
      return
    }

    window.location.assign('/')
  }

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <SidebarMenuButton
              size='lg'
              className='data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground'
            >
              <Avatar className='h-8 w-8 rounded-lg'>
                <AvatarImage src={user.avatar} alt={user.name} />
                <AvatarFallback className='rounded-lg'>{avatarFallback}</AvatarFallback>
              </Avatar>
              <div className='grid flex-1 text-left text-sm leading-tight'>
                <span className='truncate font-medium'>{user.name}</span>
                <span className='truncate text-xs'>{user.email}</span>
              </div>
              <ChevronsUpDown className='ml-auto size-4' />
            </SidebarMenuButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            className='w-(--radix-dropdown-menu-trigger-width) min-w-56 rounded-lg'
            side={isMobile ? 'bottom' : 'right'}
            align='end'
            sideOffset={4}
          >
            <DropdownMenuLabel className='p-0 font-normal'>
              <div className='flex items-center gap-2 px-1 py-1.5 text-left text-sm'>
                <Avatar className='h-8 w-8 rounded-lg'>
                  <AvatarImage src={user.avatar} alt={user.name} />
                  <AvatarFallback className='rounded-lg'>{avatarFallback}</AvatarFallback>
                </Avatar>
                <div className='grid flex-1 text-left text-sm leading-tight'>
                  <span className='truncate font-medium'>{user.name}</span>
                  <span className='truncate text-xs'>{user.email}</span>
                </div>
              </div>
            </DropdownMenuLabel>
            <DropdownMenuSeparator />

            <DropdownMenuGroup>
              <DropdownMenuItem>
                <BadgeCheck className='mr-2 h-4 w-4' />
                Account
              </DropdownMenuItem>
              <DropdownMenuItem>
                <CreditCard className='mr-2 h-4 w-4' />
                Billing
              </DropdownMenuItem>
              <DropdownMenuItem>
                <Bell className='mr-2 h-4 w-4' />
                Notifications
              </DropdownMenuItem>
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
            <DropdownMenuGroup>
              <DropdownMenuLabel className='font-normal text-xs text-muted-foreground px-2 py-1'>Theme</DropdownMenuLabel>
              <DropdownMenuItem onClick={() => setTheme('light')}>
                <Sun className='mr-2 h-4 w-4' />
                Light
                {theme === 'light' && <Check className='ml-auto h-4 w-4' />}
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => setTheme('dark')}>
                <Moon className='mr-2 h-4 w-4' />
                Dark
                {theme === 'dark' && <Check className='ml-auto h-4 w-4' />}
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => setTheme('system')}>
                <Laptop className='mr-2 h-4 w-4' />
                System
                {theme === 'system' && <Check className='ml-auto h-4 w-4' />}
              </DropdownMenuItem>
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => handleSignOut()}>
              <LogOut className='mr-2 h-4 w-4' />
              Log out
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarMenuItem>
    </SidebarMenu>
  )
}

async function resolveEndSessionEndpoint(authority: string): Promise<string | null> {
  const discoveryUrl = `${authority.replace(/\/$/, '')}/.well-known/openid-configuration`

  const response = await fetch(discoveryUrl, {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  }).catch(() => null)

  if (!response || !response.ok) {
    return null
  }

  const discovery = (await response.json()) as { end_session_endpoint?: string }
  return discovery.end_session_endpoint ?? null
}
