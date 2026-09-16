import type { Schemas } from '@/api/api.client'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { newestInstallable } from '@/domain/releases/catalogue'
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
import { KIND_LABELS, type DeploymentKind, type Environment } from '../../types/deployment'
import {
  OFFER_COPY,
  describeResources,
  firstOpen,
  whyClosed,
  type Offer,
  type OfferAvailability,
} from '../../offers'
import { OptionCard } from './components/option-card'

interface Props {
  onSubmit: (data: {
    name: string
    kind: DeploymentKind
    version: string
    environment: Environment
    offer: Offer
  }) => void
  isSubmitting?: boolean
  /** What the platform said when the last attempt failed. */
  refusal?: string
  offers: OfferAvailability[]
  offersLoading: boolean
  releases: Record<DeploymentKind, Schemas.Release[]>
  releasesLoading: boolean
}

export default function PageCreateDeployment({
  onSubmit,
  isSubmitting = false,
  refusal,
  offers,
  offersLoading,
  releases,
  releasesLoading,
}: Props) {
  const navigate = useNavigate()
  const organisationPath = useOrganisationPath()

  const [name, setName] = useState('')
  const [kind, setKind] = useState<DeploymentKind>('ferriskey')
  const [environment, setEnvironment] = useState<Environment>('development')
  const [offer, setOffer] = useState<Offer | undefined>(undefined)
  const chosen = offer ?? firstOpen(offers)

  // Not a choice. A new instance starts on the newest version the catalogue
  // offers, and moving between versions is what the upgrade screen is for:
  // offering the choice twice invites somebody to create an instance already
  // behind, for no reason they could name.
  const version = newestInstallable(releases[kind]) ?? ''

  const canSubmit = !!chosen && name.trim() !== '' && version !== '' && !isSubmitting

  return (
    <Page className='max-w-3xl'>
      <PageTitle title='New deployment' />

      <form
        className='mt-8 space-y-8'
        onSubmit={(e) => {
          e.preventDefault()
          if (chosen) {
            onSubmit({ name, kind, version, environment, offer: chosen })
          }
        }}
      >
        <Section title='Identity provider'>
          <div className='grid gap-3 sm:grid-cols-2'>
            {(Object.keys(KIND_LABELS) as DeploymentKind[]).map((value) => (
              <OptionCard
                key={value}
                selected={kind === value}
                onSelect={() => setKind(value)}
                label={KIND_LABELS[value]}
              />
            ))}
          </div>

          <div className='mt-4 space-y-2'>
            <Label>Version</Label>
            {releasesLoading ? (
              <Skeleton className='h-9 w-full sm:w-72' />
            ) : version === '' ? (
              <p className='rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200'>
                No version of {KIND_LABELS[kind]} is available yet, so there is nothing to create.
                Someone operating the platform publishes one and makes it available.
              </p>
            ) : (
              <>
                <p className='font-mono text-sm'>{version}</p>
                <p className='text-xs text-muted-foreground'>
                  The newest available version, chosen for you. It can be upgraded afterwards,
                  which is where choosing a version belongs.
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
          </div>
        </Section>

        <Section title='Offer'>
          {offersLoading ? (
            <Skeleton className='h-24 w-full' />
          ) : offers.length === 0 ? (
            <p className='text-sm text-destructive'>
              This organisation's plan opens no offer, so there is nothing to deploy on.
            </p>
          ) : (
            <div className='grid gap-3 sm:grid-cols-2'>
              {offers.map((entry) => {
                const closed = whyClosed(entry)

                return (
                  <OptionCard
                    key={entry.offer}
                    selected={chosen === entry.offer}
                    disabled={!entry.open}
                    onSelect={() => entry.open && setOffer(entry.offer)}
                    label={OFFER_COPY[entry.offer].label}
                    description={OFFER_COPY[entry.offer].description}
                    footer={
                      <span className='font-mono text-xs text-muted-foreground'>
                        {closed ?? describeResources(entry)}
                      </span>
                    }
                  />
                )
              })}
            </div>
          )}
        </Section>

        {/* Where a deployment lands is the platform's decision, so the
            reasons it cannot land anywhere are the platform's to explain --
            and this is where somebody finds out, now that nothing on the form
            pre-empts it. */}
        {refusal && (
          <p className='rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive'>
            {refusal}
          </p>
        )}

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
