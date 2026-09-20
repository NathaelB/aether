import type { Schemas } from '@/api/api.client'
import { EmptyState, Page, PageTitle } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { StatusBadge } from '@/components/ui/status-badge'
import { format, formatDistanceToNow } from 'date-fns'
import { ScrollText, Server, UserCog } from 'lucide-react'
import { platformPath } from '@/lib/paths'
import { Link } from '@tanstack/react-router'
import { interrupts, toTrailLine } from '../../fleet-trail'

interface Props {
  entries: Schemas.FleetAuditEntry[]
  isLoading: boolean
  hasMore: boolean
  isLoadingMore: boolean
  onLoadMore: () => void
}

/**
 * A cluster links to itself; a subject does not.
 *
 * There is no screen for one operator, and a link that goes nowhere is worse
 * than plain text — it promises somewhere to look.
 */
function Target({ target, label }: { target: Schemas.FleetTarget; label: string }) {
  if (target.kind !== 'data_plane') {
    return (
      <span className='inline-flex items-center gap-1.5 font-mono text-xs text-muted-foreground'>
        <UserCog className='h-3.5 w-3.5' />
        {label}
      </span>
    )
  }

  return (
    <Link
      to={platformPath(`/dataplanes/${target.id}`)}
      className='inline-flex items-center gap-1.5 font-mono text-xs text-muted-foreground hover:text-primary'
    >
      <Server className='h-3.5 w-3.5' />
      {label}
    </Link>
  )
}

export function PageFleetTrail({
  entries,
  isLoading,
  hasMore,
  isLoadingMore,
  onLoadMore,
}: Props) {
  return (
    <Page>
      <PageTitle title='Trail' />

      <p className='mt-4 max-w-2xl text-sm text-muted-foreground'>
        What was done to this installation, newest first. Separate from an organisation's own
        trail, which records what happened inside it.
      </p>

      <div className='mt-6'>
        {isLoading ? (
          <div className='space-y-2'>
            <Skeleton className='h-16' />
            <Skeleton className='h-16' />
            <Skeleton className='h-16' />
          </div>
        ) : entries.length === 0 ? (
          <EmptyState
            icon={<ScrollText className='h-5 w-5' />}
            title='Nothing has been done yet'
            description='Registering a data plane, draining one, re-issuing a credential or granting somebody a platform right each leave a line here.'
          />
        ) : (
          <>
            <ul className='divide-y rounded-lg border bg-card'>
              {entries.map((entry) => {
                const line = toTrailLine(entry)

                return (
                  <li key={entry.id} className='flex flex-wrap items-baseline gap-x-3 gap-y-1 p-4'>
                    <span className='font-medium'>{line.action}</span>

                    <Target target={entry.target} label={line.target} />

                    {line.detail && (
                      <span className='font-mono text-xs text-muted-foreground'>
                        {line.detail}
                      </span>
                    )}

                    {interrupts(entry.action) && (
                      <StatusBadge tone='warning'>Interrupts service</StatusBadge>
                    )}

                    <span className='ml-auto flex items-baseline gap-3 text-xs text-muted-foreground'>
                      <span className='font-mono'>{line.actor}</span>
                      <time
                        dateTime={entry.recorded_at}
                        title={format(new Date(entry.recorded_at), 'PPpp')}
                      >
                        {formatDistanceToNow(new Date(entry.recorded_at), { addSuffix: true })}
                      </time>
                    </span>
                  </li>
                )
              })}
            </ul>

            {hasMore && (
              <div className='mt-4 flex justify-center'>
                <Button
                  size='sm'
                  variant='outline'
                  onClick={onLoadMore}
                  disabled={isLoadingMore}
                >
                  {isLoadingMore ? 'Reading…' : 'Read further back'}
                </Button>
              </div>
            )}
          </>
        )}
      </div>
    </Page>
  )
}
