import type { Schemas } from '@/api/api.client'
import { EmptyState, Page, PageTitle, Section } from '@/components/layout/page'
import { StatusBadge } from '@/components/ui/status-badge'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { cn } from '@/lib/utils'
import { AlertTriangle, ChevronDown, Globe, Plus, Tag } from 'lucide-react'
import { useState } from 'react'
import { PublishReleaseSheet } from './publish-release-sheet'
import { audienceLabel, isGloballyAvailable, nextPatchIn, releaseLines } from '../../catalogue'
import {
  RELEASE_STATUS_LABELS,
  RELEASE_STATUS_TONES,
  RISK_LABELS,
  RISK_TONES,
  nextStatuses,
  withdrawalStrands,
} from '../../status'

const KINDS: { value: Schemas.DeploymentKind; label: string }[] = [
  { value: 'ferriskey', label: 'FerrisKey' },
  { value: 'keycloak', label: 'Keycloak' },
]

interface Props {
  kind: Schemas.DeploymentKind
  onKindChange: (kind: Schemas.DeploymentKind) => void
  releases: Schemas.ReleaseInUse[]
  isLoading: boolean
  onMove: (version: string, status: Schemas.ReleaseStatus) => void
  isMoving: boolean
  onPublish: (request: Schemas.PublishReleaseRequest) => void
  isPublishing: boolean
  onOfferToEveryone: (version: string) => void
  isWidening: boolean
}

export function PageReleases({
  kind,
  onKindChange,
  releases,
  isLoading,
  onMove,
  isMoving,
  onPublish,
  isPublishing,
  onOfferToEveryone,
  isWidening,
}: Props) {
  const [publishing, setPublishing] = useState(false)
  const [suggestedVersion, setSuggestedVersion] = useState<string | null>(null)

  const lines = releaseLines(releases)

  const openPublish = (version: string | null) => {
    setSuggestedVersion(version)
    setPublishing(true)
  }

  return (
    <Page>
      <PageTitle
        title='Releases'
        actions={
          <>
            <Tabs
              value={kind}
              onValueChange={(value) => onKindChange(value as Schemas.DeploymentKind)}
            >
              <TabsList>
                {KINDS.map(({ value, label }) => (
                  <TabsTrigger key={value} value={value}>
                    {label}
                  </TabsTrigger>
                ))}
              </TabsList>
            </Tabs>
            <Button size='sm' onClick={() => openPublish(null)}>
              <Plus className='h-4 w-4' />
              Publish a version
            </Button>
          </>
        }
      />

      <Section title='Catalogue' className='mt-8'>
        {isLoading ? (
          <div className='space-y-2'>
            <Skeleton className='h-16' />
            <Skeleton className='h-16' />
          </div>
        ) : releases.length === 0 ? (
          <EmptyState
            icon={<Tag className='h-5 w-5' />}
            title='No release recorded'
            description='Nothing can be created or upgraded to until a version is published here.'
            action={
              <Button size='sm' onClick={() => openPublish(null)}>
                <Plus className='h-4 w-4' />
                Publish a version
              </Button>
            }
          />
        ) : (
          <div className='space-y-5'>
            {lines.map((line) => (
              <div key={line.label}>
                <div className='mb-2 flex items-center justify-between gap-4'>
                  <h3 className='font-mono text-sm font-medium text-muted-foreground'>
                    {line.label}
                  </h3>
                  <Button
                    variant='ghost'
                    size='sm'
                    onClick={() => openPublish(nextPatchIn(line))}
                  >
                    <Plus className='h-3.5 w-3.5' />
                    Add a patch
                  </Button>
                </div>
                <ul className='divide-y rounded-lg border'>
                  {line.releases.map((release) => (
                    <ReleaseRow
                      key={release.id.version}
                      release={release as Schemas.ReleaseInUse}
                      onMove={onMove}
                      isMoving={isMoving}
                      onOfferToEveryone={onOfferToEveryone}
                      isWidening={isWidening}
                    />
                  ))}
                </ul>
              </div>
            ))}
          </div>
        )}
      </Section>

      <PublishReleaseSheet
        open={publishing}
        onOpenChange={setPublishing}
        suggestedVersion={suggestedVersion}
        kind={kind}
        isPublishing={isPublishing}
        onPublish={(request) => {
          onPublish(request)
          setPublishing(false)
        }}
      />
    </Page>
  )
}

function ReleaseRow({
  release,
  onMove,
  isMoving,
  onOfferToEveryone,
  isWidening,
}: {
  release: Schemas.ReleaseInUse
  onMove: (version: string, status: Schemas.ReleaseStatus) => void
  isMoving: boolean
  onOfferToEveryone: (version: string) => void
  isWidening: boolean
}) {
  const forward = nextStatuses(release.status)
  const stranded = withdrawalStrands(release.status, release.deployments)
  const global = isGloballyAvailable(release)
  // Only once it may be installed at all. Widening a planned or withdrawn
  // release would say it is offered to everyone while its status says nobody
  // may have it.
  const canOffer = !global && release.status === 'available'

  return (
    <li
      className={cn(
        'flex flex-wrap items-center gap-x-4 gap-y-3 px-4 py-3.5',
        // Deprecated and withdrawn read differently at a glance, not only by
        // their label: an operator scanning fifty versions is looking for the
        // ones that need attention, and a word in a pill is not enough.
        release.status === 'deprecated' && 'bg-muted/40',
        release.status === 'withdrawn' && 'bg-destructive/5 opacity-70',
      )}
    >
      <span
        className={cn(
          'w-28 shrink-0 font-mono text-sm font-medium tabular-nums',
          release.status === 'withdrawn' && 'line-through',
        )}
      >
        {release.id.version}
      </span>

      <StatusBadge tone={RELEASE_STATUS_TONES[release.status]}>
        {RELEASE_STATUS_LABELS[release.status]}
      </StatusBadge>

      <StatusBadge tone={RISK_TONES[release.risk]} dot={false}>
        {RISK_LABELS[release.risk]}
      </StatusBadge>

      <StatusBadge tone={global ? 'success' : 'neutral'} dot={false} icon={<Globe className='h-3 w-3' />}>
        {audienceLabel(release.rollout)}
      </StatusBadge>

      <span
        className={cn(
          'text-sm tabular-nums',
          release.deployments === 0 ? 'text-muted-foreground' : 'font-medium',
        )}
      >
        {release.deployments === 0
          ? 'nothing runs it'
          : `${release.deployments} deployment${release.deployments > 1 ? 's' : ''}`}
      </span>

      {stranded && (
        <span className='flex items-center gap-1.5 text-sm text-destructive'>
          <AlertTriangle className='h-3.5 w-3.5' />
          still serving
        </span>
      )}

      {release.notes && (
        <p className='w-full text-sm text-muted-foreground sm:w-auto sm:flex-1 sm:truncate'>
          {release.notes}
        </p>
      )}

      {canOffer && (
        <Button
          variant='outline'
          size='sm'
          className='ml-auto'
          disabled={isWidening}
          onClick={() => onOfferToEveryone(release.id.version)}
        >
          <Globe className='h-3.5 w-3.5' />
          Offer to everyone
        </Button>
      )}

      {forward.length > 0 && (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant='ghost' size='sm' disabled={isMoving} className={canOffer ? '' : 'ml-auto'}>
              Move
              <ChevronDown className='ml-1 h-3.5 w-3.5' />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align='end'>
            {forward.map((status) => (
              <DropdownMenuItem key={status} onClick={() => onMove(release.id.version, status)}>
                {RELEASE_STATUS_LABELS[status]}
                {status === 'withdrawn' && release.deployments > 0 && (
                  <span className='ml-2 text-xs text-destructive'>
                    {release.deployments} still on it
                  </span>
                )}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      )}
    </li>
  )
}
