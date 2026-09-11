import type { Schemas } from '@/api/api.client'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { installableVersions } from '@/domain/releases/catalogue'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Page, PageTitle, Section } from '@/components/layout/page'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { useNavigate } from '@tanstack/react-router'
import { useState } from 'react'
import {
  DEPLOYMENT_SIZES,
  KIND_LABELS,
  type DeploymentKind,
  type DeploymentMode,
  type DeploymentSize,
  type Environment,
} from '../../types/deployment'
import { formatCpu, formatMemory, formatStorage } from '../../types/resources'
import { OptionCard } from './components/option-card'

interface Props {
  onSubmit: (data: {
    name: string
    kind: DeploymentKind
    version: string
    environment: Environment
    region: string
    mode: DeploymentMode
    size: DeploymentSize
  }) => void
  isSubmitting?: boolean
  regions: string[]
  regionsLoading: boolean
  releases: Record<DeploymentKind, Schemas.Release[]>
  releasesLoading: boolean
}

const MODES: { value: DeploymentMode; label: string; description: string }[] = [
  {
    value: 'shared',
    label: 'Shared',
    description: 'Placed on an existing cluster with room. Available immediately.',
  },
  {
    value: 'dedicated',
    label: 'Dedicated',
    description: 'A cluster of this organisation’s own. Takes longer to start.',
  },
]

export default function PageCreateDeployment({
  onSubmit,
  isSubmitting = false,
  regions,
  regionsLoading,
  releases,
  releasesLoading,
}: Props) {
  const navigate = useNavigate()
  const organisationPath = useOrganisationPath()

  const [name, setName] = useState('')
  const [kind, setKind] = useState<DeploymentKind>('ferriskey')
  const [environment, setEnvironment] = useState<Environment>('development')
  const [mode, setMode] = useState<DeploymentMode>('shared')
  const [size, setSize] = useState<DeploymentSize>('small')
  const [chosenRegion, setChosenRegion] = useState<string | null>(null)
  const [chosenVersion, setChosenVersion] = useState<string | null>(null)

  const region = chosenRegion ?? regions[0] ?? ''

  const offered = installableVersions(releases[kind])
  // Falls back to the newest rather than holding an empty value: the choice
  // that is right almost every time should cost nothing.
  const version =
    offered.find((release) => release.id.version === chosenVersion)?.id.version ??
    offered[0]?.id.version ??
    ''

  const canSubmit = name.trim() !== '' && region !== '' && version !== '' && !isSubmitting

  return (
    <Page className='max-w-3xl'>
      <PageTitle title='New deployment' />

      <form
        className='mt-8 space-y-8'
        onSubmit={(e) => {
          e.preventDefault()
          onSubmit({ name, kind, version, environment, region, mode, size })
        }}
      >
        <Section title='Identity provider'>
          <div className='grid gap-3 sm:grid-cols-2'>
            {(Object.keys(KIND_LABELS) as DeploymentKind[]).map((value) => (
              <OptionCard
                key={value}
                selected={kind === value}
                onSelect={() => {
                  setKind(value)
                  // The versions belong to the product, so a choice made for
                  // the other one means nothing here.
                  setChosenVersion(null)
                }}
                label={KIND_LABELS[value]}
              />
            ))}
          </div>

          <div className='mt-4 space-y-2'>
            <Label htmlFor='version'>Version</Label>
            {releasesLoading ? (
              <Skeleton className='h-9 w-full sm:w-72' />
            ) : offered.length === 0 ? (
              <p className='rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200'>
                No version of {KIND_LABELS[kind]} has been published yet, so there is nothing to
                create. Someone operating the platform publishes one from the release catalogue.
              </p>
            ) : (
              <>
                <Select value={version} onValueChange={setChosenVersion}>
                  <SelectTrigger id='version' className='w-full sm:w-72'>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {offered.map((release) => (
                      <SelectItem key={release.id.version} value={release.id.version}>
                        {release.id.version}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <p className='text-xs text-muted-foreground'>
                  The instance records the version it runs, so this is exact. It can be upgraded
                  afterwards.
                </p>
              </>
            )}
          </div>
        </Section>

        <Section title='Details'>
          <div className='grid gap-4 sm:grid-cols-2'>
            <div className='space-y-2'>
              <Label htmlFor='name'>Name</Label>
              <Input
                id='name'
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder='auth'
                required
              />
            </div>

            <div className='space-y-2'>
              <Label htmlFor='environment'>Environment</Label>
              <Select
                value={environment}
                onValueChange={(value) => setEnvironment(value as Environment)}
              >
                <SelectTrigger id='environment' className='w-full'>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value='development'>Development</SelectItem>
                  <SelectItem value='staging'>Staging</SelectItem>
                  <SelectItem value='production'>Production</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className='space-y-2 sm:col-span-2'>
              <Label htmlFor='region'>Region</Label>
              <Select value={region} onValueChange={setChosenRegion} disabled={regions.length === 0}>
                <SelectTrigger id='region' className='w-full'>
                  <SelectValue placeholder={regionsLoading ? 'Loading…' : 'Select a region'} />
                </SelectTrigger>
                <SelectContent>
                  {regions.map((served) => (
                    <SelectItem key={served} value={served}>
                      {served}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {!regionsLoading && regions.length === 0 && (
                <p className='text-xs text-destructive'>
                  No data plane is registered, so there is nowhere to deploy.
                </p>
              )}
            </div>
          </div>
        </Section>

        <Section title='Isolation'>
          <div className='grid gap-3 sm:grid-cols-2'>
            {MODES.map((option) => (
              <OptionCard
                key={option.value}
                selected={mode === option.value}
                onSelect={() => setMode(option.value)}
                label={option.label}
                description={option.description}
              />
            ))}
          </div>
        </Section>

        <Section title='Size'>
          <div className='grid gap-3 sm:grid-cols-3'>
            {(Object.keys(DEPLOYMENT_SIZES) as DeploymentSize[]).map((value) => {
              const { label, description, resources } = DEPLOYMENT_SIZES[value]

              return (
                <OptionCard
                  key={value}
                  selected={size === value}
                  onSelect={() => setSize(value)}
                  label={label}
                  description={description}
                  footer={
                    <span className='font-mono text-xs text-muted-foreground'>
                      {formatCpu(resources.cpuMillis)} · {formatMemory(resources.memoryMib)} ·{' '}
                      {formatStorage(resources.storageGib)}
                    </span>
                  }
                />
              )
            })}
          </div>
        </Section>

        <div className='flex items-center justify-end gap-2 border-t pt-6'>
          <Button
            type='button'
            variant='ghost'
            onClick={() => navigate({ to: organisationPath('/deployments') })}
          >
            Cancel
          </Button>
          <Button type='submit' disabled={!canSubmit}>
            {isSubmitting ? 'Creating…' : 'Create deployment'}
          </Button>
        </div>
      </form>
    </Page>
  )
}
