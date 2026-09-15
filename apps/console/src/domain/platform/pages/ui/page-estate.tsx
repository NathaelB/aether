import type { Schemas } from '@/api/api.client'
import { EmptyState, Page, PageTitle } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { DeploymentStatusBadge } from '@/domain/deployments/pages/ui/components/deployment-status'
import { Boxes, DatabaseBackup } from 'lucide-react'
import type { EstateFilters } from '../../estate-filters'
import { isNarrowed } from '../../estate-filters'

interface Props {
  deployments: Schemas.EstateDeployment[]
  isLoading: boolean
  filters: EstateFilters
  onFilter: (filters: EstateFilters) => void
  /** The deployment an archive is being asked for, if any. */
  asking?: string
  onAskForBackup: (deployment: Schemas.EstateDeployment) => void

  /**
   * What came of the last ask.
   *
   * Said on the page rather than left to the row going quiet. Asking for an
   * archive produces nothing a screen can show — the data plane has been
   * told, and the archive turns up in that deployment's own list minutes
   * later — so without this the button looks like it did nothing.
   */
  outcome?: { ok: boolean; message: string }
}

export function PageEstate({
  deployments,
  isLoading,
  filters,
  onFilter,
  asking,
  onAskForBackup,
  outcome,
}: Props) {
  return (
    <Page>
      <PageTitle
        title='Deployments'
        actions={
          isNarrowed(filters) ? (
            <Button variant='outline' size='sm' onClick={() => onFilter({})}>
              Clear filters
            </Button>
          ) : undefined
        }
      />

      {outcome && (
        <div
          className={
            outcome.ok
              ? 'mt-6 rounded-lg border border-primary/30 bg-primary/5 px-4 py-3 text-sm'
              : 'mt-6 rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm'
          }
          role='status'
        >
          {outcome.message}
        </div>
      )}

      <div className='mt-6'>
        {isLoading ? (
          <div className='space-y-2'>
            <Skeleton className='h-10' />
            <Skeleton className='h-10' />
            <Skeleton className='h-10' />
          </div>
        ) : deployments.length === 0 ? (
          <EmptyState
            icon={<Boxes className='h-5 w-5' />}
            title={isNarrowed(filters) ? 'Nothing matches' : 'Nothing is deployed here'}
            description={
              isNarrowed(filters)
                ? 'No deployment on this installation matches what you narrowed to.'
                : 'No organisation has deployed anything on this installation yet.'
            }
          />
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Deployment</TableHead>
                <TableHead>Organisation</TableHead>
                <TableHead>Environment</TableHead>
                <TableHead>Offer</TableHead>
                <TableHead>Region</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className='text-right'>Archive</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {deployments.map(({ deployment, organisation, region }) => (
                <TableRow key={deployment.id}>
                  <TableCell>
                    <p className='font-medium'>{deployment.name}</p>
                    <p className='font-mono text-xs text-muted-foreground'>
                      {deployment.kind} {deployment.version}
                    </p>
                  </TableCell>
                  <TableCell>
                    {/* Clicking the organisation narrows to it: "what does
                        this tenant have" is the question asked right after
                        "who is on this cluster". */}
                    <button
                      type='button'
                      className='text-left hover:text-primary hover:underline'
                      onClick={() => onFilter({ ...filters, organisationId: organisation.id })}
                    >
                      {organisation.name}
                    </button>
                  </TableCell>
                  <TableCell className='text-muted-foreground'>
                    {deployment.environment}
                  </TableCell>
                  <TableCell className='text-muted-foreground'>
                    {/* A deployment created before the catalogue has none, and
                        an invented one would be a claim about what somebody
                        bought. */}
                    {deployment.offer ?? '—'}
                  </TableCell>
                  <TableCell>
                    <button
                      type='button'
                      className='font-mono text-xs hover:text-primary hover:underline'
                      onClick={() => onFilter({ ...filters, region })}
                    >
                      {region}
                    </button>
                  </TableCell>
                  <TableCell>
                    <DeploymentStatusBadge status={deployment.status} />
                  </TableCell>
                  <TableCell className='text-right'>
                    <Button
                      variant='outline'
                      size='sm'
                      disabled={asking === deployment.id}
                      onClick={() => onAskForBackup({ deployment, organisation, region })}
                    >
                      <DatabaseBackup className='mr-1.5 h-3.5 w-3.5' />
                      {asking === deployment.id ? 'Asking…' : 'Back up now'}
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </div>
    </Page>
  )
}
