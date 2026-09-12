import { Lock } from 'lucide-react'
import { EmptyState, Page } from '@/components/layout/page'
import { Skeleton } from '@/components/ui/skeleton'
import { useMyPermissions } from '@/domain/organisations/hooks/use-my-permissions'

/**
 * A page nobody can hide behind a missing tab.
 *
 * The navigation drops what the caller may not open, but a bookmark, a pasted
 * link or a browser's back button reach a page regardless. Without this they
 * reach one that renders empty because every request behind it answered 403,
 * which reads as the platform being broken rather than as a door.
 */
export function RequiresPermission({
  permission,
  what,
  children,
}: {
  permission: number
  /** Named in the refusal, so the reader knows what to ask for. */
  what: string
  children: React.ReactNode
}) {
  const { can, isLoading } = useMyPermissions()

  if (isLoading) {
    return (
      <Page>
        <Skeleton className='h-64 w-full' />
      </Page>
    )
  }

  if (!can(permission)) {
    return (
      <Page>
        <EmptyState
          icon={<Lock className='h-5 w-5' />}
          title={`You cannot ${what} in this organisation`}
          description='Somebody who manages its members can grant you a role that allows it.'
        />
      </Page>
    )
  }

  return <>{children}</>
}
