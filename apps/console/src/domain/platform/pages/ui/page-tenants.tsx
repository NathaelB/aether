import type { Schemas } from '@/api/api.client'
import { EmptyState, Page, PageTitle } from '@/components/layout/page'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Building2 } from 'lucide-react'

interface Props {
  tenants: Schemas.Tenant[]
  isLoading: boolean
  onShowDeployments: (organisationId: string) => void
}

export function PageTenants({ tenants, isLoading, onShowDeployments }: Props) {
  return (
    <Page>
      <PageTitle title='Organisations' />

      <div className='mt-6'>
        {isLoading ? (
          <div className='space-y-2'>
            <Skeleton className='h-10' />
            <Skeleton className='h-10' />
          </div>
        ) : tenants.length === 0 ? (
          <EmptyState
            icon={<Building2 className='h-5 w-5' />}
            title='Nobody has signed up'
            description='No organisation exists on this installation yet.'
          />
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Organisation</TableHead>
                <TableHead>Plan</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className='text-right'>Deployments</TableHead>
                <TableHead className='text-right'>Members</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {tenants.map(({ organisation, deployments, members }) => (
                <TableRow key={organisation.id}>
                  <TableCell>
                    <button
                      type='button'
                      className='text-left font-medium hover:text-primary hover:underline'
                      onClick={() => onShowDeployments(organisation.id)}
                    >
                      {organisation.name}
                    </button>
                    <p className='font-mono text-xs text-muted-foreground'>{organisation.slug}</p>
                  </TableCell>
                  <TableCell className='text-muted-foreground'>{organisation.plan}</TableCell>
                  <TableCell className='text-muted-foreground'>{organisation.status}</TableCell>
                  {/* Tabular figures, so the counts line up down the column
                      rather than drifting with the width of each digit. */}
                  <TableCell className='text-right tabular-nums'>{deployments}</TableCell>
                  <TableCell className='text-right tabular-nums'>{members}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </div>
    </Page>
  )
}
