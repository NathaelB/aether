'use client'

import * as React from 'react'
import {
  LayoutDashboard,
  Shield,
  Box,
  FileText,
  Settings,
  Building2,
} from 'lucide-react'

import { NavMain } from '@/components/nav-main'
import { NavUser } from '@/components/nav-user'
import { TeamSwitcher } from '@/components/team-switcher'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarRail,
} from '@/components/ui/sidebar'
import { useAuthStore } from '@/stores/auth'
import { useNavigate } from '@tanstack/react-router'
import {
  selectActiveOrganisationId,
  selectOrganisations,
  useOrganisationsStore,
} from '@/stores/organisations'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  const { profile } = useAuthStore()
  const navigate = useNavigate()
  const organisations = useOrganisationsStore(selectOrganisations)
  const activeOrganisationId = useOrganisationsStore(selectActiveOrganisationId)
  const setActiveOrganisationId = useOrganisationsStore(
    (state) => state.setActiveOrganisationId
  )
  const organisationPath = useOrganisationPath()

  if (!profile || !profile.name || !profile.email) {
    return null
  }

  if (organisations.length === 0) {
    return null
  }

  const user = {
    name: profile.name,
    email: profile.email,
  }

  const teams = organisations.map((organisation) => ({
    id: organisation.id,
    name: organisation.name,
    logo: Building2,
    plan: organisation.plan,
  }))

  const navMain = [
    {
      title: 'Dashboard',
      url: organisationPath(),
      icon: LayoutDashboard,
      isActive: true,
    },
    {
      title: 'Deployments',
      url: organisationPath('/deployments'),
      icon: Box,
    },
    {
      title: 'Backups',
      url: organisationPath('/backups'),
      icon: Shield,
    },
    {
      title: 'Monitoring',
      url: organisationPath('/monitoring'),
      icon: FileText,
    },
    {
      title: 'Settings',
      url: organisationPath('/settings'),
      icon: Settings,
    },
  ]

  return (
    <Sidebar {...props} collapsible='icon' >
      <SidebarHeader>
        <TeamSwitcher
          teams={teams}
          activeTeamId={activeOrganisationId}
          onSelectTeam={(organisationId) => {
            setActiveOrganisationId(organisationId)
            navigate({ to: `/organisations/${organisationId}` })
          }}
        />
      </SidebarHeader>
      <SidebarContent>
        <NavMain items={navMain} />
      </SidebarContent>
      <SidebarFooter>
        <NavUser user={user} />
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  )
}
