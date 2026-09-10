import type { Schemas } from '@/api/api.client'
import { Button } from '@/components/ui/button'
import { Card, EmptyState, Page, PageTitle, Section } from '@/components/layout/page'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Link } from '@tanstack/react-router'
import { formatDistanceToNow } from 'date-fns'
import { BookOpen, Boxes, ChevronRight, Plus, Server } from 'lucide-react'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { KIND_LABELS } from '@/domain/deployments/types/deployment'
import { DeploymentStatusBadge } from '@/domain/deployments/pages/ui/components/deployment-status'
import { DataPlaneStatusBadge } from '@/domain/dataplanes/pages/ui/components/dataplane-badges'

interface Props {
  organisationName: string
  deployments: Schemas.Deployment[]
  dataplanes: Schemas.DataPlane[]
  isLoading: boolean
}

function Stat({ label, value }: { label: string; value: number | string }) {
  return (
    <Card className='p-4'>
      <p className='text-sm text-muted-foreground'>{label}</p>
      <p className='mt-1 text-3xl font-semibold tabular-nums'>{value}</p>
    </Card>
  )
}

function LinkRow({ icon, label, href }: { icon: React.ReactNode; label: string; href: string }) {
  return (
    <a
      href={href}
      target='_blank'
      rel='noreferrer'
      className='flex items-center gap-3 rounded-lg border bg-card px-4 py-3 text-sm transition-colors hover:border-primary/40'
    >
      <span className='flex h-7 w-7 items-center justify-center rounded-md border bg-muted/40'>
        {icon}
      </span>
      <span className='flex-1'>{label}</span>
      <ChevronRight className='h-4 w-4 text-muted-foreground' />
    </a>
  )
}

export const PageDashboard = ({
  organisationName,
  deployments,
  dataplanes,
  isLoading,
}: Props) => {
  const organisationPath = useOrganisationPath()
  const running = deployments.filter((d) => d.status === 'successful').length
  const failed = deployments.filter((d) => d.status === 'failed').length
  const recent = deployments.slice(0, 5)

  return (
    <Page>
      <PageTitle
        icon={
          <span className='flex h-8 w-8 items-center justify-center rounded-full border text-sm font-medium uppercase'>
            {organisationName.charAt(0)}
          </span>
        }
        title={organisationName}
      />

      <div className='mt-8 grid gap-8 lg:grid-cols-[1fr_20rem]'>
        <div className='space-y-8'>
          <div className='grid gap-4 sm:grid-cols-4'>
            <Stat label='Deployments' value={isLoading ? '—' : deployments.length} />
            <Stat label='Running' value={isLoading ? '—' : running} />
            <Stat label='Failed' value={isLoading ? '—' : failed} />
            <Stat label='Data planes' value={isLoading ? '—' : dataplanes.length} />
          </div>

          <Section
            title='Recent deployments'
            aside={
              deployments.length > 0 && (
                <Link
                  to={organisationPath('/deployments')}
                  className='flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground'
                >
                  All deployments <ChevronRight className='h-3.5 w-3.5' />
                </Link>
              )
            }
          >
            {recent.length === 0 ? (
              <EmptyState
                icon={<Boxes className='h-5 w-5' />}
                title={isLoading ? 'Loading…' : 'No deployment yet'}
                description={
                  isLoading ? undefined : 'Create your first identity provider deployment.'
                }
                action={
                  !isLoading && (
                    <Button size='sm' asChild>
                      <Link to={organisationPath('/deployments/create')}>
                        <Plus className='h-4 w-4' />
                        New deployment
                      </Link>
                    </Button>
                  )
                }
              />
            ) : (
              <div className='overflow-x-auto rounded-lg border'>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Name</TableHead>
                      <TableHead>Provider</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead className='text-right'>Created</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {recent.map((deployment) => (
                      <TableRow key={deployment.id}>
                        <TableCell className='font-medium'>
                          <Link
                            to={organisationPath(`/deployments/${deployment.id}`)}
                            className='hover:underline'
                          >
                            {deployment.name}
                          </Link>
                        </TableCell>
                        <TableCell className='text-muted-foreground'>
                          {KIND_LABELS[deployment.kind]}
                        </TableCell>
                        <TableCell>
                          <DeploymentStatusBadge status={deployment.status} />
                        </TableCell>
                        <TableCell className='text-right text-xs text-muted-foreground'>
                          {formatDistanceToNow(new Date(deployment.created_at))} ago
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            )}
          </Section>

          {dataplanes.length > 0 && (
            <Section
              title='Data planes'
              aside={
                <Link
                  to={organisationPath('/dataplanes')}
                  className='flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground'
                >
                  All data planes <ChevronRight className='h-3.5 w-3.5' />
                </Link>
              }
            >
              <div className='grid gap-3 sm:grid-cols-2'>
                {dataplanes.slice(0, 4).map((dataplane) => (
                  <Link
                    key={dataplane.id}
                    to={organisationPath(`/dataplanes/${dataplane.id}`)}
                    className='flex items-center gap-3 rounded-lg border bg-card px-4 py-3 transition-colors hover:border-primary/40'
                  >
                    <span className='flex h-8 w-8 items-center justify-center rounded-md border bg-muted/40'>
                      <Server className='h-4 w-4 text-muted-foreground' />
                    </span>
                    <span className='flex-1 truncate text-sm font-medium'>{dataplane.region}</span>
                    <DataPlaneStatusBadge status={dataplane.status} />
                  </Link>
                ))}
              </div>
            </Section>
          )}
        </div>

        <aside className='space-y-3'>
          <h2 className='text-base font-semibold'>Useful links</h2>
          <LinkRow
            icon={<BookOpen className='h-3.5 w-3.5 text-muted-foreground' />}
            label='Documentation'
            href='https://github.com/NathaelB/aether'
          />
          <LinkRow
            icon={<Boxes className='h-3.5 w-3.5 text-muted-foreground' />}
            label='FerrisKey'
            href='https://github.com/ferriskey/ferriskey'
          />
        </aside>
      </div>
    </Page>
  )
}
