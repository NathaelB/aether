"use client"

import * as React from "react"
import {
  LayoutDashboard,
  Shield,
  Box,
  FileText,
  Settings,
  Building2,
} from "lucide-react"

import { NavMain } from "@/components/nav-main"
import { NavUser } from "@/components/nav-user"
import { TeamSwitcher } from "@/components/team-switcher"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarRail,
} from "@/components/ui/sidebar"

// IAM as a Service navigation data
const data = {
  user: {
    name: "Admin User",
    email: "admin@aether.io",
    avatar: "/avatars/admin.jpg",
  },
  teams: [
    {
      name: "Aether",
      logo: Building2,
      plan: "Production",
    },
  ],
  navMain: [
    {
      title: "Dashboard",
      url: "/",
      icon: LayoutDashboard,
      isActive: true,
    },
    {
      title: "Instances",
      url: "/instances",
      icon: Box,
    },
    {
      title: "Backups",
      url: "/backups",
      icon: Shield,
    },
    {
      title: "Monitoring",
      url: "/monitoring",
      icon: FileText,
    },
    {
      title: "Settings",
      url: "/settings",
      icon: Settings,
    },
  ],
}

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  return (
    <Sidebar {...props} collapsible="icon" >
      <SidebarHeader>
        <TeamSwitcher teams={data.teams} />
      </SidebarHeader>
      <SidebarContent>
        <NavMain items={data.navMain} />
      </SidebarContent>
      <SidebarFooter>
        <NavUser user={data.user} />
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  )
}
