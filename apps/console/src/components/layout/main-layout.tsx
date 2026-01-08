
import { AppSidebar } from '@/components/app-sidebar'
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb'
import { Separator } from '@/components/ui/separator'
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from '@/components/ui/sidebar'
import { Outlet } from '@tanstack/react-router'
import { SetupAppLayout } from './setup-app-layout'
import { useEffect, useState } from 'react'
import { initializeAppConfig } from '@/lib/config-app'

export function MainLayout() {
  const [isConfiguring, setIsConfiguring] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    async function setupApp() {
      setIsConfiguring(true)

      const { error: configError } = await initializeAppConfig()

      if (configError) {
        setError(configError)
      }

      setIsConfiguring(false)
    }

    setupApp()
  }, [])

  return (
    <SetupAppLayout isConfiguring={isConfiguring} error={error}>
      <SidebarProvider
        defaultOpen={false}

      >

        <AppSidebar variant='floating' />
        <SidebarInset>
          <header className='flex h-16 shrink-0 items-center gap-2 transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-12'>
            <div className='flex items-center gap-2 px-4'>
              <SidebarTrigger className='-ml-1' />
              <Separator
                orientation='vertical'
                className='mr-2 data-[orientation=vertical]:h-4'
              />
              <Breadcrumb>
                <BreadcrumbList>
                  <BreadcrumbItem className='hidden md:block'>
                    <BreadcrumbLink href='#'>
                      Building Your Application
                    </BreadcrumbLink>
                  </BreadcrumbItem>
                  <BreadcrumbSeparator className='hidden md:block' />
                  <BreadcrumbItem>
                    <BreadcrumbPage>Data Fetching</BreadcrumbPage>
                  </BreadcrumbItem>
                </BreadcrumbList>
              </Breadcrumb>
            </div>
          </header>
          <div className='flex flex-1 flex-col gap-4 p-4 pt-0'>
            <Outlet />
          </div>
        </SidebarInset>
      </SidebarProvider>
    </SetupAppLayout>
  )
}
